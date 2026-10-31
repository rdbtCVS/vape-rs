#!/usr/bin/env python3
"""TriggerBot against a real player over a LAN connection: Shield check and
Select first hit.

A fresh host world is opened to LAN on localhost and a fresh guest client
joins it (each process with its own isolated homes); both run vape-rs with
TriggerBot bound. Both players are in survival with Resistance V, an empty
main hand and a shield in the offhand, face to face 2.2 blocks apart, each
with a wall at their back. The host's hurt log gives every hurt of either
player (with what a shield blocked), so it shows when each TriggerBot hit:

* host TriggerBot (Shield check on): hits the guest, never while the
  guest's shield is up (the server sees the guest using it, and a physical
  click is blocked by it), and again once it is lowered (the host reads the
  guest's blocking from the server's model of the guest);
* guest TriggerBot, Shield check off: hits the host through the raised
  shield (each hit blocked);
* after a rejoin with Shield check on: no hit while the host's shield is up,
  hits once it is lowered (the guest reads the host's blocking from the
  remote player's entity data);
* after a rejoin with Select first hit: no hit until the host hits the guest
  once, then hits; after the guest looks away for over a second, no hit
  again.

Results go to <guest session>/result.json, measurements to
measurements.json.
"""
import argparse
import json
import os

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks, poll

FLOOR = 99
BINDS = {'combat.trigger-bot': 'KeyB'}
# The host's spot faces +z (yaw 0), the guest's faces -z (yaw 180); walls at
# z -1 and z 3 keep the knockback from pushing either out of reach.
HOST = (0.5, 0.4, 0)
GUEST = (0.5, 2.6, 180)


class Pair:
    def __init__(self, hs, gs, checks, address):
        self.hs, self.gs, self.checks, self.address = hs, gs, checks, address
        self.host = Run(hs, checks, BINDS)
        self.guest = Run(gs, checks, BINDS)
        self.m = {}

    def command(self, text):
        self.host.call('command', text)

    def effects(self):
        self.command('effect give @a minecraft:resistance 100000 4 true')
        self.command('effect give @a minecraft:saturation 100000 4 true')

    def shields(self):
        self.command('item replace entity @s weapon.offhand with minecraft:shield')
        self.command('item replace entity VapeGuest weapon.offhand with minecraft:shield')
        for name, r in (('host', self.host), ('guest', self.guest)):
            ok, s = poll(r.s, lambda s: (s.get('offhand') or {}).get('item') == 'minecraft:shield', 30)
            self.checks.check(f'the {name} holds a shield in the offhand', ok, s.get('offhand'))

    def place(self):
        for r in (self.host, self.guest):
            r.release()
            r.call('mouse', 'left', 'up')
        self.command(f'tp VapeGuest {GUEST[0]} {FLOOR + 1} {GUEST[1]} {GUEST[2]} 0')
        self.host.call('tp', HOST[0], FLOOR + 1, HOST[1])
        self.host.call('look', HOST[2], '0')
        self.guest.wait(10)
        self.guest.call('look', GUEST[2], '0')
        self.guest.wait(6)

    def other_near(self, r):
        # The agent's looking_at lists mobs only; the hits below show the aim.
        found = r.call('entities', '4', 'player').get('entities', [])
        return bool(found), found

    def hurts(self, ticks, then=None):
        """Each player's hurts the server logs over `ticks` (from `then`, if
        given), by name."""
        self.host.call('hurtlog')
        if then:
            then()
        self.guest.wait(ticks)
        out = {'host': [], 'guest': []}
        # The host's entry carries its profile name, each guest's its own.
        for entry in self.host.call('hurtlog').get('players', []):
            out['guest' if entry['name'] == 'VapeGuest' else 'host'] += entry.get('hurts', [])
        return out

    def raise_shield(self, r):
        r.call('mouse', 'right', 'down')
        r.wait(10)
        return r.call('status').get('using')

    def guest_using(self):
        guests = self.host.call('lan', 'status').get('guests', [])
        return next((g.get('using') for g in guests if g.get('name') == 'VapeGuest'), None)

    def rejoin(self, changes):
        gs = self.gs
        gs.call('quit')
        gs.process.wait(timeout=15)
        gs.log.close()
        gs.process = None
        document = gs.saved()
        document['settings'].update(changes)
        gs.seed(document)
        gs.launch(creative=False, world=None, extra=('--server', self.address, '--name', 'VapeGuest'))
        gs.call('wait-ready', '60')
        self.guest.wait(20)
        self.effects()
        saved = gs.saved()['settings']
        self.checks.check(f'guest rejoined with {sorted(changes)}',
                          all(saved.get(k) == v for k, v in changes.items())
                          and self.guest.enabled('combat.trigger-bot'), {k: saved.get(k) for k in changes})
        self.shields()
        self.place()


