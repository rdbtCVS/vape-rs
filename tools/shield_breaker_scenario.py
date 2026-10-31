#!/usr/bin/env python3
"""ShieldBreaker, HitSwap's axe and AutoMace's Stun slam over a real LAN
connection.

A fresh host world is opened to LAN on localhost and a fresh guest client
runs vape-rs against it (each process with its own isolated homes). The
host player stands two blocks in front of the guest on a stone floor,
facing it, with a shield in the off hand raised by holding the use button.
The host server gives the guest's hotbar: a diamond sword (slot 1,
selected), an iron axe (2) and a Density V mace (3). What the server does
is read from what it sends and records: the host's use flag as the guest
draws it (an axe's disable drops it to 0 while the host still holds the
button; the host's own window keeps its local use state), and the host's
hurt log, whose amounts tell the weapon (sword 7, axe 9) and whether the
shield blocked it:

* nothing on: a sword click is blocked and the shield stays up;
* ShieldBreaker (defaults: swap delay 5, swap back 5): the click is
  swallowed, the axe is selected, about 5 ticks later the axe hits and
  disables the shield, and about 5 ticks after that the sword comes back;
* ShieldBreaker with the shield lowered: no swap;
* ShieldBreaker turned off right after the click, inside its swap delay:
  the sword comes back and no axe hit follows;
* HitSwap (Axes): a sword click on a shield up for more than 5 ticks is
  swallowed, the axe is selected and HitSwap's own attack follows on the
  next tick, so the slot change and the attack reach the server in the same
  tick. The host takes a guest's damage, attack speed and shield disable from
  `attack_item`, the main-hand item at the guest's last server tick, so the
  hit lands with the sword's 7 and no disable while the axe is selected; the
  sword comes back;
* AutoMace Stun slam (Smash only off, Chance 100), clicking with the axe in
  hand: the axe hit disables the shield and the Density mace follow-up comes
  one tick later (EventPostAttack runs at the attack's HEAD), again in its
  slot change's server tick: it lands with the axe's modifiers at one tick of
  charge (about 1.83) and wears the mace; then the axe comes back.

Both players have Resistance V. Results go to <guest session>/result.json,
measurements to measurements.json.
"""
import argparse
import json
import socket
import time
from pathlib import Path

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 99
BINDS = {'combat.shield-breaker': 'KeyN', 'combat.hit-swap': 'KeyH', 'combat.auto-mace': 'KeyM'}
SETTINGS = {
    'combat.auto-mace.targets.players': True,
    'combat.auto-mace.stun-slam': True,
    'combat.auto-mace.chance': 100.0,
    'combat.auto-mace.smash-only': False,
}
# Full-strength hits: a diamond sword, an iron axe.
SWORD, AXE = 7.0, 9.0
HOTBAR = ('minecraft:diamond_sword', 'minecraft:iron_axe',
          'minecraft:mace[minecraft:enchantments={"minecraft:density":5}]')


def other_player(session):
    found = session.call('entities', '12', 'player').get('entities', [])
    return found[0] if found else None


def aimed(guest, target):
    seen = guest.call('state').get('looking_at') or {}
    return (seen.get('entity') or {}).get('id') == target['id']


def place(host, guest):
    """Guest at z=0.5 facing +z, host at z=2.5 facing the guest, both on the
    floor; the guest's crosshair on the host. True once it is."""
    guest.release()
    host.call('command', f'tp VapeGuest 0.5 {FLOOR + 1} 0.5 0 0')
    host.call('tp', '0.5', str(FLOOR + 1), '2.5')
    host.call('look', '180', '0')
    for _ in range(20):
        guest.wait(10)
        g, h = guest.player(), host.player()
        if g.get('on_ground') and h.get('on_ground') and abs(g['position'][1] - FLOOR - 1) < 0.01:
            break
    target = other_player(guest)
    for _ in range(5):
        if not target:
            return False
        guest.call('look-at-entity', str(target['id']))
        guest.wait(2)
        if aimed(guest, target):
            return True
    return False


