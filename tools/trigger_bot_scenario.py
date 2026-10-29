#!/usr/bin/env python3
"""TriggerBot host scenario in a fresh isolated world.

A stationary (NoAI) zombie stands two blocks in front of the player. Checks,
through the control socket and the server's mob health, that TriggerBot hits
it only when it should: not while disabled, when the crosshair is on it with
Mobs targeting on, not when looking away, not behind a screen, not with Mobs
off, and not after disabling. Results go to result.json.
"""
import argparse
import json

from native_session import NativeSession
from smoke_scenario import Checks

BIND = 'KeyB'
TARGET = (0.5, 100.0, 2.5)


def zombie(session):
    near = session.call('mobs', '16').get('near', [])
    return next((m for m in near if m.get('kind') == 'minecraft:zombie'), None)


def health(session):
    z = zombie(session)
    return z['health'] if z else None


def enabled(session):
    return session.saved()['modules'].get('combat.trigger-bot', {}).get('enabled', False)


def toggle(session):
    session.call('key', BIND[3:].lower(), 'tap')
    session.call('wait', '2')


def aim(session, yaw=0.0):
    session.call('tp', '0.5', '100', '0.5')
    session.call('look', str(yaw), '10')
    session.call('wait', '4')


def restart(session, changes):
    session.call('quit')
    session.process.wait(timeout=15)
    session.log.close()
    session.process = None
    document = session.saved()
    document['settings'].update(changes)
    session.seed(document)
    session.launch(creative=True)
    session.call('wait-ready', '40')


def run(resources, allow_stale):
    with NativeSession(resources, 'trigger-bot-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {'combat.trigger-bot.binds': [BIND],
                                   'combat.trigger-bot.targets.mobs': True}})
        session.launch(creative=True)
        session.call('time', '6000')
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        for _ in range(10):
            session.call('fill', '-4', '99', '-4', '4', '99', '6', 'minecraft:stone')
            session.call('fill', '-4', '100', '-4', '4', '104', '6', 'minecraft:air')
            counts = session.call('blocks', '-4', '99', '-4', '4', '99', '6').get('counts', {})
            if counts.get('minecraft:stone') == 9 * 11:
                break
            session.call('wait', '20')
        checks.check('floor is in place', counts.get('minecraft:stone') == 9 * 11, counts)
        session.call('chat', f'/summon minecraft:zombie {TARGET[0]} {TARGET[1]} {TARGET[2]} '
                     '{NoAI:1b,PersistenceRequired:1b}')
        for _ in range(10):
            session.call('wait', '10')
            if zombie(session):
                break
        checks.check('a stationary zombie stands in front', zombie(session) is not None,
                     zombie(session))
        aim(session)
        target = session.call('state').get('looking_at')
        checks.check('the crosshair is on the zombie', 'zombie' in json.dumps(target), target)

        start = health(session)
        session.call('wait', '40')
        checks.check('disabled TriggerBot does not attack', health(session) == start,
                     [start, health(session)])

        toggle(session)
        checks.check('TriggerBot enabled through its bind', enabled(session))
        session.call('wait', '60')
        hit = health(session)
        # A fist recharges in 5 ticks and the zombie is invulnerable for 10
        # after a hit, so 60 ticks allow several landed hits (0.94 each).
        checks.check('TriggerBot hits the zombie repeatedly as the cooldown recharges',
                     hit is not None and start - hit >= 2.5, [start, hit])

        aim(session, 180.0)
        before = health(session)
        session.call('wait', '40')
        checks.check('no attack while looking away', health(session) == before,
                     [before, health(session)])

        aim(session)
        session.call('key', 'e', 'tap')
        session.call('wait', '4')
        before = health(session)
        session.call('wait', '40')
        checks.check('no attack behind the inventory screen', health(session) == before,
                     {'screen': session.call('status').get('screen'), 'health': [before, health(session)]})
        session.call('key', 'escape', 'tap')
        session.call('wait', '4')

        toggle(session)
        checks.check('TriggerBot disabled through its bind', not enabled(session))
        aim(session)
        before = health(session)
        session.call('wait', '40')
        checks.check('disabled TriggerBot stops attacking', health(session) == before,
                     [before, health(session)])

        restart(session, {'combat.trigger-bot.targets.mobs': False})
        session.call('key', BIND[3:].lower(), 'tap')
        session.call('wait', '2')
        checks.check('TriggerBot re-enabled after restart', enabled(session))
        aim(session)
        before = health(session)
        session.call('wait', '40')
        checks.check('Mobs off: the zombie is not attacked', before is not None and health(session) == before,
                     [before, health(session)])
        session.capture('final')
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?',
                        default='<launcher-dir>/versions/'
                                'nightly-2026-10-28-9b772b2/.local/horizon-client/26.3')
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    raise SystemExit(0 if run(args.resources, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
