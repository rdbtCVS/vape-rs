#!/usr/bin/env python3
"""Health and Reach Display host scenario in a fresh isolated world.

Health: the centre readout at full health, after /damage (yellow, red) and
with absorption (gold heart). Reach Display: two hits on a stationary zombie
from known distances, then the 5 s reset. Readouts are checked through
screenshots, the inputs through the player's health and the zombie's health.
Results go to result.json.
"""
import argparse
import math
import os

from native_session import NativeSession
from smoke_scenario import Checks
from target_info_scenario import health as zombie_health, hit, prepare, summon


def player_health(session):
    return session.call('state')['player']['health']


def run(resources, allow_stale):
    with NativeSession(resources, 'hud-readouts-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {'render.health': {'enabled': True},
                                               'hud.reach-display': {'enabled': True}},
                      'settings': {}})
        session.launch(creative=False)
        session.call('command', 'gamerule natural_health_regeneration false')
        prepare(session, checks)
        session.call('tp', '0.5', '100', '0.5')
        session.call('look', '0', '10')
        session.call('wait', '10')
        checks.check('full health', player_health(session) == 20.0, player_health(session))
        session.capture('health-full-reach-zero')

        session.call('command', 'damage @s 7')
        session.call('wait', '10')
        checks.check('13 hp after 7 damage', player_health(session) == 13.0, player_health(session))
        session.capture('health-yellow-6.5')
        session.call('wait', '20')
        session.call('command', 'damage @s 6')
        session.call('wait', '10')
        checks.check('7 hp after 6 more', player_health(session) == 7.0, player_health(session))
        session.capture('health-red-3.5')
        session.call('command', 'effect give @s minecraft:absorption 60 1')
        session.call('wait', '10')
        session.capture('health-absorption-gold-7.5')

        summon(session, checks)
        session.call('give', 'minecraft:diamond_sword', '1')
        session.call('hotbar', '1')
        for z, label in ((0.5, 'reach-1.73'), (-0.3, 'reach-2.54')):
            session.call('tp', '0.5', '100', str(z))
            session.call('look', '0', '10')
            session.call('wait', '12')
            before = zombie_health(session)
            hit(session)
            after = zombie_health(session)
            expected = (2.2 - z) / math.cos(math.radians(10))
            checks.check(f'hit from z={z} lands (expected {expected:.2f} blocks)',
                         before is not None and after is not None and after < before,
                         [before, after])
            session.capture(label)
        session.call('wait', '110')  # over the 5 s reset
        session.capture('reach-reset-zero')
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
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
