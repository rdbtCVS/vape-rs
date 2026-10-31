#!/usr/bin/env python3
"""Player-target combat modules over a real LAN connection.

A fresh host world is opened to LAN on localhost and a fresh guest client
runs vape-rs against it (each process with its own isolated homes). The
modules under test run on the guest; the host player is the opponent:

* KnockbackDelay (Chance 100, 500 ms): the host hits the guest while the
  guest faces it; the push (the guest's own ClientboundSetEntityMotion)
  starts about 500 ms after the hurt instead of with it. Turning the module
  off while a push is held releases it at once, and a push held over a
  disconnect never replays into the next connection.
* JumpReset (Chance 100, Accuracy 100): the guest jumps on the tick the
  push lands, so the push arc rises to jump height.
* WTap (Chance 100, re-press 250 ms): a sprint hit on the host releases the
  forward key, so the guest stays unsprinting and slower for several ticks.

Both players have Resistance V (no damage, the hurt and knockback remain).
Results go to <guest session>/result.json, measurements to
measurements.json.
"""
import argparse
import json
import math
import time
from pathlib import Path

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 99
BINDS = {'combat.wtap': 'KeyG', 'combat.jump-reset': 'KeyH', 'combat.knockback-delay': 'KeyJ'}
SETTINGS = {
    'combat.wtap.chance': 100.0,
    'combat.wtap.re-press-delay': 250.0,
    'combat.jump-reset.chance': 100.0,
    'combat.jump-reset.accuracy': [100.0, 100.0],
    'combat.knockback-delay.chance': 100.0,
    'combat.knockback-delay.air-delay': [500.0, 500.0],
    'combat.knockback-delay.ground-delay': [500.0, 500.0],
}


def other_player(session):
    found = session.call('entities', '12', 'player').get('entities', [])
    return found[0] if found else None


def place(host, guest, gz=0.5, hz=2.5):
    """Guest at z=gz facing +z, host at z=hz facing the guest, both still."""
    for s in (host, guest):
        s.release()
    host.call('command', f'tp VapeGuest 0.5 {FLOOR + 1} {gz} 0 0')
    host.call('tp', '0.5', str(FLOOR + 1), str(hz))
    host.call('look', '180', '0')
    guest.wait(30)
    guest.call('look', '0', '0')
    guest.wait(4)


def struck(host, guest, toggle_on_hurt=False):
    """The host clicks the guest; guest samples (ms since the click,
    hurt_time, position, velocity, on_ground, displacement) polled as fast
    as the control socket answers, for 1.6 s. With toggle_on_hurt,
    KnockbackDelay's bind is tapped as soon as the hurt is seen."""
    target = other_player(host)
    if not target:
        return []
    host.call('look-at-entity', str(target['id']))
    start = guest.player()['position']
    host.call('mouse', 'left', 'click')
    t0 = time.monotonic()
    out, toggled = [], False
    while time.monotonic() - t0 < 1.6:
        p = guest.call('state')['player']
        sample = {'ms': round((time.monotonic() - t0) * 1000), 'hurt_time': p.get('hurt_time', 0),
                  'pos': p['position'], 'vel': p['velocity'], 'on_ground': p['on_ground'],
                  'moved': math.hypot(p['position'][0] - start[0], p['position'][2] - start[2])}
        if toggle_on_hurt and not toggled and sample['hurt_time'] > 0:
            guest.call('key', 'j', 'tap')
            sample['toggled_ms'] = round((time.monotonic() - t0) * 1000)
            toggled = True
        out.append(sample)
    return out


def push_delay(trace):
    """ms from the first sample showing the hurt to the first sample
    displaced by more than 0.1 (and the toggle time, if any)."""
    hurt = next((s for s in trace if s['hurt_time'] > 0), None)
    moved = next((s for s in trace if s['moved'] > 0.1), None)
    toggle = next((s['toggled_ms'] for s in trace if 'toggled_ms' in s), None)
    if not hurt or not moved:
        return None
    return {'hurt_ms': hurt['ms'], 'moved_ms': moved['ms'], 'delay': moved['ms'] - hurt['ms'],
            'toggle_ms': toggle}


def peak(trace):
    return max(s['pos'][1] for s in trace) - FLOOR - 1 if trace else None


def sprint_hit(host, guest):
    """The guest sprints at the still host and clicks it two blocks out; the
    guest's sprint flag and forward progress for 12 ticks after the click,
    and whether the host was hurt."""
    place(host, guest, gz=-6.5, hz=3.5)
    guest.call('key', 'ctrl', 'down')
    guest.call('key', 'w', 'down')
    before = None
    for _ in range(40):
        guest.wait(1)
        p = guest.player()
        if p.get('sprinting') and p['position'][2] > 0.6:
            before = p
            break
    guest.call('mouse', 'left', 'click')
    samples = []
    for _ in range(12):
        guest.wait(1)
        p = guest.player()
        samples.append({'sprinting': p.get('sprinting'), 'z': p['position'][2]})
    guest.release()
    hurt = host.player().get('hurt_time', 0) > 0 or any(
        h for e in host.call('hurtlog').get('players', []) for h in e.get('hurts', []))
    return before, samples, hurt


