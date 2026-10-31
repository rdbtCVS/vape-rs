#!/usr/bin/env python3
"""Sprint and SafeWalk on both sides of a real LAN connection.

A fresh host world is opened to LAN on localhost and a fresh guest client
joins it (each process with its own isolated homes); both run vape-rs with
Sprint (Cancel Invis on) and SafeWalk bound. Sprint is checked on the host
(the integrated server's own player) and on the guest (the remote movement
tick), each in survival on its own lane of a stone floor:

* without Sprint a forward walk does not sprint; with it the sprint key is
  pressed, the player sprints and walks faster;
* a pause menu hands the key back;
* Cancel Invis: an invisible survival player is not made to sprint, an
  invisible creative player (who may fly) is (reference `onPreEntityUpdate`);
* disabling Sprint releases the key.

SafeWalk is checked on the guest, on a 7x7 platform over a ten-block drop
(as in safe_walk_scenario.py): a strafe walks off without it and is held at
the edge with it, at either side; it never presses sneak or crouches; with
Direction Check a straight forward walk still leaves the edge, and after a
rejoin with Direction Check off a forward walk is held too.

Results go to <guest session>/result.json, measurements to
measurements.json.
"""
import argparse
import json
import math
import os
import time

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks, poll, state

BINDS = {'combat.sprint': 'KeyP', 'world.safe-walk': 'KeyN'}
SETTINGS = {'combat.sprint.cancel-invis': True}
CATCH = 88
PLATFORM = 99
LANES = {'host': 8.5, 'guest': -8.5}


def build(host, checks):
    for _ in range(10):
        host.call('fill', '-12', CATCH, '-12', '12', CATCH, '12', 'minecraft:stone')
        host.call('fill', '-12', CATCH + 1, '-12', '12', CATCH + 22, '12', 'minecraft:air')
        host.call('fill', '-3', PLATFORM, '-3', '3', PLATFORM, '3', 'minecraft:stone')
        counts = host.call('blocks', '-12', CATCH, '-12', '12', PLATFORM, '12').get('counts', {})
        if counts.get('minecraft:stone') == 25 * 25 + 49:
            break
        host.wait(20)
    checks.check('platform and catch floor are in place', counts.get('minecraft:stone') == 25 * 25 + 49, counts)


class Side:
    """One player of the pair: its Run, and server commands that target it."""

    def __init__(self, name, run, host, target):
        self.name, self.r, self.host, self.target = name, run, host, target

    def command(self, text):
        self.host.call('command', text.replace('@who', self.target))

    def place(self, x, y, z):
        self.r.release()
        self.r.call('key', 'w', 'up')
        self.command(f'tp @who {x} {y} {z} 0 0')
        self.r.wait(10)
        self.r.call('look', '0', '0')
        self.r.wait(6)

    def walk(self):
        """A forward walk on this side's lane: whether the sprint key was
        held and the player sprinted, and the speed over the last 20 ticks."""
        self.place(LANES[self.name], CATCH + 1, -10.5)
        self.r.call('key', 'w', 'down')
        sprinting, s = poll(self.r.s, lambda s: s['player'].get('sprinting'), 16)
        held = 'ControlLeft' in s.get('keys', [])
        start = self.r.pos()
        self.r.wait(20)
        end = self.r.player()
        self.r.call('key', 'w', 'up')
        self.r.wait(6)
        speed = math.hypot(end['position'][0] - start[0], end['position'][2] - start[2])
        return {'sprinting': bool(sprinting and end.get('sprinting')), 'key_held': held,
                'blocks_per_s': round(speed, 3), 'keys': s.get('keys', [])}


