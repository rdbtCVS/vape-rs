#!/usr/bin/env python3
"""Exercise the actual custom native GUI through host pointer/key events.

Coordinates are GUI pixels of the 800x600 offscreen window at host GUI scale
2, which equal the click GUI's logical units at its default scale: the
navigation frame sits at (6, 6) and a frame opened alone takes the first
column at (122, 6) and keeps that spot.
"""
import argparse
import json
from pathlib import Path
import gui_layout
from native_session import NativeSession

# Navigation rows (centres): 22-unit header, 20-unit rows, 14-unit MISC label.
COMBAT, RENDER = gui_layout.category('Combat'), gui_layout.category('Render')
FRIENDS, PROFILES = (30, 172), (30, 192)
GEAR, HUD_EDITOR = (94, 16), (107, 232)
# A frame in the first column: header 6..26, body from y=26, 20-unit rows.
FIRST_ROW, FIRST_DOTS = (160, 36), (221, 36)
# Fullbright's offset from the Render frame's first row, by name order with
# default-hidden modules (AntiDebuff) left out; its inline settings open
# below it.
FB = gui_layout.row('Render', 'render.fullbright')[1] - FIRST_ROW[1]
FULLBRIGHT_ROW, FULLBRIGHT_DOTS = (160, 36 + FB), (221, 36 + FB)
CREATE, IMPORT, PROFILE_FIELD, SUBMIT = (150, 36), (203, 36), (170, 54), (221, 54)
# With the name entry closed, profile rows are 18 apart from y=47 and an open
# row menu lists Save, Rename, Export and Delete under that row.
def profile_row(i):
    return 160, 55 + 18 * i
def profile_dots(i):
    return 221, 55 + 18 * i
MENU_RENAME, MENU_EXPORT, MENU_DELETE = (170, 83), (170, 96), (170, 109)
CONFIRM_DELETE = (167, 161)
PERSON, ALIAS, FRIEND, ENEMY = (175, 36), (175, 53), (150, 70), (203, 70)