def summary(hurts):
    return [{k: h.get(k) for k in ('t', 'source', 'amount', 'blocked', 'hurt')} for h in hurts]


def run(resources, allow_stale):
    with NativeSession(resources, 'trigger-lan-host-', allow_stale=allow_stale) as hs, \
            NativeSession(resources, 'trigger-lan-guest-', allow_stale=allow_stale) as gs:
        checks = Checks(gs.root)
        binds = {f'{k}.binds': [v] for k, v in BINDS.items()}
        hs.seed({'schema': 1, 'modules': {}, 'settings': {**binds, 'combat.trigger-bot.shield-check': True}})
        hs.launch(creative=False, world='TriggerHost')
        host = Run(hs, checks, BINDS)
        host.call('time', '6000')
        host.call('tp', HOST[0], FLOOR + 1, HOST[1])
        host.call('wait-ready', '40')
        for _ in range(5):
            host.call('fill', '-8', str(FLOOR - 3), '-8', '8', str(FLOOR), '8', 'minecraft:obsidian')
            host.call('fill', '-8', str(FLOOR + 1), '-8', '8', str(FLOOR + 6), '8', 'minecraft:air')
            floor = host.call('blocks', '-8', str(FLOOR), '-8', '8', str(FLOOR), '8').get('counts', {})
            if floor.get('minecraft:obsidian') == 17 * 17:
                break
            host.wait(20)
        for z in (-1, 3):
            host.call('fill', '-2', str(FLOOR + 1), str(z), '2', str(FLOOR + 3), str(z), 'minecraft:obsidian')
        checks.check('obsidian floor is in place', floor.get('minecraft:obsidian') == 17 * 17, floor)
        host.call('command', 'gamerule mob_spawning false')
        host.call('command', 'difficulty peaceful')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        address = f'localhost:{port}'

        gs.seed({'schema': 1, 'modules': {}, 'settings': binds})
        gs.launch(creative=False, world=None, extra=('--server', address, '--name', 'VapeGuest'))
        gs.call('wait-ready', '60')
        p = Pair(hs, gs, checks, address)
        p.guest.wait(20)
        p.effects()
        p.shields()
        p.place()
        for name, r in (('host', p.host), ('guest', p.guest)):
            ok, found = p.other_near(r)
            checks.check(f'the {name} sees the other player within 4 blocks', ok, found)

        # --- Host TriggerBot, Shield check on --------------------------------
        host.toggle('combat.trigger-bot', True)
        open_hits = p.hurts(60)['guest']
        checks.check('host TriggerBot hits the guest', len(open_hits) >= 2, summary(open_hits))
        using = p.raise_shield(p.guest)
        served = p.guest_using()
        checks.check('the guest raises its shield (the server sees it in use)', bool(using) and bool(served),
                     [using, served])
        s0 = host.call('state').get('swings', 0)
        blocked_window = p.hurts(60)['guest']
        swung = host.call('state').get('swings', 0) - s0
        checks.check('Shield check: host TriggerBot does not hit the guest behind its shield',
                     not blocked_window and swung == 0, [summary(blocked_window), swung])
        host.toggle('combat.trigger-bot', False)
        host.call('mouse', 'left', 'click')
        clicked = p.hurts(10)['guest']
        checks.check("a physical click is blocked by the guest's shield",
                     bool(clicked) and all(h.get('blocked', 0) > 0 for h in clicked), summary(clicked))
        p.guest.call('mouse', 'right', 'up')
        p.guest.wait(6)
        host.toggle('combat.trigger-bot', True)
        lowered = p.hurts(60)['guest']
        checks.check('Shield check: host TriggerBot hits again once the shield is lowered', len(lowered) >= 1,
                     summary(lowered))
        host.toggle('combat.trigger-bot', False)
        p.m['host_trigger'] = {'open': summary(open_hits), 'guest_using': [using, served],
                               'shielded': summary(blocked_window), 'shielded_swings': swung,
                               'click_on_shield': summary(clicked), 'lowered': summary(lowered)}

        # --- Guest TriggerBot, Shield check off ------------------------------
        p.place()
        p.guest.toggle('combat.trigger-bot', True)
        using = p.raise_shield(host)
        through = p.hurts(60)['host']
        checks.check('the host raises its shield', bool(using), using)
        checks.check('Shield check off: guest TriggerBot hits the host through its shield, each hit blocked',
                     len(through) >= 1 and all(h.get('blocked', 0) > 0 for h in through), summary(through))
        host.call('mouse', 'right', 'up')
        p.m['guest_no_shield_check'] = {'host_using': using, 'shielded': summary(through)}

        # --- Guest TriggerBot, Shield check on -------------------------------
        p.rejoin({'combat.trigger-bot.shield-check': True})
        using = p.raise_shield(host)
        s0 = p.guest.call('state').get('swings', 0)
        shielded = p.hurts(60)['host']
        swung = p.guest.call('state').get('swings', 0) - s0
        checks.check('Shield check: guest TriggerBot does not hit the host behind its shield',
                     bool(using) and not shielded and swung == 0, [using, summary(shielded), swung])
        host.call('mouse', 'right', 'up')
        host.wait(6)
        lowered = p.hurts(60)['host']
        checks.check('Shield check: guest TriggerBot hits the host once the shield is lowered', len(lowered) >= 1,
                     summary(lowered))
        p.m['guest_shield_check'] = {'host_using': using, 'shielded': summary(shielded), 'shielded_swings': swung,
                                     'lowered': summary(lowered)}

        # --- Guest TriggerBot, Select first hit ------------------------------
        p.rejoin({'combat.trigger-bot.shield-check': False, 'combat.trigger-bot.select-first-hit': True})
        waiting = p.hurts(40)['host']
        checks.check('Select first hit: the guest does not hit the host first', not waiting, summary(waiting))
        struck = p.hurts(40, lambda: host.call('mouse', 'left', 'click'))
        checks.check('the host hits the guest once', len(struck['guest']) >= 1, summary(struck['guest']))
        checks.check('Select first hit: the guest hits back once the host hit it', len(struck['host']) >= 1,
                     summary(struck['host']))
        p.guest.call('look', '0', '-60')
        p.guest.wait(30)
        p.guest.call('look', GUEST[2], '0')
        p.guest.wait(2)
        reset = p.hurts(40)['host']
        checks.check('Select first hit: over a second looking away resets the wait', not reset, summary(reset))
        p.m['guest_select_first_hit'] = {'waiting': summary(waiting), 'host_click': summary(struck['guest']),
                                         'after_hit': summary(struck['host']), 'after_look_away': summary(reset)}
        p.guest.toggle('combat.trigger-bot', False)

        gs.capture('final')
        hs.capture('host-final')
        alive = gs.call('state')
        checks.check('guest still connected (no kick)', alive.get('player') is not None, alive.get('screen'))
        for name, session in (('host', hs), ('guest', gs)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
        (gs.root / 'measurements.json').write_text(json.dumps(p.m, indent=2, default=str))
        gs.call('disconnect')
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}', gs.root)
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?', default=os.environ.get('HORIZON_RESOURCES'))
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run(args.resources, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