def host_seen(guest):
    """The host's shield use (ticks) as the server's synced flags show it to
    the guest; 0 when it is down."""
    seen = (guest.call('status').get('server') or {}).get('player_equipment') or []
    return seen[0].get('using', 0) if seen else None


def shield(host, guest, raised=True):
    """Lowers the host's shield and raises it again until the server has it
    up (an axe's disable keeps it down for 5 seconds), held past the 5-tick
    warm-up. The host's use ticks as the guest sees them."""
    host.call('mouse', 'right', 'up')
    host.wait(4)
    if not raised:
        return host_seen(guest)
    for _ in range(12):
        host.call('mouse', 'right', 'down')
        host.wait(10)
        if (host_seen(guest) or 0) > 5:
            break
        host.call('mouse', 'right', 'up')
        host.wait(20)
    return host_seen(guest)


def hurts(host):
    """The host's hurts since the last call (its entry has its own name)."""
    out = []
    for p in host.call('hurtlog').get('players', []):
        if p.get('name') != 'VapeGuest':
            out += p.get('hurts', [])
    return out


def wear(guest):
    return [h.get('damage', 0) for h in guest.call('inventory').get('hotbar', [])[:3]]


def stock(host, guest, checks):
    host.call('command', 'clear VapeGuest')
    guest.wait(10)
    for item in HOTBAR:
        host.call('command', f'give VapeGuest {item} 1')
        guest.wait(6)
    ok, bar = False, []
    for _ in range(20):
        bar = guest.call('inventory').get('hotbar', [])[:3]
        text = [json.dumps(h) for h in bar]
        ok = (len(bar) == 3 and 'diamond_sword' in text[0] and 'iron_axe' in text[1]
              and 'density' in text[2])
        if ok:
            break
        guest.wait(10)
    checks.check('guest hotbar: sword, iron axe, Density V mace (server-side)', ok, bar)
    guest.call('select', '1')
    guest.wait(4)
    return bar


class Probe:
    """One kept connection to a session's control socket (text lines in, one
    JSON line out, run between frames): fast enough to see a slot held for a
    single tick, which a `horizon-ctl` process per sample can miss."""

    def __init__(self, session):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(str(session.socket))
        self.reader = self.sock.makefile('r')

    def call(self, *args):
        self.sock.sendall((' '.join(map(str, args)) + '\n').encode())
        return json.loads(self.reader.readline())

    def close(self):
        self.reader.close()
        self.sock.close()


def click(host, guest, gs, name, toggle=None, seconds=1.2):
    """The guest clicks the host; samples the guest's selected slot and the
    host's shield use as the server shows it (ms since the click) and
    captures the first swap. With toggle, that module's bind is tapped as
    soon as the first swap shows (inside a 5-tick swap delay)."""
    hurts(host)
    worn = wear(guest)
    start = guest.call('status').get('selected')
    probe = Probe(gs)
    probe.call('mouse', 'left', 'click')
    t0 = time.monotonic()
    samples, shot = [], False
    while time.monotonic() - t0 < seconds:
        st = probe.call('status')
        seen = (st.get('server') or {}).get('player_equipment') or [{}]
        samples.append({'ms': round((time.monotonic() - t0) * 1000), 'selected': st.get('selected'),
                        'shield': seen[0].get('using', 0)})
        if not shot and st.get('selected') != start:
            if toggle:
                probe.call('key', toggle, 'tap')
            gs.capture(f'{name}-swapped')
            shot = True
    probe.close()
    guest.wait(6)
    log = hurts(host)
    used = [b - a for a, b in zip(worn, wear(guest))]
    swaps = [s for a, s in zip(samples, samples[1:]) if s['selected'] != a['selected']]
    down = next((s['ms'] for s in samples if s['shield'] == 0), None)
    return {'samples': samples, 'swaps': swaps, 'selected_seen': sorted({s['selected'] for s in samples}),
            'selected_after': guest.call('status').get('selected'), 'disabled_ms': down,
            'shield_after': host_seen(guest), 'host_holds_use': host.call('state').get('using'),
            'hurts': [{k: h.get(k) for k in ('amount', 'blocked', 'hurt', 't')} for h in log],
            'wear': used, 'worn_slots': [i + 1 for i, d in enumerate(used) if d]}


