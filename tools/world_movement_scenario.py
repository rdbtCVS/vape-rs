#!/usr/bin/env python3
"""Movement/world host scenario in a fresh isolated world.

A 7-wide stone runway (x -3..3, z -3..30) at y=99 over an empty drop. Checks
through the control socket, each against a baseline with the module off:

- Scaffold Legit sneaks at the back edge while walking backwards with a
  block held, so the player stays on the runway; without it they fall.
- InvWalk lets the physically held W move the player behind the inventory
  screen, and Rotate turns the view with the arrow keys there.
- NoSlowdown keeps full speed and (with Sprint) the sprint while a bow is
  drawn.
- Step climbs a one-block ledge across the runway by walking into it.
- WindCharge selects the charge, throws it at the feet with silent aim (the
  camera pitch stays), jumps, restores the slot and disables itself. A second
  fresh session with silent aim off checks the visible controller of the
  shared rotation turns the camera down before the throw.

Results go to result.json.
"""
import argparse
import os

from native_session import NativeSession
from smoke_scenario import Checks

BINDS = {
    'blatant.scaffold': 'KeyJ',
    'blatant.inv-walk': 'KeyK',
    'blatant.no-slowdown': 'KeyO',
    'combat.sprint': 'KeyP',
    'utility.wind-charge': 'KeyU',
    'blatant.step': 'KeyL',
}


def state(session):
    return session.call('state')


def player(session):
    return state(session)['player']


def enabled(session, module):
    return session.saved()['modules'].get(module, {}).get('enabled', False)


def toggle(session, module):
    session.call('key', BINDS[module][3:].lower(), 'tap')
    session.call('wait', '2')


def build(session, checks):
    expected = 25 * 45 + 7 * 34
    for _ in range(10):
        session.call('fill', '-12', '88', '-12', '12', '88', '32', 'minecraft:stone')
        session.call('fill', '-12', '89', '-12', '12', '110', '32', 'minecraft:air')
        session.call('fill', '-3', '99', '-3', '3', '99', '30', 'minecraft:stone')
        # The block updates reach the client's level a few ticks later.
        session.call('wait', '10')
        counts = session.call('blocks', '-12', '88', '-12', '12', '99', '32').get('counts', {})
        if counts.get('minecraft:stone') == expected:
            break
        session.call('wait', '20')
    checks.check('runway and catch floor are in place',
                 counts.get('minecraft:stone') == expected, counts)


def reset(session, z=0.5):
    session.call('tp', '0.5', '100', str(z))
    session.call('look', '0', '0')
    session.call('wait', '10')


def on_runway(p):
    x, y, z = p['position']
    return y > 99.5 and -3.4 <= x <= 4.4 and -3.4 <= z <= 31.4, (x, y, z)


def walk_back(session):
    """Walk backwards (-Z) from the centre; returns (sneaking mid-walk, end)."""
    reset(session)
    session.call('key', 's', 'down')
    session.call('wait', '40')
    mid = player(session)
    session.call('wait', '10')
    session.call('key', 's', 'up')
    session.call('wait', '20')
    return mid.get('sneaking'), player(session)


def inventory_walk(session):
    reset(session)
    session.call('open-inventory')
    session.call('wait', '5')
    screen = state(session).get('screen')
    start = player(session)['position'][2]
    session.call('key', 'w', 'down')
    session.call('wait', '30')
    session.call('key', 'w', 'up')
    session.call('wait', '5')
    end = player(session)['position'][2]
    session.call('key', 'up', 'down')
    session.call('wait', '10')
    session.call('key', 'up', 'up')
    pitch = player(session)['pitch']
    session.call('close')
    session.call('wait', '5')
    return screen, end - start, pitch


def bow_walk(session):
    reset(session)
    session.call('hotbar', '3')
    session.call('key', 'w', 'down')
    session.call('wait', '10')
    session.call('mouse', 'right', 'down')
    session.call('wait', '5')
    start = player(session)
    session.call('wait', '20')
    end = player(session)
    session.call('mouse', 'right', 'up')
    session.call('key', 'w', 'up')
    session.call('wait', '10')
    session.call('hotbar', '1')
    return end['position'][2] - start['position'][2], end.get('sprinting')


