#!/usr/bin/env python3
"""Exercise the actual custom native GUI through host pointer/key events."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession


def run(resources, in_frame=False):
    with NativeSession(resources, 'gui-', in_frame) as session:
        def capture(name):
            session.capture(name)
        def click(x, y):
            session.call('click-at', str(x), str(y))
        def type_text(text):
            for char in text:
                if not char.isascii() or not char.isalnum():
                    raise ValueError('this scenario uses simple physical letter keys')
                session.call('key', char.lower(), 'tap')
        def replace(x, y, text):
            click(x, y)
            session.call('key', 'ctrl', 'down')
            session.call('key', 'a', 'tap')
            session.call('key', 'ctrl', 'up')
            type_text(text)
        def key(name):
            session.call('key', name, 'tap')
        session.launch()
        key('rshift')
        capture('main')
        assert session.call('state')['screen'] == 'paused'
        click(40, 228)  # Legit modules in the reference-width sidebar.
        capture('modules')
        click(150, 116)
        capture('module-enabled')
        assert session.saved()['modules']['hud.coordinates']['enabled'] is True
        click(348, 116)
        capture('settings')
        click(330, 245)
        capture('vertical-setting')
        assert session.saved()['settings']['hud.coordinates.display'] == 'vertical'
        click(330, 184)
        key('g')
        capture('binding')
        assert session.saved()['settings']['hud.coordinates.binds'] == ['KeyG']
        click(315, 46)
        capture('global-settings')
        click(330, 184)
        capture('multi-bind')
        assert session.saved()['settings']['client.multi-bind'] is True
        click(235, 46)
        capture('profiles')
        click(240, 90)
        type_text('one')
        click(235, 118)
        capture('profile-saved')
        folder = session.current.parent
        assert (folder / 'profiles/one.json').is_file()
        click(338, 171)
        capture('profile-exported')
        assert (folder / 'exports/one.json').is_file()
        replace(240, 90, 'two')
        click(235, 145)
        capture('profile-renamed')
        assert (folder / 'profiles/two.json').is_file()
        assert not (folder / 'profiles/one.json').exists()
        replace(240, 90, 'copy')
        click(235, 118)
        capture('profile-copy')
        assert (folder / 'profiles/copy.json').is_file()
        click(335, 145)
        capture('delete-confirmation')
        assert (folder / 'profiles/copy.json').is_file()
        click(235, 155)
        capture('profile-deleted')
        assert not (folder / 'profiles/copy.json').exists()
        (folder / 'imports').mkdir(exist_ok=True)
        (folder / 'imports/incoming.json').write_text(json.dumps({'schema': 1, 'modules': {'hud.fps': {'enabled': True}}, 'future': {'preserved': True}}))
        replace(240, 90, 'incoming')
        click(235, 170)
        capture('profile-imported')
        imported = json.loads((folder / 'profiles/incoming.json').read_text())
        assert imported['modules']['hud.fps']['enabled'] is False
        assert imported['future']['preserved'] is True
        click(55, 107)
        click(335, 118)
        capture('profile-loaded')
        assert session.saved()['modules']['hud.coordinates']['enabled'] is False
        assert session.saved()['modules']['hud.fps']['enabled'] is False
        click(165, 46)
        capture('friends')
        click(35, 90)
        type_text('alice')
        click(35, 117)
        type_text('buddy')
        click(45, 146)
        capture('friend-added')
        assert session.saved()['friends'][0]['name'] == 'alice'
        assert session.saved()['friends'][0]['alias'] == 'buddy'
        replace(35, 90, 'rival')
        replace(35, 117, 'foe')
        click(45, 174)
        capture('enemy-added')
        assert session.saved()['enemies'][0]['name'] == 'rival'
        click(376, 90)
        capture('enemy-removed')
        assert not session.saved()['enemies']
        click(340, 90)
        capture('friend-target')
        assert session.saved()['friends'][0]['target'] is False
        click(106, 211)
        before = session.call('state')
        session.call('key', 'w', 'down')
        click(2, 5)  # Actual pointer click outside the owned window must not attack.
        capture('input-captured')
        after = session.call('state')
        assert after['player']['position'] == before['player']['position']
        assert after['swings'] == before['swings']
        session.call('key', 'w', 'up')
        click(373, 46)
        capture('closed')
        assert session.call('state')['screen'] == 'playing'
        key('rshift')
        capture('reopened')
        click(235, 46)
        capture('profiles-again')
        click(55, 126)
        click(335, 118)
        capture('saved-profile-loaded')
        assert session.saved()['modules']['hud.coordinates']['enabled'] is True
        assert session.saved()['settings']['hud.coordinates.binds'] == ['KeyG']
        assert session.saved()['settings']['hud.coordinates.display'] == 'vertical'
        click(373, 46)
        capture('hud-after-profile')
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': ['GUI opens/closes safely', 'module toggle', 'setting editor', 'binding capture', 'global setting', 'profile save/rename/delete/import/export/load', 'import does not enable modules', 'local friend/enemy edits', 'GUI captures movement/attack', 'profile restores HUD configuration']}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--in-frame', action='store_true')
    args = parser.parse_args()
    run(args.resources, args.in_frame)