def sprint_checks(side, checks, m):
    n = side.name
    r = side.r
    base = side.walk()
    checks.check(f'{n}: a walk without Sprint does not sprint', not base['sprinting'], base)
    r.toggle('combat.sprint', True)
    on = side.walk()
    checks.check(f'{n}: Sprint presses the sprint key and the player sprints',
                 on['sprinting'] and on['key_held'], on)
    checks.check(f'{n}: the sprint is faster than the walk', on['blocks_per_s'] >= 1.15 * base['blocks_per_s'],
                 [on['blocks_per_s'], base['blocks_per_s']])

    # A pause menu hands the key back; it is pressed again after resuming.
    side.place(LANES[n], CATCH + 1, -10.5)
    r.call('key', 'w', 'down')
    poll(r.s, lambda s: 'ControlLeft' in s.get('keys', []), 16)
    r.call('pause')
    time.sleep(0.5)
    paused = state(r.s).get('keys', [])
    r.call('resume')
    r.call('key', 'w', 'down')
    resumed, s = poll(r.s, lambda s: 'ControlLeft' in s.get('keys', []), 20)
    r.call('key', 'w', 'up')
    checks.check(f'{n}: a pause menu releases the sprint key', 'ControlLeft' not in paused, paused)
    checks.check(f'{n}: the key is pressed again after the menu', resumed, s.get('keys'))

    # Cancel Invis: the latch is released first (disable), then invisibility.
    r.toggle('combat.sprint', False)
    off_keys = state(r.s).get('keys', [])
    checks.check(f'{n}: disabling Sprint releases the sprint key', 'ControlLeft' not in off_keys, off_keys)
    side.command('effect give @who minecraft:invisibility 600 0 true')
    r.wait(6)
    r.toggle('combat.sprint', True)
    invisible = side.walk()
    checks.check(f'{n}: Cancel Invis: no sprint while invisible in survival',
                 not invisible['sprinting'] and not invisible['key_held'], invisible)
    side.command('gamemode creative @who')
    r.wait(6)
    flier = side.walk()
    checks.check(f'{n}: Cancel Invis: an invisible creative player (may fly) sprints',
                 flier['sprinting'] and flier['key_held'], flier)
    side.command('gamemode survival @who')
    side.command('effect clear @who minecraft:invisibility')
    r.toggle('combat.sprint', False)
    r.wait(4)
    m[f'sprint_{n}'] = {'off': base, 'on': on, 'paused_keys': paused, 'invisible_survival': invisible,
                        'invisible_creative': flier}


def on_platform(p):
    x, y, z = p['position']
    return y > PLATFORM + 0.5 and -3.4 <= x <= 4.4 and -3.4 <= z <= 4.4, [round(v, 3) for v in (x, y, z)]


def edge_walk(side, key, ticks=50):
    side.place(0.5, PLATFORM + 1, 0.5)
    side.r.call('key', key, 'down')
    side.r.wait(ticks)
    side.r.call('key', key, 'up')
    side.r.wait(20)
    return on_platform(side.r.player())


def safe_walk_checks(side, checks, m):
    r = side.r
    off, at_off = edge_walk(side, 'a')
    checks.check('guest without SafeWalk: a strafe walks off the edge', not off, at_off)
    r.toggle('world.safe-walk', True)
    held, at_held = edge_walk(side, 'a')
    checks.check('guest SafeWalk holds a strafing player at the edge', held and at_held[0] > 3.5, at_held)
    other, at_other = edge_walk(side, 'd')
    checks.check('guest SafeWalk holds at the opposite edge', other and at_other[0] < -2.5, at_other)
    side.place(0.5, PLATFORM + 1, 0.5)
    r.call('key', 'a', 'down')
    r.wait(4)
    keys = r.call('status').get('keys', [])
    p = r.player()
    r.call('key', 'a', 'up')
    eye = p['eye'][1] - p['position'][1]
    checks.check('guest SafeWalk does not press sneak', 'ShiftLeft' not in keys, keys)
    checks.check('guest SafeWalk does not crouch (standing eye height)', not p.get('sneaking') and eye > 1.5, eye)
    forward, at_forward = edge_walk(side, 'w')
    checks.check('guest Direction Check: a straight forward walk leaves the edge', not forward, at_forward)
    r.toggle('world.safe-walk', False)
    gone, at_gone = edge_walk(side, 'a')
    checks.check('guest SafeWalk disabled: a strafe walks off again', not gone, at_gone)
    m['safe_walk_guest'] = {'off': at_off, 'held_a': at_held, 'held_d': at_other, 'forward': at_forward,
                            'disabled': at_gone, 'eye': eye, 'keys': keys}


