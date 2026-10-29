#!/usr/bin/env python3
"""Exercise native caret/selection, isolated clipboard and modal input routing."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession


def run(resources):
    with NativeSession(resources, 'text-editor-') as session:
        # Run this driver under xvfb-run: the game needs a display even with
        # offscreen rendering. Clipboard operations stay in its private X server.
        import os
        if not os.environ.get('VAPE_TEST_PRIVATE_DISPLAY'):
            raise RuntimeError('run with VAPE_TEST_PRIVATE_DISPLAY=1 xvfb-run -a')
        session.env.pop('WAYLAND_DISPLAY', None)
        session.env.pop('WAYLAND_SOCKET', None)
        def key(name):
            session.call('key', name, 'tap')
        def type_text(text):
            for c in text:
                key(c)
        def click(x, y):
            session.call('click-at', str(x), str(y))
        def shortcut(name):
            session.call('key', 'ctrl', 'down')
            key(name)
            session.call('key', 'ctrl', 'up')
        session.launch()
        key('rshift')
        session.capture('main')
        click(235, 46)
        session.capture('profiles')
        click(240, 90)
        type_text('abcde')
        key('home')
        key('right')
        key('right')
        key('delete')
        session.call('key', 'shift', 'down')
        key('right')
        key('right')
        session.call('key', 'shift', 'up')
        session.capture('selected')
        type_text('xy')
        key('end')
        key('backspace')
        session.capture('edited')
        shortcut('a')
        shortcut('c')
        click(235, 118)
        session.capture('saved')
        folder = session.current.parent / 'profiles'
        assert (folder / 'abx.json').is_file()
        click(335, 145)
        session.capture('confirmation')
        # Every background control and field is inert while modal is shown.
        click(240, 90)
        type_text('bad')
        click(235, 118)
        click(165, 46)
        session.capture('modal-captured')
        assert (folder / 'abx.json').is_file()
        assert not (folder / 'abxbad.json').exists()
        click(335, 155)
        session.capture('cancelled')
        click(165, 46)
        session.capture('friends')
        click(35, 90)
        type_text('alpha')
        click(35, 117)
        shortcut('v')
        session.capture('pasted')
        shortcut('a')
        shortcut('x')
        session.capture('cut')
        shortcut('v')
        click(45, 146)
        session.capture('person-added')
        assert session.saved()['friends'][0]['name'] == 'alpha'
        assert session.saved()['friends'][0]['alias'] == 'abx'
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': ['caret home/end/arrows', 'forward/backward deletion', 'shift selection replacement', 'clipboard copy/cut/paste on private test display', 'modal blocks background typing/clicks', 'cancel preserves profile']}, indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