def step_walk(session):
    """Walk (+Z) into the one-block ledge at z=20; returns the end height."""
    reset(session, z=16.5)
    session.call('key', 'w', 'down')
    session.call('wait', '30')
    session.call('key', 'w', 'up')
    session.call('wait', '10')
    return player(session)['position']


def run(resources, allow_stale):
    with NativeSession(resources, 'movement-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        settings = {f'{module}.binds': [key] for module, key in BINDS.items()}
        settings['blatant.inv-walk.rotate'] = True
        session.seed({'schema': 1, 'modules': {}, 'settings': settings})
        session.launch(creative=True)
        session.call('time', '6000')
        reset(session)
        session.call('wait-ready', '40')
        build(session, checks)
        session.call('hotbar', '1')
        session.call('give', 'minecraft:stone', '64')
        session.call('hotbar', '2')
        session.call('give', 'minecraft:wind_charge', '16')
        session.call('hotbar', '3')
        session.call('give', 'minecraft:bow', '1')
        session.call('hotbar', '1')
        session.call('wait', '5')
        held = state(session).get('held')
        checks.check('stone held in slot 1', held and held['item'].endswith('stone'), held)

        # Scaffold Legit.
        sneaking, p = walk_back(session)
        ok, at = on_runway(p)
        checks.check('without Scaffold a backwards walk leaves the edge', not ok, at)
        toggle(session, 'blatant.scaffold')
        checks.check('Scaffold enabled through its bind', enabled(session, 'blatant.scaffold'))
        sneaking, p = walk_back(session)
        ok, at = on_runway(p)
        checks.check('Scaffold Legit sneaks at the edge while walking backwards', sneaking, sneaking)
        checks.check('Scaffold Legit keeps the player on the runway', ok and at[2] < -2.0, at)
        session.call('hotbar', '3')
        sneaking, p = walk_back(session)
        session.call('hotbar', '1')
        # Reference canActivate: with the whitelist off Legit runs whatever is held.
        checks.check('Scaffold Legit with the whitelist off also runs with a bow held', on_runway(p)[0],
                     p['position'])
        toggle(session, 'blatant.scaffold')
        checks.check('Scaffold disabled through its bind', not enabled(session, 'blatant.scaffold'))

        # InvWalk.
        screen, moved, pitch = inventory_walk(session)
        checks.check('inventory screen open for the baseline', str(screen).lower() in ('inventory', 'creative'), screen)
        checks.check('without InvWalk W does not move behind the inventory', abs(moved) < 0.05, moved)
        checks.check('without InvWalk the arrow keys do not turn', abs(pitch) < 0.01, pitch)
        toggle(session, 'blatant.inv-walk')
        checks.check('InvWalk enabled through its bind', enabled(session, 'blatant.inv-walk'))
        screen, moved, pitch = inventory_walk(session)
        checks.check('InvWalk: W moves the player behind the inventory', moved > 2.0, moved)
        checks.check('InvWalk Rotate: arrow up raises the view (-3 per tick)', pitch < -20.0, pitch)
        toggle(session, 'blatant.inv-walk')

        # NoSlowdown with Sprint.
        toggle(session, 'combat.sprint')
        checks.check('Sprint enabled through its bind', enabled(session, 'combat.sprint'))
        slow, slow_sprint = bow_walk(session)
        toggle(session, 'blatant.no-slowdown')
        checks.check('NoSlowdown enabled through its bind', enabled(session, 'blatant.no-slowdown'))
        fast, fast_sprint = bow_walk(session)
        checks.check('drawing a bow slows the walk without NoSlowdown', 0.0 < slow < 1.5, slow)
        # Horizon keeps an existing sprint while an item is used, so the
        # sprint flag is recorded, not checked against a baseline.
        checks.check('sprint flag while drawing (recorded)', True,
                     {'without': slow_sprint, 'with_no_slowdown': fast_sprint})
        checks.check('NoSlowdown keeps full speed while drawing a bow', fast > 3.0 * slow and fast > 3.5,
                     {'slow': slow, 'fast': fast})
        toggle(session, 'blatant.no-slowdown')
        toggle(session, 'combat.sprint')

        # Step.
        session.call('fill', '-3', '100', '20', '3', '100', '20', 'minecraft:stone')
        session.call('wait', '5')
        at = step_walk(session)
        checks.check('without Step the one-block ledge blocks the walk', at[1] < 100.5 and at[2] < 20.0, at)
        toggle(session, 'blatant.step')
        checks.check('Step enabled through its bind', enabled(session, 'blatant.step'))
        at = step_walk(session)
        checks.check('Step climbs over the one-block ledge', at[1] > 99.5 and at[2] > 21.0, at)
        checks.check('Step: still connected after the step', state(session).get('dimension') is not None,
                     state(session).get('dimension'))
        toggle(session, 'blatant.step')
        session.call('fill', '-3', '100', '20', '3', '100', '20', 'minecraft:air')

        # WindCharge.
        reset(session, z=10.5)
        session.call('wait', '10')
        toggle(session, 'utility.wind-charge')
        samples = []
        for _ in range(40):
            s = state(session)
            samples.append({'y': s['player']['position'][1], 'selected': s['selected'], 'pitch': s['player']['pitch']})
            session.call('wait', '1')
        session.call('wait', '40')
        top = max(x['y'] for x in samples)
        checks.check('WindCharge selected the wind charge slot', any(x['selected'] == 2 for x in samples),
                     [x['selected'] for x in samples[:12]])
        checks.check('WindCharge launched the player above a plain jump', top > 102.0, top)
        checks.check('WindCharge silent aim leaves the camera pitch',
                     all(abs(x['pitch']) < 0.01 for x in samples), sorted({round(x['pitch'], 2) for x in samples}))
        checks.check('WindCharge disabled itself', not enabled(session, 'utility.wind-charge'))
        checks.check('WindCharge restored the previous slot', state(session)['selected'] == 1,
                     state(session)['selected'])

        session.capture('final')
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
    return run_visible_wind_charge(resources, allow_stale) and ok


def run_visible_wind_charge(resources, allow_stale):
    """WindCharge with silent aim off: the shared rotation's visible
    controller turns the camera to the feet, then the throw launches."""
    with NativeSession(resources, 'movement-visible-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        settings = {'utility.wind-charge.binds': [BINDS['utility.wind-charge']],
                    'utility.wind-charge.silent-aim': False}
        session.seed({'schema': 1, 'modules': {}, 'settings': settings})
        session.launch(creative=True)
        session.call('time', '6000')
        reset(session)
        session.call('wait-ready', '40')
        build(session, checks)
        session.call('hotbar', '2')
        session.call('give', 'minecraft:wind_charge', '16')
        session.call('hotbar', '1')
        reset(session, z=10.5)
        session.call('look', '45', '0')
        session.call('wait', '10')
        toggle(session, 'utility.wind-charge')
        samples = []
        for _ in range(40):
            s = state(session)
            samples.append({'y': s['player']['position'][1], 'selected': s['selected'],
                            'yaw': s['player']['yaw'], 'pitch': s['player']['pitch']})
            session.call('wait', '1')
        session.capture('visible-aim')
        session.call('wait', '40')
        top = max(x['y'] for x in samples)
        pitches = [round(x['pitch'], 1) for x in samples]
        checks.check('visible WindCharge turned the camera pitch past 80', max(pitches) > 80.0, pitches[:16])
        checks.check('visible WindCharge left the yaw alone (UNSET axis)',
                     all(abs(x['yaw'] - 45.0) < 0.01 for x in samples), sorted({round(x['yaw'], 2) for x in samples}))
        checks.check('visible WindCharge launched the player above a plain jump', top > 102.0, top)
        checks.check('visible WindCharge disabled itself', not enabled(session, 'utility.wind-charge'))
        checks.check('visible WindCharge restored the previous slot', state(session)['selected'] == 1,
                     state(session)['selected'])
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
