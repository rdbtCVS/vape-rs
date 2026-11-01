#!/usr/bin/env python3
"""FastPlace "Held Item" (All / Blocks / Projectiles) in a fresh isolated
survival world on the integrated server.

The reference (`FastPlace.onPreTick`) lowers the right-click delay only when
the held items match the mode: Blocks needs a block item in the main or off
hand, Projectiles a snowball, egg, ender pearl, eye of ender or fire charge
in either hand, All anything. Each probe holds the use key for a fixed number
of client ticks and counts what the server consumed (the inventory counts
include the off hand):

- snowball in the main hand, aimed at the sky (a projectile);
- experience bottle in the main hand (a thrown item that is no projectile
  class in the reference, and no block);
- stone in the main hand with experience bottles in the off hand, aimed at
  the sky: the main hand places nothing, the off hand throws (the reference's
  Blocks check is satisfied by the main-hand block);
- a placement burst against a wall: stone placed toward the player, timed
  in client ticks between the first and the third placement (repeated when
  a poll skipped ticks and so cannot time a placement).

A first session measures every probe with FastPlace off, then on with
"All"; the home is relaunched with "Blocks" and with "Projectiles". Without
the module the host repeats a held use every 4 ticks; with a matching item
FastPlace (delay 1) repeats it every tick. Results go to result.json and
measurements.json.
"""
import argparse
import json
import os

from movement_scenario import FLOOR, Run, relaunch
from native_session import NativeSession
from smoke_scenario import Checks

BINDS = {'world.fast-place': 'KeyP'}
HOLD = 20
# A held use repeats every 4 ticks without FastPlace (1 click + 20 / 4), every
# tick with it.
SLOW_MAX = 8
FAST_MIN = 12
WALL_Z = 4


class Place(Run):
    def count(self, item):
        return self.call('inventory').get('counts', {}).get(f'minecraft:{item}', 0)

    def clear(self):
        self.call('chat', '/clear @s')
        self.wait(4)

    def offhand(self, item, count):
        self.call('chat', f'/item replace entity @s weapon.offhand with minecraft:{item} {count}')
        for _ in range(20):
            self.wait(4)
            off = self.call('status').get('offhand') or {}
            if off.get('item', '').endswith(item):
                return True
        self.checks.check(f'{item} arrived in the off hand', False)
        return False

    def hold(self, ticks):
        self.call('mouse', 'right', 'down')
        self.wait(ticks)
        self.call('mouse', 'right', 'up')
        self.wait(10)


def sky(r, main, off=None):
    """Items consumed while the use key is held for HOLD ticks at the sky."""
    r.reset()
    r.clear()
    used = off or main
    if off:
        r.give(main, 64)
        r.offhand(off, 64)
    else:
        r.give(main, 64)
    r.call('look', '0', '-60')
    r.wait(4)
    before = r.count(used)
    r.hold(HOLD)
    return before - r.count(used)


def wall_once(r):
    """A stone wall at z=WALL_Z; stone held at eye level toward it. Each poll
    waits one tick; a placement seen at a poll happened after the previous
    poll's tick, so its tick is exact when the poll advanced by one tick."""
    r.fill(-1, FLOOR + 1, WALL_Z, 1, FLOOR + 4, WALL_Z, 'minecraft:stone')
    r.fill(-1, FLOOR + 1, 0, 1, FLOOR + 4, WALL_Z - 1, 'minecraft:air')
    r.reset(z=0.6)
    r.clear()
    r.give('stone', 64)
    r.call('look', '0', '0')
    r.wait(6)
    hit = (r.call('state').get('looking_at') or {}).get('block') or {}
    seen = []
    r.call('mouse', 'right', 'down')
    t0 = prev = r.call('state')['ticks']
    last = 0
    for _ in range(30):
        now = r.call('wait', 1)['ticks']
        placed = r.call('blocks', 0, FLOOR + 2, 1, 0, FLOOR + 2, WALL_Z - 1).get('counts', {}).get(
            'minecraft:stone', 0)
        if placed != last:
            seen.append({'placed': placed, 'tick': now - t0, 'after_tick': prev - t0})
            last = placed
        prev = now
        if placed >= 3:
            break
    r.call('mouse', 'right', 'up')
    r.wait(6)
    first = next((s for s in seen if s['placed'] >= 1), None)
    third = next((s for s in seen if s['placed'] >= 3), None)
    span = third['tick'] - first['tick'] if first and third else None
    exact = bool(first and third) and all(s['tick'] - s['after_tick'] == 1 for s in (first, third))
    r.fill(-1, FLOOR + 1, 0, 1, FLOOR + 4, WALL_Z, 'minecraft:air')
    return {'span': span, 'exact': exact, 'seen': seen, 'hit': hit}


