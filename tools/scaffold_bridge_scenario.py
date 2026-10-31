#!/usr/bin/env python3
"""Scaffold GodBridge and TellyBridge in fresh isolated worlds.

A 3-wide stone runway (x -1..1, z -3..20) at y=99 over an empty drop with a
catch floor at y=88. The player faces +Z and walks backwards (-Z) off the
runway end with a stack of stone, as the reference's manual activation
expects:

- GodBridge sneaks at the edge and places nothing until two blocks are
  placed by hand on the edge's side face ("Activation blocks" 2); then it
  bridges on its own while S is held: the stone appears beyond the runway
  and the player never drops below the runway's top.
- Sprint's movement-input lock (reference `Sprint.onPreEntityUpdate`):
  enabling Sprint while the bridge holds the lock does not press the sprint
  key; after S is released the bridge lets go and Sprint presses it, and a
  forward walk sprints faster than the walk without it.
- TellyBridge (second session, "Require right click" off so the held use
  key does not place on its own) sprint-jumps along the line after the same
  manual activation, places the blocks behind it and towers by "Y increase".

Results go to result.json in each session root.
"""
import argparse
import math
import os

from native_session import NativeSession
from smoke_scenario import Checks, poll

BINDS = {'blatant.scaffold': 'KeyJ', 'combat.sprint': 'KeyP'}
FLOOR = (-12, 88, -40, 12, 88, 12)
RUNWAY = (-1, 99, -3, 1, 99, 20)


def state(session):
    return session.call('state')


def player(session):
    return state(session)['player']


def enabled(session, module):
    return session.saved()['modules'].get(module, {}).get('enabled', False)


def toggle(session, module):
    session.call('key', BINDS[module][3:].lower(), 'tap')
    session.call('wait', '2')


def volume(box):
    return math.prod(box[i + 3] - box[i] + 1 for i in range(3))


def stone(session, x1, y1, z1, x2, y2, z2):
    r = session.call('blocks', *map(str, (x1, y1, z1, x2, y2, z2)))
    return r.get('counts', {}).get('minecraft:stone', 0), r.get('blocks', [])


def build(session, checks):
    expected = {'floor': volume(FLOOR), 'runway': volume(RUNWAY), 'between': 0}
    counts = {}
    for _ in range(10):
        session.call('fill', *map(str, FLOOR), 'minecraft:stone')
        session.call('fill', '-12', '89', '-40', '12', '112', '-14', 'minecraft:air')
        session.call('fill', '-12', '89', '-13', '12', '112', '12', 'minecraft:air')
        session.call('fill', *map(str, RUNWAY), 'minecraft:stone')
        session.call('wait', '10')
        counts = {'floor': stone(session, *FLOOR)[0], 'runway': stone(session, *RUNWAY)[0],
                  # Nothing in the drop between the floor and the runway.
                  'between': stone(session, -12, 89, -40, 12, 98, 12)[0]}
        if counts == expected:
            break
        session.call('wait', '20')
    checks.check('runway and catch floor are in place', counts == expected, {'counts': counts, 'expected': expected})


def reset(session, z=0.5, pitch=0):
    session.call('key', 's', 'up')
    session.call('key', 'w', 'up')
    session.call('tp', '0.5', '100', str(z))
    session.call('look', '0', str(pitch))
    session.call('wait', '10')


def beyond(session):
    """Stone placed past the runway end (z < -3), any height up to y=106."""
    return stone(session, -3, 99, -40, 3, 106, -4)


def walk_to_edge(session, edge_z):
    """Hold S (facing +Z) until the player stands sneaking on the edge."""
    session.call('key', 's', 'down')
    return poll(session, lambda s: s['player']['sneaking'] and s['player']['position'][2] < edge_z, 60)


def aim_side(session, z):
    """Pitch down until the crosshair is on the north (-Z) face of the block
    at (0, 99, z), the face the reference's manual placement uses."""
    for pitch in (82, 81, 83, 80, 84, 79, 85, 86, 78):
        session.call('look', '0', str(pitch))
        session.call('wait', '1')
        hit = (state(session).get('looking_at') or {}).get('block') or {}
        if hit.get('face') == 'north' and hit.get('pos') == [0, 99, z]:
            return pitch, hit
    return None, hit


