#!/usr/bin/env python3
"""Exercise NoHurtCam without changing actual damage or hurt timer state.

Facing a brick wall on a stone floor, each hit is followed by frames over
the hurt tilt (it peaks about two ticks after the hit), compared with the
still frame just before it: with the module on they match it, off the view
tilts, on again they match again. Results go to result.json and
measurements.json, frames next to them.
"""
import argparse
import json
from pathlib import Path
import time

import gui_layout
from legit_render_scenario import Frames, diff
from movement_scenario import FLOOR
from native_session import NativeSession
from smoke_scenario import Checks

MODULE = 'render.no-hurt-camera'


def hurt_tilt(r, session, label):
    """The largest difference from the still frame over a hit's hurt tilt."""
    r.reset()
    r.wait(20)  # past the previous hurt time
    still = r.frame(f'{label}-still', settle=2)
    before = session.call('state')['player']['health']
    session.call('command', 'damage @s 1 minecraft:generic')
    for _ in range(100):
        player = session.call('state')['player']
        if player['health'] < before and player['hurt_time'] > 0:
            break
        time.sleep(0.01)
    else:
        raise AssertionError('authoritative damage/hurt state did not arrive')
    diffs = [diff(still, r.frame(f'{label}-hurt-{i}', settle=1)) for i in range(5)]
    r.m[label] = {'hurt_time': player['hurt_time'], 'health': [before, player['health']], 'diffs': diffs}
    return player, max(diffs)


def tilt_frames(session, checks):
    r = Frames(session, checks, {MODULE: 'KeyH'})
    session.call('time', '6000')
    session.call('command', 'gamerule advance_time false')
    session.call('command', 'gamerule natural_health_regeneration false')
    # Each pass's tp and damage feedback would scroll a full chat.
    session.call('command', 'gamerule send_command_feedback false')
    r.arena()
    r.fill(-4, FLOOR + 1, 4, 9, FLOOR + 11, 4, 'minecraft:bricks')
    session.call('command', 'gamemode survival')
    r.wait(10)
    hurt, on = hurt_tilt(r, session, 'enabled')
    checks.check('NoHurtCam on: the hit lands (health down, hurt time running)', hurt['hurt_time'] > 0, hurt)
    r.toggle(MODULE, False)
    hurt, off = hurt_tilt(r, session, 'disabled')
    # On a loaded machine the first capture can land after the ten-tick
    # tilt; measure again (every attempt stays in measurements.json).
    for attempt in range(1, 3):
        if off > 4:
            break
        hurt, off = hurt_tilt(r, session, f'disabled-retry-{attempt}')
    checks.check('NoHurtCam off: the hit lands', hurt['hurt_time'] > 0, hurt)
    r.toggle(MODULE, True)
    hurt, again = hurt_tilt(r, session, 'enabled-again')
    m = [on, off, again]
    checks.check('NoHurtCam off: the hurt tilt moves the view', off > 4, m)
    checks.check('NoHurtCam on: the view stays as it was before the hit',
                 on < off * 0.15 and again < off * 0.15, m)
    session.call('command', 'gamemode creative')
    (session.root / 'measurements.json').write_text(json.dumps(r.m, indent=2))


def run(resources):
    with NativeSession(resources, 'no-hurt-camera-') as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {'render.no-hurt-camera': {'enabled': True}},
                      'settings': {'render.no-hurt-camera.binds': ['KeyH']}})
        def damaged(previous):
            for _ in range(50):
                player = session.call('state')['player']
                if player['health'] < previous and player['hurt_time'] > 0:
                    return player
                time.sleep(0.02)
            raise AssertionError('authoritative damage/hurt state did not arrive')
        session.launch()
        tilt_frames(session, checks)
        session.call('chat', '/gamemode survival')
        session.call('tp', '0', '100', '0')
        session.call('chat', '/effect give @s minecraft:slow_falling 60 0 true')
        session.call('look', '0', '0')
        session.call('wait', '2')
        before = session.call('state')['player']['health']
        session.call('chat', '/damage @s 2 minecraft:generic')
        hurt = damaged(before)
        session.capture('enabled-hurt')
        assert session.saved()['modules']['render.no-hurt-camera']['enabled'] is True
        session.call('key', 'h', 'tap')
        session.call('wait', '2')
        assert session.saved()['modules']['render.no-hurt-camera']['enabled'] is False
        session.capture('disabled')
        session.call('wait', '12')
        before = session.call('state')['player']['health']
        session.call('chat', '/damage @s 2 minecraft:generic')
        hurt = damaged(before)
        session.capture('disabled-hurt')
        session.call('key', 'rshift', 'tap')
        session.capture('gui')
        session.call('click-at', *map(str, gui_layout.category('Render')))
        session.capture('modules')
        session.call('click-at', *map(str, gui_layout.row('Render', 'render.no-hurt-camera')))
        session.capture('enabled-in-gui')
        assert session.saved()['modules']['render.no-hurt-camera']['enabled'] is True
        session.call('key', 'rshift', 'tap')
        session.close()
        session.launch()
        assert session.saved()['modules']['render.no-hurt-camera']['enabled'] is True
        session.capture('after-restart')
        session.call('key', 'f10', 'tap')
        session.call('wait', '2')
        assert session.saved()['modules']['render.no-hurt-camera']['enabled'] is False
        session.capture('panic-disabled')
        ok = all(c['ok'] for c in checks.results)
        (session.root / 'assertions.json').write_text(json.dumps({'passed': ok,
            'checks': ['hurt tilt absent on, present off, absent on again (frame differences)',
                       'actual damage and hurt timer remain active', 'physical binding', 'GUI toggle',
                       'restart preference', 'panic disables camera override'],
            'limits': ['Frames are compared by mean difference; the exact tilt angle is tested in embedded native Rust tests.']}, indent=2)+'\n')
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}',
              session.root)
        return ok


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    raise SystemExit(0 if run(args.resources) else 1)