def summary(r):
    return {k: r[k] for k in ('selected_seen', 'selected_after', 'disabled_ms', 'shield_after',
                              'host_holds_use', 'hurts', 'worn_slots', 'swaps')}


def near(found, expected):
    return len(found) == len(expected) and all(abs(a - b) < 0.6 for a, b in zip(found, expected))


def amounts(r, blocked):
    """The hurt amounts, only fully blocked (or only landed) ones."""
    return [h['amount'] for h in r['hurts']
            if (h.get('blocked', 0) >= h['amount'] - 1e-6) == blocked and not (blocked and h.get('hurt'))]


def run(resources, allow_stale):
    with NativeSession(resources, 'shield-host-', allow_stale=allow_stale) as hs, \
            NativeSession(resources, 'shield-guest-', allow_stale=allow_stale) as gs:
        checks = Checks(gs.root)
        m = {}
        hs.seed({'schema': 1, 'modules': {}, 'settings': {}})
        hs.launch(creative=False, world='ShieldHost')
        host = Run(hs, checks, {})
        host.call('time', '6000')
        host.call('tp', '0.5', str(FLOOR + 1), '2.5')
        host.call('wait-ready', '40')
        # The server's world (the ctl `fill` only changes the window's copy).
        host.call('command', f'fill -8 {FLOOR - 3} -12 8 {FLOOR} 12 minecraft:stone')
        host.call('command', f'fill -8 {FLOOR + 1} -12 8 {FLOOR + 7} 12 minecraft:air')
        host.wait(10)
        host.call('command', 'gamerule spawn_mobs false')
        host.call('command', 'difficulty peaceful')
        host.call('command', 'item replace entity @s weapon.offhand with minecraft:shield')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        address = f'localhost:{port}'

        gs.seed({'schema': 1, 'modules': {},
                 'settings': {**{f'{k}.binds': [v] for k, v in BINDS.items()}, **SETTINGS}})
        gs.launch(creative=False, world=None, extra=('--server', address, '--name', 'VapeGuest'))
        guest = Run(gs, checks, BINDS)
        guest.call('wait-ready', '60')
        host.call('command', 'effect give @a minecraft:resistance 100000 4 true')
        host.call('command', 'effect give @a minecraft:saturation 100000 4 true')
        m['inventory'] = stock(host, guest, checks)
        checks.check('guest joined, stands on the floor and aims at the host', place(host, guest),
                     {'guest': guest.player()['position'], 'host': other_player(guest)})
        using = shield(host, guest)
        checks.check('the server has the host raising the off-hand shield', (using or 0) > 5, using)

        # --- nothing on ------------------------------------------------------
        r = m['baseline'] = click(host, guest, gs, 'baseline')
        checks.check('nothing on: the sword hit (7) is blocked and the shield stays up',
                     r['selected_seen'] == [1] and near(amounts(r, True), [SWORD])
                     and r['disabled_ms'] is None and (r['shield_after'] or 0) > 0, summary(r))

        # --- ShieldBreaker ---------------------------------------------------
        guest.toggle('combat.shield-breaker', True)
        aim = place(host, guest)
        shield(host, guest)
        r = m['shield_breaker'] = click(host, guest, gs, 'shield-breaker')
        to_axe = next((s['ms'] for s in r['samples'] if s['selected'] == 2), None)
        back = next((s['ms'] for s in r['samples'] if to_axe is not None and s['ms'] > to_axe
                     and s['selected'] == 1), None)
        r['axe_ms'], r['back_ms'] = to_axe, back
        # The axe has been held for a few server ticks, so the host gives it
        # the axe's modifiers (at that partial charge) and its disable.
        checks.check('ShieldBreaker: the click becomes one blocked axe hit that disables the shield',
                     aim and to_axe is not None and len(amounts(r, True)) == 1
                     and not amounts(r, False) and r['disabled_ms'] is not None
                     and r['shield_after'] == 0 and r['host_holds_use'], summary(r))
        checks.check('ShieldBreaker: the axe waits the swap delay before attacking (about 5 ticks)',
                     to_axe is not None and r['disabled_ms'] is not None
                     and 150 <= r['disabled_ms'] - to_axe <= 900, {'axe_ms': to_axe, 'disabled_ms': r['disabled_ms']})
        checks.check('ShieldBreaker swaps back to the sword after the swap back delay',
                     back is not None and r['selected_after'] == 1 and r['disabled_ms'] is not None
                     and back > r['disabled_ms'] and back - to_axe >= 300,
                     {'axe_ms': to_axe, 'back_ms': back, 'disabled_ms': r['disabled_ms']})

        aim = place(host, guest)
        shield(host, guest, raised=False)
        r = m['shield_breaker_lowered'] = click(host, guest, gs, 'shield-breaker-lowered')
        checks.check('ShieldBreaker: a lowered shield takes the sword hit (7), no swap',
                     aim and r['selected_seen'] == [1] and near(amounts(r, False), [SWORD])
                     and r['worn_slots'] == [1], summary(r))

        # Turned off while waiting to attack (the bind is tapped once the axe
        # shows, inside the 5-tick swap delay): the slot comes back.
        aim = place(host, guest)
        shield(host, guest)
        r = m['shield_breaker_disabled'] = click(host, guest, gs, 'shield-breaker-disabled', toggle='n')
        checks.check('ShieldBreaker turned off while waiting: the sword is back and no axe hit follows',
                     aim and not guest.enabled('combat.shield-breaker') and 2 in r['selected_seen']
                     and r['selected_after'] == 1 and r['hurts'] == [] and r['disabled_ms'] is None,
                     summary(r))

        # --- HitSwap axe -------------------------------------------------------
        guest.toggle('combat.hit-swap', True)
        aim = place(host, guest)
        shield(host, guest)
        r = m['hit_swap_axe'] = click(host, guest, gs, 'hit-swap-axe')
        checks.check("HitSwap: the sword click is swallowed, the axe is selected and HitSwap's own "
                     'hit follows (one blocked hit)',
                     aim and 2 in r['selected_seen'] and len(amounts(r, True)) == 1
                     and not amounts(r, False), summary(r))
        checks.check("host: that hit shares its slot change's server tick and takes the guest's "
                     "last-tick weapon (the sword's 7, no disable)",
                     near(amounts(r, True), [SWORD]) and r['disabled_ms'] is None, summary(r))
        checks.check('HitSwap swaps back to the sword', r['selected_after'] == 1, summary(r))
        guest.toggle('combat.hit-swap', False)

        # --- AutoMace Stun slam ---------------------------------------------------
        guest.toggle('combat.auto-mace', True)
        aim = place(host, guest)
        guest.call('select', '2')
        guest.wait(20)
        shield(host, guest)
        r = m['stun_slam'] = click(host, guest, gs, 'stun-slam')
        landed = amounts(r, False)
        checks.check('Stun slam: the axe hit (9) disables the shield and the mace follow-up lands',
                     aim and near(amounts(r, True), [AXE]) and len(landed) == 1
                     and r['disabled_ms'] is not None and 3 in r['selected_seen'] and r['worn_slots'] == [3],
                     summary(r))
        # Iron axe at one tick of charge: 9 * (0.2 + 0.8 * (1.5 / 22.2)^2).
        checks.check("host: the mace follow-up takes the guest's last-tick weapon (the axe at one "
                     'tick of charge, about 1.83) and wears the mace',
                     len(landed) == 1 and abs(landed[0] - 1.833) < 0.2 and r['worn_slots'] == [3],
                     summary(r))
        checks.check('Stun slam swaps back to the axe', r['selected_after'] == 2, summary(r))
        guest.toggle('combat.auto-mace', False)
        host.call('mouse', 'right', 'up')

        for name, session in (('host', hs), ('guest', gs)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
        gs.capture('final')
        (gs.root / 'measurements.json').write_text(json.dumps(m, indent=2, default=str))
        gs.call('disconnect')
        failed = [r['check'] for r in checks.results if not r['ok']]
        print(f'{len(checks.results) - len(failed)}/{len(checks.results)} checks passed', gs.root, flush=True)
        return failed


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    raise SystemExit(1 if run(args.resources, args.allow_stale) else 0)