def hand_place(session, checks, label, count=2):
    """Place `count` blocks by hand off the runway end while walking back."""
    placed = []
    for i in range(count):
        z = -3 - i
        ok, s = walk_to_edge(session, z + 0.0)
        checks.check(f'{label}: sneaking at the edge of z={z} before hand placement {i + 1}', ok,
                     s['player']['position'])
        pitch, hit = aim_side(session, z)
        checks.check(f'{label}: the crosshair is on the side face of (0, 99, {z})', pitch is not None,
                     {'pitch': pitch, 'hit': hit})
        session.call('mouse', 'right', 'click')
        session.call('wait', '4')
        count_now, blocks = stone(session, 0, 99, z - 1, 0, 99, z - 1)
        placed.append(count_now == 1)
        checks.check(f'{label}: hand placement {i + 1} put stone at (0, 99, {z - 1})', count_now == 1, blocks)
    return all(placed)


def bridge_walk(session, ticks, samples):
    """Keep S held for `ticks` ticks, sampling the player every 2."""
    for _ in range(0, ticks, 2):
        s = state(session)
        p = s['player']
        samples.append({'pos': [round(v, 2) for v in p['position']], 'sneaking': p['sneaking'],
                        'sprinting': p['sprinting'], 'keys': s.get('keys', []),
                        'yaw': round(p['yaw'], 1), 'pitch': round(p['pitch'], 1)})
        session.call('wait', '2')


def forward_walk(session):
    """A forward walk along the runway: sprint key, sprinting and speed over
    the last 20 ticks (as tools/sprint_safe_walk_lan_scenario.py)."""
    reset(session, z=-1.5)
    session.call('key', 'w', 'down')
    sprinting, s = poll(session, lambda s: s['player'].get('sprinting'), 16)
    held = 'ControlLeft' in s.get('keys', [])
    start = s['player']['position']
    session.call('wait', '20')
    end = player(session)
    session.call('key', 'w', 'up')
    session.call('wait', '6')
    speed = math.hypot(end['position'][0] - start[0], end['position'][2] - start[2])
    return {'sprinting': bool(sprinting and end.get('sprinting')), 'key_held': held,
            'blocks_per_s': round(speed, 3)}


def setup(session, checks, mode, extra=None):
    settings = {f'{module}.binds': [key] for module, key in BINDS.items()}
    settings['blatant.scaffold.mode'] = mode
    settings.update(extra or {})
    session.seed({'schema': 1, 'modules': {}, 'settings': settings})
    session.launch(creative=True)
    session.call('time', '6000')
    reset(session)
    session.call('wait-ready', '40')
    build(session, checks)
    session.call('hotbar', '1')
    session.call('give', 'minecraft:stone', '64')
    session.call('wait', '5')
    held = state(session).get('held')
    checks.check('stone held in slot 1', held and held['item'].endswith('stone'), held)


def view_bridge(session, name):
    """Look back along the bridge from the runway for the capture."""
    session.call('key', 's', 'up')
    session.call('tp', '0.5', '100', '2.5')
    session.call('look', '180', '25')
    session.call('wait', '20')
    session.capture(name)


