#!/usr/bin/env python3
"""KillAura and SilentAura host scenario in a fresh isolated world.

A stationary (NoAI) zombie stands about two blocks from the player. Checks,
through the control socket and the server's mob health, that KillAura hits
it without the crosshair on it while it is inside the max angle, not outside
it, not behind a screen and not once disabled; and that SilentAura hits it
through its managed rotation while the view stays where it was, and stops
once disabled. Results go to result.json.
"""
import argparse
import json
import os

from native_session import NativeSession
from smoke_scenario import Checks

KILL = ('combat.kill-aura', 'KeyK')
SILENT = ('combat.silent-aura', 'KeyJ')
TARGET = (0.9, 100.0, 2.5)


def zombie(session):
    near = session.call('mobs', '16').get('near', [])
    return next((m for m in near if m.get('kind') == 'minecraft:zombie'), None)


def health(session):
    z = zombie(session)
    return z['health'] if z else None


def enabled(session, module):
    return session.saved()['modules'].get(module[0], {}).get('enabled', False)


def toggle(session, module):
    session.call('key', module[1][3:].lower(), 'tap')
    session.call('wait', '2')


def aim(session, yaw):
    session.call('tp', '0.5', '100', '0.5')
    session.call('look', str(yaw), '10')
    session.call('wait', '4')


def view(session):
    p = session.call('state').get('player') or {}
    return [p.get('yaw'), p.get('pitch')]


def damage_over(session, ticks):
    before = health(session)
    session.call('wait', str(ticks))
    after = health(session)
    return before, after


def run(resources, allow_stale):
    with NativeSession(resources, 'aura-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        settings = {}
        for module, bind in (KILL, SILENT):
            settings[f'{module}.binds'] = [bind]
            settings[f'{module}.targets.mobs'] = True
        session.seed({'schema': 1, 'modules': {}, 'settings': settings})
        session.launch(creative=True)
        session.call('chat', '/gamerule spawn_mobs false')
        session.call('time', '18000')
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
                     '{NoAI:1b,PersistenceRequired:1b,Health:1000f,'
                     'attributes:[{id:"minecraft:max_health",base:1000}]}')
        found = None
        for _ in range(20):
            session.call('wait', '10')
            found = zombie(session)
            if found:
                break
        checks.check('a stationary zombie stands nearby', found is not None,
                     found and {'id': found.get('id'), 'health': found.get('health')})

        # About 41 degrees off: the crosshair misses, KillAura's 90 degree cone (45 each side) does not.
        aim(session, 30.0)
        target = session.call('state').get('looking_at')
        checks.check('the crosshair is off the zombie', 'zombie' not in json.dumps(target), target)
        before, after = damage_over(session, 40)
        checks.check('disabled KillAura does not attack', before == after, [before, after])

        toggle(session, KILL)
        checks.check('KillAura enabled through its bind', enabled(session, KILL))
        before, after = damage_over(session, 60)
        checks.check('KillAura hits the zombie off-crosshair inside its max angle',
                     before is not None and after is not None and before - after >= 1.5, [before, after])

        aim(session, 180.0)
        before, after = damage_over(session, 40)
        checks.check('KillAura: no attack outside the max angle', before == after, [before, after])

        aim(session, 30.0)
        session.call('key', 'e', 'tap')
        session.call('wait', '4')
        before, after = damage_over(session, 40)
        checks.check('KillAura: no attack behind the inventory screen (GUI check)', before == after,
                     {'screen': session.call('status').get('screen'), 'health': [before, after]})
        session.call('key', 'escape', 'tap')
        session.call('wait', '4')

        toggle(session, KILL)
        checks.check('KillAura disabled through its bind', not enabled(session, KILL))
        aim(session, 30.0)
        before, after = damage_over(session, 40)
        checks.check('disabled KillAura stops attacking', before == after, [before, after])

        # SilentAura: the view stays about 51 degrees off (inside 60 each side); the managed rotation aims.
        aim(session, 40.0)
        start_view = view(session)
        toggle(session, SILENT)
        checks.check('SilentAura enabled through its bind', enabled(session, SILENT))
        before, after = damage_over(session, 80)
        # Fist hits at partial strength: two or more landed hits.
        checks.check('SilentAura hits the zombie repeatedly through its managed rotation',
                     before is not None and after is not None and before - after >= 1.5, [before, after])
        end_view = view(session)
        checks.check('SilentAura leaves the view where it was',
                     all(a is not None and b is not None and abs(a - b) < 1e-3
                         for a, b in zip(start_view, end_view)), [start_view, end_view])

        aim(session, 180.0)
        before, after = damage_over(session, 40)
        checks.check('SilentAura: no attack outside its 120 degree max angle', before == after,
                     [before, after])

        toggle(session, SILENT)
        checks.check('SilentAura disabled through its bind', not enabled(session, SILENT))
        aim(session, 40.0)
        session.call('wait', '10')
        before, after = damage_over(session, 40)
        checks.check('disabled SilentAura stops attacking', before == after, [before, after])
        session.capture('final')
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?',
                        default=os.environ.get('HORIZON_RESOURCES'),
                        help='prepared Minecraft 26.3 resources (or set HORIZON_RESOURCES)')
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run(args.resources, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
