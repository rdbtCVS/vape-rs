#!/usr/bin/env python3
"""Target Info HUD host scenario in a fresh isolated world.

A stationary (NoAI) zombie stands two blocks in front of the player. The
scenario hovers it, hits it with a diamond sword, looks away until the
target times out, opens the GUI preview, then restarts with Show Hovered off
and checks only an attack selects the target. The panel is checked through
screenshots; landed hits through the zombie's health. Results go to
result.json.
"""
import argparse
import os

from native_session import NativeSession
from smoke_scenario import Checks

TARGET = (0.5, 100.0, 2.5)


def zombie(session):
    near = session.call('mobs', '16').get('near', [])
    return next((m for m in near if m.get('kind') == 'minecraft:zombie'), None)


def health(session):
    z = zombie(session)
    return z['health'] if z else None


def aim(session, yaw=0.0):
    session.call('tp', '0.5', '100', '0.5')
    session.call('look', str(yaw), '10')
    session.call('wait', '4')


def hit(session):
    session.call('mouse', 'left', 'down')
    session.call('wait', '1')
    session.call('mouse', 'left', 'up')
    session.call('wait', '6')


def prepare(session, checks):
    session.call('command', 'gamerule spawn_mobs false')
    session.call('time', '18000')
    session.call('tp', '0.5', '100', '0.5')
    session.call('wait-ready', '40')
    counts = {}
    for _ in range(10):
        session.call('fill', '-4', '99', '-4', '4', '99', '6', 'minecraft:stone')
        session.call('fill', '-4', '100', '-4', '4', '104', '6', 'minecraft:air')
        counts = session.call('blocks', '-4', '99', '-4', '4', '99', '6').get('counts', {})
        if counts.get('minecraft:stone') == 9 * 11:
            break
        session.call('wait', '20')
    checks.check('floor is in place', counts.get('minecraft:stone') == 9 * 11, counts)


def summon(session, checks):
    session.call('command', f'summon minecraft:zombie {TARGET[0]} {TARGET[1]} {TARGET[2]} '
                 '{NoAI:1b,PersistenceRequired:1b}')
    for _ in range(10):
        session.call('wait', '10')
        if zombie(session):
            break
    checks.check('a stationary zombie stands in front', zombie(session) is not None,
                 zombie(session))


def run(resources, allow_stale):
    with NativeSession(resources, 'hud-target-info-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {'hud.target-info': {'enabled': True}},
                      'settings': {'hud.target-info.hits-comparator': True,
                                   'hud.target-info.x': 300, 'hud.target-info.y': 200}})
        session.launch(creative=False)
        prepare(session, checks)
        summon(session, checks)
        session.call('give', 'minecraft:diamond_sword', '1')
        session.call('hotbar', '1')
        aim(session)
        session.capture('hovered')

        start = health(session)
        hit(session)
        first = health(session)
        checks.check('the first hit lands', start is not None and first is not None and first < start,
                     [start, first])
        session.capture('after-one-hit')
        session.call('wait', '10')  # invulnerability after a hit
        hit(session)
        second = health(session)
        checks.check('the second hit lands', second is not None and second < first, [first, second])
        session.capture('after-two-hits')

        aim(session, 180.0)
        session.call('wait', '10')
        session.capture('looked-away-still-shown')
        session.call('wait', '70')  # over the 3 s timeout
        session.capture('timed-out')

        session.call('key', 'rshift', 'tap')
        session.capture('gui-preview')
        session.call('key', 'escape', 'tap')

        session.close()
        document = session.saved()
        document['settings']['hud.target-info.show-hovered'] = False
        document['settings']['hud.target-info.damage-comparator'] = False
        document['settings']['hud.target-info.combo-counter'] = False
        document['settings']['hud.target-info.hits-comparator'] = False
        session.seed(document)
        session.launch(creative=False)
        session.call('wait-ready', '40')
        session.call('give', 'minecraft:diamond_sword', '1')
        session.call('hotbar', '1')
        checks.check('Show Hovered off persisted',
                     session.saved()['settings'].get('hud.target-info.show-hovered') is False)
        aim(session)
        checks.check('the zombie is still there', zombie(session) is not None)
        session.capture('hover-off-no-panel')
        before = health(session)
        hit(session)
        after = health(session)
        checks.check('an attack lands with hover off', before is not None and after is not None
                     and after < before, [before, after])
        session.capture('attack-selects-no-stats')
        session.call('wait', '10')
        for _ in range(8):
            if (health(session) or 0) <= 0:
                break
            hit(session)
            session.call('wait', '10')
        checks.check('the zombie was killed', (health(session) or 0) <= 0, health(session))
        session.capture('target-killed')
        session.call('wait', '30')  # dead targets clear after 1 s
        session.capture('killed-cleared')
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