def run_god_bridge(resources, allow_stale):
    with NativeSession(resources, 'scaffold-god-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        setup(session, checks, 'GodBridge')
        walk = forward_walk(session)
        checks.check('a walk without Sprint does not sprint', not walk['sprinting'], walk)

        toggle(session, 'blatant.scaffold')
        checks.check('Scaffold (GodBridge) enabled through its bind', enabled(session, 'blatant.scaffold'))

        # No hand placements: only the edge sneak, nothing is placed.
        reset(session, pitch=82)
        session.call('key', 's', 'down')
        session.call('wait', '50')
        idle = player(session)
        idle_count, idle_blocks = beyond(session)
        checks.check('GodBridge before activation sneaks at the edge and holds the player on the runway',
                     idle['sneaking'] and idle['position'][1] > 99.5 and -3.4 < idle['position'][2] < -2.9,
                     idle['position'])
        checks.check('GodBridge before activation places nothing by itself', idle_count == 0, idle_blocks)

        # The reference's manual activation: two blocks by hand.
        reset(session, pitch=82)
        hand_place(session, checks, 'GodBridge')
        samples = []
        bridge_walk(session, 20, samples)
        toggle(session, 'combat.sprint')
        checks.check('Sprint enabled through its bind while the bridge runs', enabled(session, 'combat.sprint'))
        bridge_walk(session, 60, samples)
        mid = state(session)
        count, blocks = beyond(session)
        lowest = min(x['pos'][1] for x in samples)
        far = min(x['pos'][2] for x in samples)
        locked = samples[10:]
        checks.check('GodBridge placed stone beyond the two hand-placed blocks', count >= 6,
                     {'count': count, 'blocks': blocks[:24]})
        checks.check('GodBridge carried the player along the bridge', far < -8.0, far)
        checks.check('GodBridge: the player never dropped below the runway top', lowest > 99.5, lowest)
        checks.check('Sprint lock: Sprint enabled while bridging does not press the sprint key',
                     all('ControlLeft' not in x['keys'] for x in locked),
                     sorted({k for x in locked for k in x['keys']}))
        checks.check('Sprint lock: the player does not sprint while bridging',
                     not any(x['sprinting'] for x in locked), None)
        session.capture('godbridge-bridging')

        # Letting go of S ends the bridge, the lock goes and Sprint presses.
        session.call('key', 's', 'up')
        released, s = poll(session, lambda s: 'ControlLeft' in s.get('keys', []), 60)
        checks.check('after S is released the bridge lets go and Sprint presses the sprint key', released,
                     {'keys': s.get('keys'), 'pos': s['player']['position']})
        settled = player(session)
        checks.check('the player stays on the bridge after letting go', settled['position'][1] > 99.5,
                     settled['position'])
        on = forward_walk(session)
        checks.check('Sprint sprints on a forward walk once the lock is gone', on['sprinting'] and on['key_held'], on)
        checks.check('the sprint is faster than the walk', on['blocks_per_s'] >= 1.15 * walk['blocks_per_s'],
                     [on['blocks_per_s'], walk['blocks_per_s']])
        final_count, final_blocks = beyond(session)
        checks.check('bridge blocks recorded', True, {'count': final_count, 'blocks': final_blocks,
                                                      'samples': samples[::5], 'mid_keys': mid.get('keys')})
        view_bridge(session, 'godbridge-bridge')
        toggle(session, 'combat.sprint')
        toggle(session, 'blatant.scaffold')
        checks.check('Scaffold disabled through its bind', not enabled(session, 'blatant.scaffold'))
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def run_telly_bridge(resources, allow_stale):
    with NativeSession(resources, 'scaffold-telly-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        setup(session, checks, 'TellyBridge', {'blatant.scaffold.require-right-click': False})
        toggle(session, 'blatant.scaffold')
        checks.check('Scaffold (TellyBridge) enabled through its bind', enabled(session, 'blatant.scaffold'))

        reset(session, pitch=82)
        session.call('key', 's', 'down')
        session.call('wait', '50')
        idle = player(session)
        idle_count, idle_blocks = beyond(session)
        checks.check('TellyBridge before activation sneaks at the edge and holds the player on the runway',
                     idle['sneaking'] and idle['position'][1] > 99.5, idle['position'])
        checks.check('TellyBridge before activation places nothing by itself', idle_count == 0, idle_blocks)

        reset(session, pitch=82)
        hand_place(session, checks, 'TellyBridge')
        samples = []
        bridge_walk(session, 70, samples)
        count, blocks = beyond(session)
        session.call('key', 's', 'up')
        session.call('wait', '30')
        end = player(session)
        levels = sorted({b[1] for b in blocks})
        checks.check('TellyBridge placed stone beyond the two hand-placed blocks', count >= 6,
                     {'count': count, 'blocks': blocks[:30]})
        checks.check('TellyBridge sprint-jumped along the line', any(x['sprinting'] for x in samples)
                     and min(x['pos'][2] for x in samples) < -8.0,
                     samples[::4])
        checks.check('TellyBridge "Y increase" raised the bridge above the runway level', max(levels or [0]) >= 100,
                     levels)
        checks.check('TellyBridge: the player did not fall to the catch floor', end['position'][1] > 99.5,
                     end['position'])
        session.capture('tellybridge-bridging')
        view_bridge(session, 'tellybridge-bridge')
        toggle(session, 'blatant.scaffold')
        checks.check('Scaffold disabled through its bind', not enabled(session, 'blatant.scaffold'))
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?',
                        default=os.environ.get('HORIZON_RESOURCES'),
                        help='prepared Minecraft 26.3 resources (or set HORIZON_RESOURCES)')
    parser.add_argument('--allow-stale', action='store_true')
    parser.add_argument('--only', choices=('god', 'telly'))
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    ok = True
    if args.only in (None, 'god'):
        ok = run_god_bridge(args.resources, args.allow_stale) and ok
    if args.only in (None, 'telly'):
        ok = run_telly_bridge(args.resources, args.allow_stale) and ok
    raise SystemExit(0 if ok else 1)


if __name__ == '__main__':
    main()
