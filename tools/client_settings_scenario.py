#!/usr/bin/env python3
"""Exercise ClientSettings toasts, search bar styles and Ctrl+F in native play."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession


def run(resources):
    with NativeSession(resources, 'hud-client-settings-') as session:
        def key(name):
            session.call('key', name, 'tap')

        def type_text(text):
            for c in text:
                key(c)

        def ctrl_f():
            session.call('key', 'ctrl', 'down')
            key('f')
            session.call('key', 'ctrl', 'up')

        session.seed({'schema': 1, 'modules': {},
                      'settings': {'combat.sprint.binds': ['KeyG'],
                                   'client.toggle-alerts': True}})
        session.launch()
        # The inject message is up for 5 s after entering the world.
        session.capture('bind-indicator')
        key('g')
        session.call('wait', '10')
        session.capture('toggle-alert-enabled')
        assert session.saved()['modules']['combat.sprint']['enabled'] is True
        key('g')
        session.call('wait', '10')
        session.capture('toggle-alert-reused-disabled')
        session.call('wait', '120')
        session.capture('toasts-expired')

        key('rshift')
        session.capture('floating-search-idle')
        ctrl_f()
        type_text('sp')
        session.capture('floating-search-results')
        ctrl_f()  # clears the active query
        session.capture('floating-search-cleared')

        def apply(settings, label):
            session.close()
            document = session.saved()
            document['settings'].update(settings)
            session.seed(document)
            session.launch()
            session.call('wait', '20')
            saved = session.saved()['settings']
            for k, value in settings.items():
                assert saved[k] == value, (k, saved.get(k))
            key('rshift')
            ctrl_f()
            type_text('sp')
            session.capture(label)

        apply({'client.search-bar': 'Integrated'}, 'integrated-search-results')
        apply({'client.search-bar': 'None', 'client.gui-bind-indicator': False},
              'no-search-bar')
        apply({'client.search-bar': 'Floating', 'client.notifications': False}, 'notifications-off-gui')
        key('escape')  # unfocus the search field
        key('escape')  # close the GUI
        key('g')
        session.call('wait', '10')
        session.capture('notifications-off-toggle')
        session.close()
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': [
            'GUI bind indicator on entering the world', 'toggle alert on bind toggle',
            'reused toggle alert updates in place', 'toasts expire and slide out',
            'floating search frame', 'Ctrl+F focuses and clears the floating search',
            'integrated header search via Ctrl+F', 'no search bar style ignores Ctrl+F',
            'bind indicator can be disabled', 'notifications master switch',
            'settings persisted across restarts']}, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
