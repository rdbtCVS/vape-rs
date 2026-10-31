#!/usr/bin/env python3
"""Validate the local profile model in native play: migration of a
pre-profile live state, switching from the GUI with the switch alert,
capture of the profile left behind, global values kept, a profile keybind
captured in the GUI and gated by screens, right-click state apply, the
selected profile's inert Delete, restart, and "Auto-load module states"
off."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession

PROFILES = (30, 192)
# The Profiles frame in the first column: rows 18 apart from y=47; an open
# row menu lists Bind, Save, Rename, Export and Delete under its row.
def profile_row(i):
    return 160, 55 + 18 * i
def profile_dots(i):
    return 221, 55 + 18 * i
MENU_BIND, MENU_DELETE = (170, 70), (170, 122)


def run(resources):
    with NativeSession(resources, 'profile-switch-') as session:
        session.seed({'schema': 1, 'modules': {
            'render.fullbright': {'enabled': True}, 'hud.coordinates': {'enabled': True}},
            'settings': {'client.profile-alerts': True, 'render.fullbright.mode': 'gamma'}})
        folder = session.current.parent / 'profiles'
        folder.mkdir(parents=True)
        (folder / 'legacy.json').write_text(json.dumps({'schema': 1, 'modules': {
            'hud.fps': {'enabled': True}},
            'settings': {'hud.coordinates.display': 'vertical'}}))

        def click(point, button=()):
            session.call('click-at', str(point[0]), str(point[1]), *button)
            session.call('wait', '2')

        def key(name, action='tap'):
            session.call('key', name, action)
            session.call('wait', '2')

        def enabled(module):
            return session.saved()['modules'].get(module, {}).get('enabled', False)

        def profile_file(name):
            return json.loads((folder / f'{name}.json').read_text())

        checks = []
        session.launch()
        saved = session.saved()
        assert saved['profile'] == 'Default', saved.get('profile')
        assert saved['profile_order'] == ['Default', 'legacy'], saved['profile_order']
        assert profile_file('Default')['modules']['render.fullbright']['enabled'] is True
        assert enabled('render.fullbright') and enabled('hud.coordinates')
        checks.append('a pre-profile live state becomes the selected "Default" profile')

        key('rshift')
        click(PROFILES)
        session.capture('migrated')
        click(profile_row(1))
        session.capture('switched')
        saved = session.saved()
        assert saved['profile'] == 'legacy'
        assert not enabled('render.fullbright') and not enabled('hud.coordinates')
        assert enabled('hud.fps')
        assert saved['settings']['hud.coordinates.display'] == 'vertical'
        assert saved['settings']['client.profile-alerts'] is True
        default = profile_file('Default')
        assert default['modules']['render.fullbright']['enabled'] is True
        assert default['settings']['render.fullbright.mode'] == 'gamma'
        checks.append('a left click switches, capturing the profile left behind')
        checks.append('global values survive a switch; per-profile values follow it')

        click(profile_dots(0))
        click(MENU_BIND)
        key('k')
        session.capture('profile-bind')
        assert session.saved()['profile_binds'] == {'Default': ['KeyK']}
        click(profile_dots(0))
        key('rshift')
        assert session.call('state')['screen'] == 'playing'

        key('e')
        assert session.call('state')['screen'] == 'inventory'
        key('k')
        assert session.saved()['profile'] == 'legacy'
        key('esc')
        assert session.call('state')['screen'] == 'playing'
        checks.append('an open screen blocks a profile bind')
        key('k')
        session.capture('bind-switched')
        assert session.saved()['profile'] == 'Default'
        assert enabled('render.fullbright') and enabled('hud.coordinates')
        assert not enabled('hud.fps')
        assert profile_file('legacy')['modules']['hud.fps']['enabled'] is True
        checks.append('a profile bind captured in the GUI switches profiles')

        key('rshift')
        click(profile_row(1), ['right'])
        session.capture('right-click-applied')
        assert session.saved()['profile'] == 'Default'
        assert not enabled('render.fullbright')
        assert enabled('hud.coordinates')
        checks.append('a right click applies saved non-HUD states without switching')

        click(profile_dots(0))
        click(MENU_DELETE)
        session.capture('selected-delete-ignored')
        assert (folder / 'Default.json').is_file()
        click(profile_dots(0))
        key('rshift')
        checks.append('the selected profile cannot be deleted')

        session.close()
        session.launch()
        saved = session.saved()
        assert saved['profile'] == 'Default'
        assert saved['profile_binds'] == {'Default': ['KeyK']}
        assert not enabled('render.fullbright') and enabled('hud.coordinates')
        session.capture('after-restart')
        checks.append('the selected profile, order and binds persist across restart')
        (session.root / 'assertions.json').write_text(json.dumps(
            {'passed': True, 'checks': checks}, indent=2) + '\n')

    with NativeSession(resources, 'profile-autoload-off-') as session:
        session.seed({'schema': 1, 'profile': 'A', 'profile_order': ['A', 'B'],
                      'profile_binds': {'B': ['KeyK']},
                      'modules': {'render.fullbright': {'enabled': True},
                                  'hud.coordinates': {'enabled': True}},
                      'settings': {'client.profile-auto-load': False}})
        folder = session.current.parent / 'profiles'
        folder.mkdir(parents=True)
        (folder / 'A.json').write_text(json.dumps({'schema': 1, 'modules': {
            'render.fullbright': {'enabled': True}, 'hud.coordinates': {'enabled': True}}}))
        (folder / 'B.json').write_text(json.dumps({'schema': 1, 'modules': {
            'hud.fps': {'enabled': True}},
            'settings': {'hud.coordinates.display': 'vertical'}}))

        def enabled(module):
            return session.saved()['modules'].get(module, {}).get('enabled', False)

        session.launch()
        # Startup restores only the HUD modules' states.
        assert not enabled('render.fullbright')
        assert enabled('hud.coordinates')
        session.capture('startup')
        session.call('key', 'k', 'tap')
        session.call('wait', '2')
        saved = session.saved()
        assert saved['profile'] == 'B'
        assert enabled('hud.coordinates') and not enabled('hud.fps')
        assert saved['settings']['hud.coordinates.display'] == 'vertical'
        assert saved['settings']['client.profile-auto-load'] is False
        session.capture('switched-states-kept')
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': [
            'with auto-load off, startup restores only HUD module states',
            'with auto-load off, a switch applies values but keeps module states']},
            indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
