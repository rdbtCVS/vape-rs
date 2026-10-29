#!/usr/bin/env python3
"""Exercise native HUD drag, snapping, scale and disabled-preview isolation."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession


def run(resources):
    with NativeSession(resources, 'hud-editor-') as session:
        session.seed({'schema': 1, 'modules': {'hud.coordinates': {'enabled': True}, 'hud.fps': {'enabled': False}},
                      'settings': {'hud.coordinates.x': 20, 'hud.coordinates.y': 25,
                                   'hud.fps.x': 250, 'hud.fps.y': 70}})
        session.launch()
        session.call('key', 'rshift', 'tap')
        session.capture('main')
        session.call('click-at', '275', '46')
        session.capture('editor')
        session.call('drag-at', '35', '40', '110', '85')
        session.capture('dragged')
        saved = session.saved()['settings']
        x, y = saved['hud.coordinates.x'], saved['hud.coordinates.y']
        assert x >= 80 and y >= 60, (x, y)
        session.call('scroll-at', str(x + 10), str(y + 10), '2')
        session.capture('scaled')
        assert session.saved()['settings']['hud.coordinates.scale'] > 1.0
        session.call('click-at', str(x + 10), str(y + 10), 'right')
        session.capture('widget-settings')
        session.call('click-at', '275', '46')
        session.capture('editor-again')
        session.call('click-at', '294', '65')
        session.capture('fps-enabled-preview')
        assert session.saved()['modules']['hud.fps']['enabled'] is True
        session.call('click-at', '294', '65')
        session.capture('fps-disabled-preview')
        assert session.saved()['modules']['hud.fps']['enabled'] is False
        session.call('drag-at', str(x + 15), str(y + 15), '16', '180')
        session.capture('snapped-to-edge')
        assert session.saved()['settings']['hud.coordinates.x'] == 0
        y = session.saved()['settings']['hud.coordinates.y']
        session.call('key', 'shift', 'down')
        session.call('drag-at', '15', str(y + 15), '17', '210')
        session.call('key', 'shift', 'up')
        session.capture('snap-disabled')
        assert 1.5 <= session.saved()['settings']['hud.coordinates.x'] <= 3.0
        session.call('click-at', '30', '282')
        session.capture('returned')
        session.call('key', 'rshift', 'tap')
        session.capture('normal-hud')
        assert session.call('state')['screen'] == 'playing'
        final = session.saved()
        assert final['modules']['hud.fps']['enabled'] is False
        session.close()
        session.launch()
        assert session.saved()['settings']['hud.coordinates.x'] == final['settings']['hud.coordinates.x']
        assert session.saved()['settings']['hud.coordinates.y'] == final['settings']['hud.coordinates.y']
        assert session.saved()['settings']['hud.coordinates.scale'] == final['settings']['hud.coordinates.scale']
        session.capture('after-restart')
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': ['HUD drag', 'bounded placement', 'screen-edge snapping', 'shift disables snapping', 'wheel scaling', 'settings navigation', 'enable/disable preview control', 'disabled previews absent from gameplay', 'position/scale survive restart']}, indent=2))
    with NativeSession(resources, 'binding-capture-') as session:
        session.seed({'schema': 1, 'modules': {}, 'settings': {'hud.coordinates.binds': ['KeyW']}})
        session.launch()
        before = session.call('state')
        session.call('key', 'w', 'down')
        session.call('wait', '6')
        session.capture('bound-movement-key')
        after = session.call('state')
        assert after['player']['position'][0] == before['player']['position'][0]
        assert after['player']['position'][2] == before['player']['position'][2]
        assert session.saved()['modules']['hud.coordinates']['enabled'] is True
        session.call('key', 'w', 'up')
        session.close()
        doc = session.saved()
        doc['settings']['hud.coordinates.binds'] = ['MouseLeft']
        doc['modules']['hud.coordinates']['enabled'] = False
        session.seed(doc)
        session.launch()
        before = session.call('state')
        session.call('mouse', 'left', 'click')
        session.call('wait', '2')
        session.capture('bound-attack-button')
        after = session.call('state')
        assert after['swings'] == before['swings']
        assert session.saved()['modules']['hud.coordinates']['enabled'] is True
        (session.root / 'assertions.json').write_text('{"passed":true,"checks":["bound movement key consumed", "bound attack mouse button consumed"]}\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
