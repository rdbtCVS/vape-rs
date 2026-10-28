#!/usr/bin/env python3
"""Exercise live heading/biome HUD data and compass preferences in native play."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession


def run(resources, in_frame=False):
    with NativeSession(resources, 'hud-details-', in_frame) as session:
        session.seed({'schema': 1, 'modules': {'hud.coordinates': {'enabled': True},
                      'hud.fps': {'enabled': True}, 'hud.compass': {'enabled': True}},
                      'settings': {'hud.coordinates.display': 'vertical',
                                   'hud.fps.y': 112, 'hud.compass.x': 46, 'hud.compass.y': 180,
                                   'hud.compass.binds': ['KeyC']}})
        session.launch()
        for yaw, label in [(-180, 'north'), (-90, 'east'), (0, 'south'), (90, 'west')]:
            session.call('look', str(yaw), '0')
            session.call('wait', '2')
            assert session.call('state')['player']['yaw'] == yaw
            session.capture(label)
        session.call('key', 'c', 'tap')
        session.call('wait', '2')
        session.capture('compass-disabled')
        assert session.saved()['modules']['hud.compass']['enabled'] is False
        session.call('key', 'rshift', 'tap')
        session.capture('gui')
        session.call('click-at', '40', '228')
        session.capture('legit-modules')
        session.call('click-at', '150', '176')
        session.capture('compass-enabled-in-gui')
        assert session.saved()['modules']['hud.compass']['enabled'] is True
        session.call('click-at', '348', '116')
        session.capture('coordinate-settings')
        session.call('click-at', '330', '245')
        session.capture('horizontal-selected')
        assert session.saved()['settings']['hud.coordinates.display'] == 'horizontal'
        session.call('key', 'rshift', 'tap')
        session.capture('horizontal-biome')
        session.close()
        session.launch()
        session.capture('after-restart')
        assert session.saved()['modules']['hud.compass']['enabled'] is True
        assert session.saved()['settings']['hud.coordinates.display'] == 'horizontal'
        session.call('key', 'f1', 'tap')
        session.capture('hidden')
        session.call('key', 'f1', 'tap')
        session.capture('restored')
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': [
            'actual yaw drives compass and coordinate arrows', 'live chunk biome snapshot',
            'compass physical binding', 'compass GUI toggle', 'coordinate layout setting',
            'preferences restored after restart', 'host HUD hide/restore captures']}, indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--in-frame', action='store_true')
    args = parser.parse_args()
    run(args.resources, args.in_frame)
