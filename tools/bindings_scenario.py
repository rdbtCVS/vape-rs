#!/usr/bin/env python3
"""Validate module bindings in native play: toggle, hold, chords, mouse
buttons, screen gating, the hidden GUI hotkey, GUI rebinding and the empty
GUI bind fallback."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession
import gui_layout


def run(resources):
    with NativeSession(resources, 'bindings-') as session:
        session.seed({'schema': 1, 'modules': {}, 'settings': {
            'client.multi-bind': True,
            'hud.coordinates.binds': ['KeyG'],
            'hud.fps.binds': ['F9'], 'hud.fps.activation': 'hold',
            'hud.clock.binds': ['ControlLeft', 'KeyJ'],
            'hud.compass.binds': ['MouseMiddle']}})

        def enabled(module):
            return session.saved()['modules'].get(module, {}).get('enabled', False)

        def screen():
            return session.call('state')['screen']

        session.launch()
        session.capture('start')
        session.call('key', 'g', 'tap')
        session.call('wait', '2')
        assert enabled('hud.coordinates')
        session.capture('toggled-on')
        session.call('key', 'g', 'tap')
        session.call('wait', '2')
        assert not enabled('hud.coordinates')

        session.call('key', 'f9', 'down')
        session.call('wait', '2')
        assert enabled('hud.fps')
        session.capture('held')
        session.call('key', 'f9', 'up')
        session.call('wait', '2')
        assert not enabled('hud.fps')

        # A chord toggles only with every key down, in any order.
        session.call('key', 'j', 'tap')
        session.call('wait', '2')
        assert not enabled('hud.clock')
        session.call('key', 'ctrl', 'down')
        session.call('key', 'j', 'tap')
        session.call('key', 'ctrl', 'up')
        session.call('wait', '2')
        assert enabled('hud.clock')
        session.capture('chord')
        session.call('key', 'j', 'down')
        session.call('key', 'ctrl', 'tap')
        session.call('key', 'j', 'up')
        session.call('wait', '2')
        assert not enabled('hud.clock')

        session.call('mouse', 'middle', 'click')
        session.call('wait', '2')
        assert enabled('hud.compass')
        session.capture('mouse-bind')
        session.call('mouse', 'middle', 'click')
        session.call('wait', '2')
        assert not enabled('hud.compass')

        # No bind activates while a screen is open.
        # The scenario world is creative, so E opens the creative inventory.
        # The host hands mouse buttons to play_mouse (the mouse-bind hook)
        # only on the playing screen, and its agent refuses `mouse` behind a
        # screen, so only keys are pressed here.
        for opener, name in (('e', 'creative'), ('t', 'chat')):
            session.call('key', opener, 'tap')
            session.call('wait', '2')
            assert screen() == name, screen()
            session.call('key', 'g', 'tap')
            session.call('key', 'f9', 'down')
            session.call('wait', '2')
            session.capture(f'{name}-blocks-binds')
            session.call('key', 'f9', 'up')
            assert not enabled('hud.coordinates') and not enabled('hud.fps')
            assert not enabled('hud.compass')
            session.call('key', 'esc', 'tap')
            session.call('wait', '2')
            assert screen() == 'playing', screen()

        # Nor while the click GUI is open; Escape then closes the GUI rather
        # than opening the pause menu, which shows the GUI had the key.
        session.call('key', 'rshift', 'tap')
        session.call('wait', '2')
        session.call('key', 'g', 'tap')
        session.call('key', 'f9', 'down')
        session.call('wait', '2')
        session.capture('gui-blocks-binds')
        session.call('key', 'f9', 'up')
        assert not enabled('hud.coordinates') and not enabled('hud.fps')
        session.call('key', 'esc', 'tap')
        session.call('wait', '2')
        assert screen() == 'playing', screen()
        session.call('key', 'g', 'tap')
        session.call('wait', '2')
        assert enabled('hud.coordinates')

        # The reference's hidden hotkey: Ctrl + Home opens the GUI whatever
        # its binding.
        session.call('key', 'ctrl', 'down')
        session.call('key', 'home', 'tap')
        session.call('key', 'ctrl', 'up')
        session.call('wait', '2')
        session.capture('ctrl-home-opens-gui')
        session.call('key', 'g', 'tap')
        session.call('wait', '2')
        assert enabled('hud.coordinates')
        session.call('key', 'esc', 'tap')
        session.call('wait', '2')
        assert screen() == 'playing', screen()
        session.call('key', 'g', 'tap')
        session.call('wait', '2')
        assert not enabled('hud.coordinates')

        rebind(session, enabled)
        session.close()
        session.launch()
        saved = session.saved()['settings']
        assert saved['render.fullbright.binds'] == ['MouseRight'], saved['render.fullbright.binds']
        assert saved['render.fullbright.activation'] == 'hold'
        session.capture('after-restart')
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': [
            'toggle bind', 'hold bind releases', 'chord needs every key in any order',
            'mouse button bind', 'inventory and chat block key binds',
            'click GUI blocks binds and takes Escape', 'Ctrl + Home opens the GUI',
            'GUI rebinding captures a multi-key chord', 'Escape and left click clear a bind',
            'shift click switches toggle and hold', 'mouse button captured from the GUI and held',
            'rebinding persists across restart']}, indent=2) + '\n')

    with NativeSession(resources, 'bindings-empty-gui-') as session:
        # An empty GUI bind falls back to Right Shift, so the GUI stays reachable.
        session.seed({'schema': 1, 'modules': {}, 'settings': {
            'client.gui.binds': [], 'hud.coordinates.binds': ['KeyG']}})
        session.launch()
        assert session.saved()['settings']['client.gui.binds'] == ['ShiftRight']
        session.call('key', 'rshift', 'tap')
        session.call('wait', '2')
        session.capture('fallback-gui-open')
        session.call('key', 'esc', 'tap')
        session.call('wait', '2')
        assert session.call('state')['screen'] == 'playing'
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': [
            'empty GUI bind falls back to Right Shift and opens the GUI']}, indent=2) + '\n')


# Click GUI positions (800x600 window, GUI scale 2) from gui_layout.py: the
# Render frame opens in the first column, and Fullbright's expanded settings
# put its Bind row 37 below the row.
RENDER = gui_layout.category('Render')
FULLBRIGHT_DOTS = gui_layout.dots('Render', 'render.fullbright')
BIND_ROW = (180, FULLBRIGHT_DOTS[1] + 37)


def rebind(session, enabled):
    def click(point):
        session.call('click-at', str(point[0]), str(point[1]))
        session.call('wait', '2')

    def key(name, action='tap'):
        session.call('key', name, action)

    def binds():
        return session.saved()['settings'].get('render.fullbright.binds')

    fullbright = 'render.fullbright'
    key('rshift')
    session.call('wait', '2')
    click(RENDER)
    click(FULLBRIGHT_DOTS)
    session.capture('render-settings')
    # Multi-Keybinding: keys add up until the first one is released.
    click(BIND_ROW)
    session.capture('capturing')
    key('ctrl', 'down')
    key('k', 'down')
    key('ctrl', 'up')
    key('k', 'up')
    session.call('wait', '2')
    assert binds() == ['ControlLeft', 'KeyK'], binds()
    session.capture('chord-captured')
    key('rshift')
    session.call('wait', '2')
    key('k')
    session.call('wait', '2')
    assert not enabled(fullbright)
    key('ctrl', 'down')
    key('k')
    key('ctrl', 'up')
    session.call('wait', '2')
    assert enabled(fullbright)
    session.capture('captured-chord-toggles')
    key('ctrl', 'down')
    key('k')
    key('ctrl', 'up')
    session.call('wait', '2')
    assert not enabled(fullbright)

    key('rshift')
    session.call('wait', '2')
    click(BIND_ROW)
    key('esc')
    session.call('wait', '2')
    assert binds() == [], binds()
    session.capture('escape-cleared')
    click(BIND_ROW)
    key('k')
    session.call('wait', '2')
    assert binds() == ['KeyK'], binds()
    click(BIND_ROW)
    click(BIND_ROW)  # a left click ends the capture with nothing
    assert binds() == [], binds()
    session.capture('left-click-cleared')

    key('shift', 'down')
    click(BIND_ROW)
    key('shift', 'up')
    assert session.saved()['settings']['render.fullbright.activation'] == 'hold'
    session.capture('shift-click-hold')

    click(BIND_ROW)
    # The open GUI is the pause screen, so the right button goes through the
    # window (the GUI's event hook), not the agent's `mouse`.
    session.call('click-at', str(BIND_ROW[0]), str(BIND_ROW[1]), 'right')
    session.call('wait', '2')
    assert binds() == ['MouseRight'], binds()
    session.capture('mouse-captured')
    key('rshift')
    session.call('wait', '2')
    session.call('mouse', 'right', 'down')
    session.call('wait', '2')
    assert enabled(fullbright)
    session.capture('mouse-hold')
    session.call('mouse', 'right', 'up')
    session.call('wait', '2')
    assert not enabled(fullbright)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
