#!/usr/bin/env python3
"""Exercise NoHurtCam without changing actual damage or hurt timer state."""
import argparse
import json
from pathlib import Path
import time
from native_session import NativeSession


def run(resources):
    with NativeSession(resources, 'no-hurt-camera-') as session:
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
        session.call('click-at', '40', '228')
        session.capture('modules')
        session.call('click-at', '150', '207')
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
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True,
            'checks': ['actual damage and hurt timer remain active', 'physical binding', 'GUI toggle',
                       'restart preference', 'panic disables camera override'],
            'limits': ['Camera matrix behavior is tested in embedded native Rust tests; captures alone do not prove exact visual parity.']}, indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