def run(resources):
    with NativeSession(resources, 'gui-') as session:
        def capture(name):
            session.capture(name)
        def click(point):
            session.call('click-at', str(point[0]), str(point[1]))
        def type_text(text):
            for char in text:
                if not char.isascii() or not char.isalnum():
                    raise ValueError('this scenario uses simple physical letter keys')
                session.call('key', char.lower(), 'tap')
        def replace(point, text):
            click(point)
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
        folder = session.current.parent

        # Category frame: a row toggles, the dots expand inline settings.
        click(RENDER)
        capture('render-frame')
        click(FULLBRIGHT_ROW)
        capture('module-enabled')
        assert session.saved()['modules']['render.fullbright']['enabled'] is True
        click(FULLBRIGHT_DOTS)
        capture('settings')
        click((180, 91 + FB))  # Mode opens its choice list below the row.
        click((170, 121 + FB))
        capture('choice-setting')
        assert session.saved()['settings']['render.fullbright.mode'] == 'gamma'
        click((180, 73 + FB))
        key('g')
        capture('binding')
        assert session.saved()['settings']['render.fullbright.binds'] == ['KeyG']
        click(RENDER)

        # The header gear opens the client settings frame.
        click(GEAR)
        capture('global-settings')
        click((170, 113))  # Settings sort by id: blur background, bind indicator, GUI scale (24 high), GUI binding, multi-bind.
        capture('multi-bind')
        assert session.saved()['settings']['client.multi-bind'] is True
        click(GEAR)

        # Legit and HUD modules live in the HUD editor's frame.
        click(HUD_EDITOR)
        capture('hud-editor')
        click((190, 116))  # Coords, fifth by name.
        capture('hud-module-enabled')
        assert session.saved()['modules']['hud.coordinates']['enabled'] is True
        click((26, 287))  # BACK
        assert session.call('state')['screen'] == 'paused'

        # Profiles: create, export, rename, copy, delete, import and load.
        click(PROFILES)
        capture('profiles')
        click(CREATE)
        type_text('one')
        click(SUBMIT)
        capture('profile-saved')
        assert (folder / 'profiles/one.json').is_file()
        click(profile_dots(0))
        click(MENU_EXPORT)
        capture('profile-exported')
        assert (folder / 'exports/one.json').is_file()
        click(MENU_RENAME)
        type_text('two')
        click(SUBMIT)
        capture('profile-renamed')
        assert (folder / 'profiles/two.json').is_file()
        assert not (folder / 'profiles/one.json').exists()
        click(CREATE)
        type_text('copy')
        click(SUBMIT)
        capture('profile-copy')
        assert (folder / 'profiles/copy.json').is_file()
        click(profile_dots(0))  # copy sorts before two
        click(MENU_DELETE)
        capture('delete-confirmation')
        assert (folder / 'profiles/copy.json').is_file()
        click(CONFIRM_DELETE)
        capture('profile-deleted')
        assert not (folder / 'profiles/copy.json').exists()
        (folder / 'imports').mkdir(exist_ok=True)
        (folder / 'imports/incoming.json').write_text(json.dumps({'schema': 1, 'modules': {'hud.fps': {'enabled': True}}, 'future': {'preserved': True}}))
        click(IMPORT)
        type_text('incoming')
        click(SUBMIT)
        capture('profile-imported')
        imported = json.loads((folder / 'profiles/incoming.json').read_text())
        assert imported['modules']['hud.fps']['enabled'] is False
        assert imported['future']['preserved'] is True
        click(profile_row(0))  # incoming sorts before two
        capture('profile-loaded')
        assert session.saved()['modules']['render.fullbright']['enabled'] is False
        assert session.saved()['modules']['hud.fps']['enabled'] is False
        click(PROFILES)

        # Friends and enemies are local name lists.
        click(FRIENDS)
        capture('friends')
        click(PERSON)
        type_text('alice')
        click(ALIAS)
        type_text('buddy')
        click(FRIEND)
        capture('friend-added')
        assert session.saved()['friends'][0]['name'] == 'alice'
        assert session.saved()['friends'][0]['alias'] == 'buddy'
        replace(PERSON, 'rival')
        replace(ALIAS, 'foe')
        click(ENEMY)
        capture('enemy-added')
        assert session.saved()['enemies'][0]['name'] == 'rival'
        click((220, 82))  # enemies list first: rival's remove cross
        capture('enemy-removed')
        assert not session.saved()['enemies']
        click((208, 82))  # then alice's target toggle, moved up into its row
        capture('friend-target')
        assert session.saved()['friends'][0]['target'] is False
        click(FRIENDS)

        # Pointer and movement input stay with the GUI while it is open.
        before = session.call('state')
        session.call('key', 'w', 'down')
        click((300, 280))  # Empty screen outside every frame must not attack.
        capture('input-captured')
        after = session.call('state')
        assert after['player']['position'] == before['player']['position']
        assert after['swings'] == before['swings']
        session.call('key', 'w', 'up')
        key('rshift')
        capture('closed')
        assert session.call('state')['screen'] == 'playing'

        key('rshift')
        capture('reopened')
        click(PROFILES)
        click(profile_row(1))
        capture('saved-profile-loaded')
        saved = session.saved()
        assert saved['modules']['render.fullbright']['enabled'] is True
        assert saved['settings']['render.fullbright.binds'] == ['KeyG']
        assert saved['settings']['render.fullbright.mode'] == 'gamma'
        assert saved['modules']['hud.coordinates']['enabled'] is True
        key('rshift')
        capture('after-profile')
        assert session.call('state')['screen'] == 'playing'
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': ['GUI opens/closes safely', 'category frame module toggle', 'inline choice setting', 'binding capture', 'header settings frame', 'HUD editor module frame', 'profile create/export/rename/delete/import/load', 'import does not enable modules', 'local friend/enemy edits', 'GUI captures movement/attack', 'profile restores module configuration']}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