def run(resources, allow_stale):
    with NativeSession(resources, 'pvp-host-', allow_stale=allow_stale) as hs, \
            NativeSession(resources, 'pvp-guest-', allow_stale=allow_stale) as gs:
        checks = Checks(gs.root)
        m = {}
        hs.seed({'schema': 1, 'modules': {}, 'settings': {}})
        hs.launch(creative=False, world='PvpHost')
        host = Run(hs, checks, {})
        host.call('time', '6000')
        host.call('tp', '0.5', str(FLOOR + 1), '0.5')
        host.call('wait-ready', '40')
        host.call('fill', '-8', str(FLOOR - 3), '-12', '8', str(FLOOR), '12', 'minecraft:stone')
        host.call('fill', '-8', str(FLOOR + 1), '-12', '8', str(FLOOR + 7), '12', 'minecraft:air')
        host.call('command', 'gamerule mob_spawning false')
        host.call('command', 'difficulty peaceful')
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
        place(host, guest)
        checks.check('guest joined and sees the host player', other_player(guest) is not None,
                     guest.player()['position'])

        # --- KnockbackDelay ---------------------------------------------
        off = struck(host, guest)
        place(host, guest)
        guest.toggle('combat.knockback-delay', True)
        on = struck(host, guest)
        place(host, guest)
        # Disable while the push is held: it lands at once.
        cut = struck(host, guest, toggle_on_hurt=True)
        guest.wait(4)
        cut_off = not guest.enabled('combat.knockback-delay')
        d_off, d_on, d_cut = push_delay(off), push_delay(on), push_delay(cut)
        m['knockback_delay'] = {'off': off, 'on': on, 'disabled_mid_hold': cut,
                                'delays': [d_off, d_on, d_cut]}
        checks.check('a host hit pushes the guest with the hurt without KnockbackDelay',
                     d_off is not None and d_off['delay'] <= 150, d_off)
        checks.check('KnockbackDelay 500 ms: the push starts about 500 ms after the hurt',
                     d_on is not None and 380 <= d_on['delay'] <= 900, {'off': d_off, 'on': d_on})
        checks.check('disabling KnockbackDelay mid-hold releases the push at once',
                     cut_off and d_cut is not None and d_cut['toggle_ms'] is not None
                     and d_cut['moved_ms'] - d_cut['toggle_ms'] <= 250 and d_cut['delay'] < 380,
                     {'disabled': cut_off, 'cut': d_cut})

        # Held over a disconnect: nothing replays into the next connection.
        place(host, guest)
        guest.toggle('combat.knockback-delay', True)
        target = other_player(host)
        host.call('look-at-entity', str(target['id']))
        host.call('mouse', 'left', 'click')
        guest.wait(1)
        gs.call('disconnect')
        time.sleep(1)
        gs.call('connect', address)
        gs.call('wait-ready', '60')
        guest.wait(10)
        host.call('command', f'tp VapeGuest 0.5 {FLOOR + 1} -4.5 0 0')
        guest.wait(20)
        settle = guest.player()['position']
        guest.wait(20)
        later = guest.player()
        drift = math.hypot(later['position'][0] - settle[0], later['position'][2] - settle[2])
        m['knockback_delay_disconnect'] = {'settle': settle, 'later': later['position'], 'drift': drift}
        checks.check('a push held over a disconnect never replays after reconnecting',
                     drift < 0.05 and later['velocity'][0] == 0 and later['velocity'][2] == 0,
                     {'drift': drift, 'velocity': later['velocity']})
        guest.toggle('combat.knockback-delay', False)

        # --- JumpReset ----------------------------------------------------
        place(host, guest)
        plain = struck(host, guest)
        place(host, guest)
        guest.toggle('combat.jump-reset', True)
        jump = struck(host, guest)
        place(host, guest)
        guest.toggle('combat.jump-reset', False)
        after = struck(host, guest)
        p_off, p_on, p_after = peak(plain), peak(jump), peak(after)
        m['jump_reset'] = {'off': plain, 'on': jump, 'after': after, 'peaks': [p_off, p_on, p_after]}
        checks.check('the push lands without JumpReset (a knockback arc)', p_off is not None and p_off > 0.2, p_off)
        checks.check('JumpReset: the guest jumps as the push lands (a jump-height arc)',
                     p_on is not None and p_off is not None and p_on > p_off + 0.1,
                     {'off_peak': p_off, 'jump_reset_peak': p_on})
        checks.check('JumpReset disabled: the knockback arc again',
                     p_after is not None and abs(p_after - p_off) < 0.1, {'off': p_off, 'after': p_after})

        # --- WTap ------------------------------------------------------------
        b_off, s_off, hit_off = sprint_hit(host, guest)
        guest.toggle('combat.wtap', True)
        guest.wait(30)
        b_on, s_on, hit_on = sprint_hit(host, guest)
        guest.toggle('combat.wtap', False)
        un_off = sum(1 for s in s_off if not s['sprinting'])
        un_on = sum(1 for s in s_on if not s['sprinting'])
        m['wtap'] = {'off': s_off, 'on': s_on, 'before': [b_off, b_on], 'hit': [hit_off, hit_on]}
        checks.check('WTap: the guest was sprinting at the host before each click',
                     bool(b_off) and bool(b_on), [bool(b_off), bool(b_on)])
        checks.check('WTap: both sprint clicks hurt the host', hit_off and hit_on, [hit_off, hit_on])
        checks.check('WTap: after a sprint hit the forward key is released, so sprint stays off longer',
                     un_on >= un_off + 2, {'off_unsprinting_ticks': un_off, 'on_unsprinting_ticks': un_on})

        for name, session in (('host', hs), ('guest', gs)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
            checks.check(f'no replay failures in the {name} log', 'replaying held packets failed' not in log)
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
