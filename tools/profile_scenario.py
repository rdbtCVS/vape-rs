#!/usr/bin/env python3
"""Assert real native persistence/chord effects; capture HUD for visual review."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession


def run(resources, in_frame=False):
    with NativeSession(resources, 'profiles-', in_frame) as session:
        session.seed({'schema': 1, 'modules': {'hud.coordinates': {'enabled': True}, 'hud.fps': {'enabled': False}},
                      'settings': {'hud.coordinates.display': 'vertical', 'hud.coordinates.x': 18,
                                   'hud.coordinates.y': 20, 'hud.coordinates.scale': 1.25,
                                   'hud.coordinates.color': 0xff06a17e, 'hud.coordinates.background': False,
                                   'hud.fps.binds': ['KeyG', 'F9'], 'hud.fps.activation': 'hold',
                                   'future.option': {'preserve': True}}, 'future': {'preserve': True}})
        session.launch()
        session.capture('loaded-layout')
        assert session.saved()['modules']['hud.coordinates']['enabled'] is True
        session.call('key', 'g', 'down')
        session.call('wait', '2')
        assert session.saved()['modules']['hud.fps']['enabled'] is False
        session.call('key', 'f9', 'down')
        session.call('wait', '2')
        assert session.saved()['modules']['hud.fps']['enabled'] is True
        session.capture('held-chord')
        session.call('key', 'g', 'up')
        session.call('wait', '2')
        assert session.saved()['modules']['hud.fps']['enabled'] is False
        session.call('key', 'f9', 'up')
        session.call('key', 'f10', 'tap')
        session.call('wait', '2')
        assert session.saved()['modules']['hud.coordinates']['enabled'] is False
        assert session.saved()['future']['preserve'] is True
        assert session.saved()['settings']['future.option']['preserve'] is True
        session.capture('panic')
        session.close()
        session.launch()
        assert session.saved()['modules']['hud.coordinates']['enabled'] is False
        session.call('key', 'f8', 'tap')
        session.call('wait', '2')
        saved = session.saved()
        assert saved['modules']['hud.coordinates']['enabled'] is True
        assert saved['settings']['hud.coordinates.display'] == 'vertical'
        assert saved['settings']['hud.coordinates.scale'] == 1.25
        session.capture('after-restart')
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': ['settings affect HUD', 'chord requires all keys', 'held release disables', 'panic persists', 'restart restores settings', 'unknown fields preserved']}, indent=2))
    with NativeSession(resources, 'corrupt-profile-', in_frame) as session:
        session.current.parent.mkdir(parents=True, exist_ok=True)
        session.current.write_bytes(b'broken profile JSON')
        session.launch()
        session.call('key', 'f8', 'tap')
        session.call('wait', '2')
        assert session.current.read_bytes() == b'broken profile JSON'
        assert 'preserving its file' in (session.root / 'game.log').read_text()
        session.capture('fallback-preserves-corrupt-file')
        (session.root / 'assertions.json').write_text('{"passed":true,"checks":["corrupt profile preserved", "client remains usable"]}\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--in-frame', action='store_true')
    args = parser.parse_args()
    run(args.resources, args.in_frame)