def wall(r, attempts=3):
    """The client ticks between the first and the third placement (None if
    fewer than three). A poll that advanced several ticks (a stalled frame
    catching up) cannot time the placement it saw, so the probe repeats; every
    attempt is recorded."""
    tries = []
    for _ in range(attempts):
        tries.append(wall_once(r))
        if tries[-1]['exact']:
            break
    return {**tries[-1], 'attempts': tries}


def probes(r):
    return {
        'snowball': sky(r, 'snowball'),
        'experience_bottle': sky(r, 'experience_bottle'),
        'stone_main_bottle_off': sky(r, 'stone', 'experience_bottle'),
        'wall': wall(r),
    }


def fast(n):
    return n >= FAST_MIN


def slow(n):
    return 1 <= n <= SLOW_MAX


def expect(r, mode, got, baseline):
    """Checks for one mode against the reference's item classes."""
    want = {
        'all': {'snowball': True, 'experience_bottle': True, 'stone_main_bottle_off': True, 'wall': True},
        'blocks': {'snowball': False, 'experience_bottle': False, 'stone_main_bottle_off': True, 'wall': True},
        'projectiles': {'snowball': True, 'experience_bottle': False, 'stone_main_bottle_off': False,
                        'wall': False},
    }[mode]
    for probe in ('snowball', 'experience_bottle', 'stone_main_bottle_off'):
        n = got[probe]
        ok = fast(n) if want[probe] else slow(n)
        r.checks.check(f'{mode}: {probe} {"repeats every tick" if want[probe] else "keeps the host repeat"}',
                       ok, {'used': n, 'off_baseline': baseline[probe], 'hold_ticks': HOLD})
    span, off_span = got['wall']['span'], baseline['wall']['span']
    if want['wall']:
        ok = span is not None and off_span is not None and span <= 4 and span < off_span
    else:
        ok = span is not None and span >= 6
    ok = ok and got['wall']['exact'] and baseline['wall']['exact']
    r.checks.check(f'{mode}: wall placements {"every tick" if want["wall"] else "at the host repeat"}', ok,
                   {'span_ticks': span, 'off_span_ticks': off_span, 'seen': got['wall']['seen'],
                    'attempts': len(got['wall']['attempts'])})


def setup(r):
    r.call('chat', '/difficulty peaceful')
    r.call('time', '6000')
    r.call('chat', '/gamerule doDaylightCycle false')
    r.wait(20)
    r.arena()


def run(resources, allow_stale):
    with NativeSession(resources, 'fast-place-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()},
                                   'world.fast-place.held-item': 'all', 'world.fast-place.delay': 1}})
        session.launch(creative=True)
        session.call('chat', '/gamemode survival')
        r = Place(session, checks, BINDS)
        setup(r)
        m = {}
        r.toggle('world.fast-place', False)
        m['off'] = baseline = probes(r)
        for probe in ('snowball', 'experience_bottle', 'stone_main_bottle_off'):
            checks.check(f'off: {probe} keeps the host repeat (every 4 ticks)', slow(baseline[probe]),
                         baseline[probe])
        checks.check('off: wall placements at the host repeat',
                     baseline['wall']['exact'] and (baseline['wall']['span'] or 0) >= 6, baseline['wall'])
        checks.check('the wall probe aims at the wall face', baseline['wall']['hit'].get('face') == 'north',
                     baseline['wall']['hit'])
        r.toggle('world.fast-place', True)
        m['all'] = probes(r)
        expect(r, 'all', m['all'], baseline)

        for mode in ('blocks', 'projectiles'):
            relaunch(session, {'world.fast-place.held-item': mode})
            r.wait(10)
            checks.check(f'{mode}: FastPlace still enabled after the relaunch', r.enabled('world.fast-place'),
                         session.saved()['modules'].get('world.fast-place'))
            checks.check(f'{mode}: the Held Item setting was saved',
                         session.saved()['settings'].get('world.fast-place.held-item') == mode,
                         session.saved()['settings'].get('world.fast-place.held-item'))
            m[mode] = probes(r)
            expect(r, mode, m[mode], baseline)

        r.toggle('world.fast-place', False)
        alive = session.call('state')
        checks.check('still in the world at the end (no kick)', alive.get('player') is not None,
                     alive.get('screen'))
        (session.root / 'measurements.json').write_text(json.dumps(m, indent=2, default=str))
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}',
              session.root)
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
