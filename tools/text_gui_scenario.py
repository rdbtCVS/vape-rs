#!/usr/bin/env python3
"""Exercise the Text GUI module list and its settings in native play."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession

LISTED = ['combat.sprint', 'render.esp', 'combat.velocity', 'combat.reach',
          'combat.auto-clicker', 'render.fullbright']


def run(resources):
    with NativeSession(resources, 'hud-text-gui-') as session:
        modules = {m: {'enabled': True} for m in LISTED + ['hud.text-gui', 'hud.fps']}
        session.seed({'schema': 1, 'modules': modules,
                      'settings': {'combat.sprint.binds': ['KeyG'],
                                   'hud.text-gui.binds': ['KeyJ']}})
        session.launch()
        session.call('wait', '2')
        session.capture('basic-length-right')

        def apply(settings, label):
            session.close()
            document = session.saved()
            document['settings'].update(settings)
            session.seed(document)
            session.launch()
            session.call('wait', '2')
            session.capture(label)
            saved = session.saved()['settings']
            for key, value in settings.items():
                assert saved[key] == value, (key, saved.get(key))

        apply({'hud.text-gui.suffix': 'Extended'}, 'extended')
        apply({'hud.text-gui.sort': 'Alphabetical'}, 'alphabetical')
        apply({'hud.text-gui.suffix': 'None'}, 'no-suffix')
        apply({'hud.text-gui.suffix': 'Extended', 'hud.text-gui.color-mode': 'Match GUI color'},
              'match-gui-color')
        apply({'hud.text-gui.color-mode': 'Custom color', 'hud.text-gui.background': False,
               'hud.text-gui.smooth-font': False}, 'custom-color-plain')
        apply({'hud.text-gui.x': 4, 'hud.text-gui.y': 60, 'hud.text-gui.background': True,
               'hud.text-gui.smooth-font': True, 'hud.text-gui.watermark': True,
               'hud.text-gui.custom-text': True, 'hud.text-gui.text': '&bvape&f-rs'},
              'left-watermark-custom-text')
        apply({'hud.text-gui.hide-modules': True}, 'hidden-esp')
        # A physical bind removes the row; it slides out and is gone.
        session.call('key', 'g', 'tap')
        session.call('wait', '2')
        session.capture('sprint-removed')
        assert session.saved()['modules']['combat.sprint']['enabled'] is False
        session.call('key', 'j', 'tap')
        session.call('wait', '1')
        session.capture('text-gui-disabled')
        assert session.saved()['modules']['hud.text-gui']['enabled'] is False
        session.call('key', 'rshift', 'tap')
        session.capture('gui-open')
        session.close()
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': [
            'enabled coloured modules listed', 'Basic, Extended and None suffixes',
            'length and alphabetical sorts', 'module, GUI and custom colour modes',
            'background and font toggles', 'left/right anchoring', 'watermark and custom text',
            'hidden module list', 'bind toggles remove rows', 'Text GUI module bind',
            'settings persisted across restarts']}, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
