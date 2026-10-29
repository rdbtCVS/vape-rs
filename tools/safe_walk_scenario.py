#!/usr/bin/env python3
"""SafeWalk host scenario in a fresh isolated world.

A 7x7 stone platform at y=99 stands over an empty drop. Checks, through the
control socket, that without SafeWalk the player walks off the edge; with it
the host's edge back-off holds the player on the platform when strafing;
Direction Check lets a straight forward walk leave the edge; with Direction
Check off a forward walk is held too; and SafeWalk never crouches the player
(no sneak key, no crouch slowdown). Results go to result.json.
"""
import argparse

from native_session import NativeSession
from smoke_scenario import Checks

BIND = 'KeyN'


def player(session):
    return session.call('state')['player']


def enabled(session):
    return session.saved()['modules'].get('world.safe-walk', {}).get('enabled', False)


def toggle(session):
    session.call('key', BIND[3:].lower(), 'tap')
    session.call('wait', '2')


def build(session, checks):
    for _ in range(10):
        session.call('fill', '-12', '88', '-12', '12', '88', '12', 'minecraft:stone')
        session.call('fill', '-12', '89', '-12', '12', '110', '12', 'minecraft:air')
        session.call('fill', '-3', '99', '-3', '3', '99', '3', 'minecraft:stone')
        counts = session.call('blocks', '-12', '88', '-12', '12', '99', '12').get('counts', {})
        if counts.get('minecraft:stone') == 25 * 25 + 49:
            break
        session.call('wait', '20')
    checks.check('platform and catch floor are in place',
                 counts.get('minecraft:stone') == 25 * 25 + 49, counts)


def walk(session, key, ticks=50):
    """Reset to the platform centre facing +Z, hold `key`, return the player."""
    session.call('tp', '0.5', '100', '0.5')
    session.call('look', '0', '0')
    session.call('wait', '10')
    session.call('key', key, 'down')
    session.call('wait', str(ticks))
    session.call('key', key, 'up')
    session.call('wait', '20')
    p = player(session)
    return p


def on_platform(p):
    x, y, z = p['position']
    return y > 99.5 and -3.4 <= x <= 4.4 and -3.4 <= z <= 4.4, (x, y, z)


def run(resources, in_frame, allow_stale):
    prefix = 'safe-walk-inframe-' if in_frame else 'safe-walk-'
    with NativeSession(resources, prefix, in_frame, allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {'world.safe-walk.binds': [BIND]}})
        session.launch(creative=True)
        session.call('time', '6000')
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        build(session, checks)

        ok, at = on_platform(walk(session, 'a'))
        checks.check('without SafeWalk a strafe walks off the edge', not ok, at)

        toggle(session)
        checks.check('SafeWalk enabled through its bind', enabled(session))
        ok, at = on_platform(walk(session, 'a'))
        checks.check('SafeWalk holds a strafing player at the edge', ok, at)
        checks.check('held at the edge, not stopped early', at[0] > 3.5, at)
        ok, at = on_platform(walk(session, 'd'))
        checks.check('SafeWalk holds at the opposite edge', ok and at[0] < -2.5, at)
        session.call('key', 'a', 'down')
        session.call('wait', '4')
        keys = session.call('status').get('keys', [])
        p = player(session)
        session.call('key', 'a', 'up')
        checks.check('SafeWalk does not press sneak', 'ShiftLeft' not in keys, keys)
        eye = p['eye'][1] - p['position'][1]
        checks.check('SafeWalk does not crouch the player (standing eye height)',
                     not p.get('sneaking') and eye > 1.5, eye)
        ok, at = on_platform(walk(session, 'w'))
        checks.check('Direction Check lets a straight forward walk leave the edge', not ok, at)

        toggle(session)
        checks.check('SafeWalk disabled through its bind', not enabled(session))
        ok, at = on_platform(walk(session, 'a'))
        checks.check('disabled SafeWalk no longer holds the edge', not ok, at)

        session.call('quit')
        session.process.wait(timeout=15)
        session.log.close()
        session.process = None
        document = session.saved()
        document['settings']['world.safe-walk.direction-check'] = False
        document['modules']['world.safe-walk'] = {**document['modules'].get('world.safe-walk', {}),
                                                  'enabled': True}
        session.seed(document)
        session.launch(creative=True)
        session.call('wait-ready', '40')
        checks.check('SafeWalk restored enabled from the profile', enabled(session))
        ok, at = on_platform(walk(session, 'w'))
        checks.check('Direction Check off: a forward walk is held too', ok and at[1] > 99.5, at)
        session.capture('final')
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?',
                        default='<launcher-dir>/versions/'
                                'nightly-2026-10-28-9b772b2/.local/horizon-client/26.3')
    parser.add_argument('--in-frame', action='store_true')
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    raise SystemExit(0 if run(args.resources, args.in_frame, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
