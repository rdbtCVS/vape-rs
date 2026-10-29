#!/usr/bin/env python3
"""AimAssist host scenario in a fresh isolated world.

Summons a stationary (NoAI) zombie beside the player and checks, through the
control socket, that AimAssist turns the real host view toward it only when it
should: not while disabled, toward the target when enabled, not without the
attack button when "Require mouse down" is on, not behind a screen, not for a
mob when Mobs targeting is off, and that disabling stops the turn. Results go
to result.json in the session directory.
"""
import argparse
import json
import math

from native_session import NativeSession
from smoke_scenario import Checks

BIND = 'KeyJ'
TARGET = (3.5, 100.0, 4.5)
# Minecraft yaw toward the target from (0.5, 0.5): -atan2(dx, dz).
WANT = -math.degrees(math.atan2(TARGET[0] - 0.5, TARGET[2] - 0.5))


def yaw(session):
    return session.call('state')['player']['yaw']


def off(session):
    """Signed degrees between the view and the target direction."""
    return (yaw(session) - WANT + 180.0) % 360.0 - 180.0


def toggle(session):
    session.call('key', BIND[3:].lower(), 'tap')
    session.call('wait', '2')


def enabled(session):
    return session.saved()['modules'].get('combat.aim-assist', {}).get('enabled', False)


def settle(session, ticks=40):
    """Let frames run with real time passing (AimAssist steps per millisecond)."""
    session.call('wait', str(ticks))


def run(resources, in_frame, allow_stale):
    prefix = 'aim-assist-inframe-' if in_frame else 'aim-assist-'
    with NativeSession(resources, prefix, in_frame, allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({
            'schema': 1,
            'modules': {},
            'settings': {
                'combat.aim-assist.binds': [BIND],
                'combat.aim-assist.require-mouse': False,
                'combat.aim-assist.targets.mobs': True,
                'combat.aim-assist.distance': 8.0,
            },
        })
        session.launch(creative=True)
        session.call('time', '6000')
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        for _ in range(10):
            session.call('fill', '-4', '99', '-4', '8', '99', '10', 'minecraft:stone')
            session.call('fill', '-4', '100', '-4', '8', '104', '10', 'minecraft:air')
            counts = session.call('blocks', '-4', '99', '-4', '8', '99', '10').get('counts', {})
            if counts.get('minecraft:stone') == 13 * 15:
                break
            session.call('wait', '20')
        checks.check('scenario floor is in place', counts.get('minecraft:stone') == 13 * 15, counts)
        session.call('chat', f'/summon minecraft:zombie {TARGET[0]} {TARGET[1]} {TARGET[2]} '
                     '{NoAI:1b,PersistenceRequired:1b}')
        for _ in range(10):
            session.call('wait', '10')
            mobs = session.call('mobs', '16')
            if 'zombie' in json.dumps(mobs):
                break
        checks.check('a stationary zombie stands beside the player',
                     'zombie' in json.dumps(mobs), mobs)
        session.call('tp', '0.5', '100', '0.5')
        session.call('look', '0', '0')
        settle(session, 20)

        start = off(session)
        checks.check('view starts away from the target', abs(start) > 30.0, start)
        settle(session)
        checks.check('disabled AimAssist leaves the view alone', abs(off(session) - start) < 0.01,
                     off(session))

        toggle(session)
        checks.check('AimAssist enabled through its bind', enabled(session))
        best = abs(start)
        for _ in range(12):
            settle(session, 10)
            best = min(best, abs(off(session)))
        checks.check('AimAssist turned the host view toward the zombie', best < 10.0,
                     {'start': start, 'closest': best, 'want': WANT})

        # Screens stop the turn.
        session.call('look', '0', '0')
        session.call('key', 'e', 'tap')
        session.call('wait', '2')
        screen = session.call('status').get('screen')
        before = off(session)
        settle(session)
        checks.check('no turn behind the inventory screen',
                     abs(off(session) - before) < 0.01, {'screen': screen, 'off': off(session)})
        session.call('key', 'escape', 'tap')
        session.call('wait', '4')

        # Disabling stops the turn.
        toggle(session)
        checks.check('AimAssist disabled through its bind', not enabled(session))
        session.call('look', '0', '0')
        settle(session, 4)
        before = off(session)
        settle(session)
        checks.check('disabled AimAssist stops turning', abs(off(session) - before) < 0.01,
                     off(session))

        # Require mouse down (reference default): no turn without attack held.
        session.call('quit')
        session.process.wait(timeout=15)
        session.log.close()
        session.process = None
        document = session.saved()
        document['settings']['combat.aim-assist.require-mouse'] = True
        document['modules']['combat.aim-assist'] = {**document['modules'].get('combat.aim-assist', {}),
                                                    'enabled': True}
        session.seed(document)
        session.launch(creative=True)
        session.call('tp', '0.5', '100', '0.5')
        session.call('look', '0', '0')
        settle(session, 20)
        mobs = session.call('mobs', '16')
        checks.check('zombie persisted across the restart', 'zombie' in json.dumps(mobs))
        checks.check('AimAssist restored enabled from the profile', enabled(session))
        before = off(session)
        settle(session)
        checks.check('require-mouse: no turn without the attack button',
                     abs(off(session) - before) < 0.01, off(session))
        session.call('mouse', 'left', 'down')
        best = abs(before)
        for _ in range(12):
            settle(session, 10)
            best = min(best, abs(off(session)))
        session.call('mouse', 'left', 'up')
        checks.check('require-mouse: turns while attack is held', best < abs(before) - 10.0,
                     {'start': before, 'closest': best})

        # Mobs targeting off: the zombie is no longer a target.
        session.call('quit')
        session.process.wait(timeout=15)
        session.log.close()
        session.process = None
        document = session.saved()
        document['settings']['combat.aim-assist.require-mouse'] = False
        document['settings']['combat.aim-assist.targets.mobs'] = False
        session.seed(document)
        session.launch(creative=True)
        session.call('tp', '0.5', '100', '0.5')
        session.call('look', '0', '0')
        settle(session, 20)
        before = off(session)
        settle(session)
        checks.check('Mobs off: the zombie is not targeted', abs(off(session) - before) < 0.01,
                     off(session))
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