def rejoin(gs, address, changes, modules):
    gs.call('quit')
    gs.process.wait(timeout=15)
    gs.log.close()
    gs.process = None
    document = gs.saved()
    document['settings'].update(changes)
    for module, enabled in modules.items():
        document['modules'][module] = {**document['modules'].get(module, {}), 'enabled': enabled}
    gs.seed(document)
    gs.launch(creative=False, world=None, extra=('--server', address, '--name', 'VapeGuest'))
    gs.call('wait-ready', '60')
    gs.call('wait', '20')


def run(resources, allow_stale):
    with NativeSession(resources, 'sprint-lan-host-', allow_stale=allow_stale) as hs, \
            NativeSession(resources, 'sprint-lan-guest-', allow_stale=allow_stale) as gs:
        checks = Checks(gs.root)
        m = {}
        seed = {'schema': 1, 'modules': {},
                'settings': {**{f'{k}.binds': [v] for k, v in BINDS.items()}, **SETTINGS}}
        hs.seed(seed)
        hs.launch(creative=False, world='SprintHost')
        host = Run(hs, checks, BINDS)
        host.call('time', '6000')
        host.call('wait-ready', '40')
        # Build the floor before moving onto it: a survival host placed at
        # y=89 over open terrain can fall to its death while the world loads.
        build(host, checks)
        host.call('tp', LANES['host'], CATCH + 1, 8.5)
        host.call('command', 'gamerule mob_spawning false')
        host.call('command', 'difficulty peaceful')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        address = f'localhost:{port}'

        gs.seed(seed)
        gs.launch(creative=False, world=None, extra=('--server', address, '--name', 'VapeGuest'))
        gs.call('wait-ready', '60')
        guest = Run(gs, checks, BINDS)
        guest.wait(20)
        host.call('command', 'effect give @a minecraft:resistance 100000 4 true')
        host.call('command', 'effect give @a minecraft:saturation 100000 4 true')
        sides = {'host': Side('host', host, host, '@s'), 'guest': Side('guest', guest, host, 'VapeGuest')}
        sides['guest'].place(LANES['guest'], CATCH + 1, -10.5)
        alive = state(gs)
        checks.check('guest joined the LAN world in survival',
                     alive['player']['health'] > 0 and alive.get('gamemode') == 'survival',
                     [alive.get('gamemode'), alive['player']['position']])

        sprint_checks(sides['host'], checks, m)
        sprint_checks(sides['guest'], checks, m)
        safe_walk_checks(sides['guest'], checks, m)

        # Direction Check off, through a rejoin with SafeWalk enabled.
        rejoin(gs, address, {'world.safe-walk.direction-check': False}, {'world.safe-walk': True})
        host.call('command', 'effect give VapeGuest minecraft:resistance 100000 4 true')
        checks.check('guest rejoined with SafeWalk on and Direction Check off',
                     guest.enabled('world.safe-walk')
                     and gs.saved()['settings'].get('world.safe-walk.direction-check') is False)
        held, at = edge_walk(sides['guest'], 'w')
        checks.check('guest Direction Check off: a forward walk is held too', held, at)
        m['safe_walk_guest']['forward_no_direction_check'] = at
        guest.toggle('world.safe-walk', False)

        gs.capture('final')
        hs.capture('host-final')
        alive = gs.call('state')
        checks.check('guest still connected (no kick)', alive.get('player') is not None, alive.get('screen'))
        for name, session in (('host', hs), ('guest', gs)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
        (gs.root / 'measurements.json').write_text(json.dumps(m, indent=2, default=str))
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
