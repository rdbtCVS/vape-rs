#!/usr/bin/env python3
"""Text GUI "Click disable" and "Match GUI color" in native play, over the
open click GUI.

The click GUI no longer hides the HUD or pauses the world: the vanilla HUD
stays under it, the Text GUI draws over it, and the host's "Game Paused"
title is gone. With Click disable on, a press on a Text GUI row toggles its
module; a module switched off this way stays listed with a grey
"| Disabled" suffix until the GUI closes, and a hovered row's colour blends
toward white. Match GUI color paints the rows with ClientSettings Gui Color.

Coordinates are GUI pixels of the 800x600 window at host GUI scale 2 (a
400x300 screen; captures are twice that). The Text GUI sits at (4000, 2),
clamped to the right edge, at scale 1: rows are 11 high from y=3 and their
boxes end at x=400. Alphabetical sort puts AutoClicker (red, no Basic
suffix) in row 0 above ESP (green). Blur background is off so a GUI capture
can be compared with play directly.
"""
import argparse
import colorsys
import json
from pathlib import Path
import numpy as np
from PIL import Image
from native_session import NativeSession

ROW0 = (396, 8.5)
# Capture pixels: the Text GUI's right-anchored rows, the vanilla hotbar,
# and where the host centres "Game Paused" (y=10, white, shadowed).
TEXT_GUI = (520, 0, 800, 60)
ROW0_BAND = (520, 6, 800, 28)
HOTBAR = (220, 556, 580, 600)
TITLE = (320, 16, 480, 42)
GUI_COLOR = 0xff2f7ae5


def pixels(path, box):
    return np.asarray(Image.open(path).convert('RGB').crop(box), dtype=int)


def red(path, box=ROW0_BAND):
    """AutoClicker's text pixels (0xff0b11, or blended toward white)."""
    p = pixels(path, box)
    mask = (p[..., 0] > 150) & (p[..., 0] - p[..., 1] > 70) & (p[..., 0] - p[..., 2] > 70)
    return p, mask


def red_count(path, box=ROW0_BAND):
    return int(red(path, box)[1].sum())


def red_green(path):
    """Mean green channel of the red text: a hover blend raises it."""
    p, mask = red(path)
    return float(p[mask][:, 1].mean()) if mask.any() else None


def red_left(path):
    """Leftmost red column: right-anchored, a longer suffix moves it left."""
    _, mask = red(path)
    columns = np.flatnonzero(mask.any(0))
    return int(columns[0]) + ROW0_BAND[0] if len(columns) else None


def green_count(path):
    p = pixels(path, TEXT_GUI)
    return int(((p[..., 1] > 150) & (p[..., 1] - p[..., 0] > 70) & (p[..., 1] - p[..., 2] > 70)).sum())


def rms(a, b, box):
    return float(np.sqrt(np.mean((pixels(a, box) - pixels(b, box)) ** 2)))


def text_hue(path):
    """Hue of the mean saturated Text GUI pixel."""
    p = pixels(path, TEXT_GUI).reshape(-1, 3)
    p = p[(p.max(1) - p.min(1) > 80) & (p.max(1) > 120)]
    if not len(p):
        return None
    r, g, b = p.mean(0) / 255
    return colorsys.rgb_to_hsv(r, g, b)[0]


def run(resources):
    with NativeSession(resources, 'text-gui-click-') as session:
        checks = []

        def check(name, ok, detail=None):
            checks.append({'check': name, 'passed': bool(ok), 'detail': detail})
            (session.root / 'assertions.json').write_text(
                json.dumps({'passed': all(c['passed'] for c in checks), 'checks': checks},
                           indent=2) + '\n')
            if not ok:
                raise AssertionError(f'{name}: {detail}')

        def key(name):
            session.call('key', name, 'tap')

        def capture(name):
            session.capture(name)
            return session.root / (name + '.png')

        def hover(point):
            session.call('scroll-at', str(point[0]), str(point[1]), '0')

        def click(point):
            session.call('click-at', str(point[0]), str(point[1]))

        def enabled(module):
            return session.saved()['modules'].get(module, {}).get('enabled', False)

        def relaunch(settings):
            session.close()
            document = session.saved()
            document['settings'].update(settings)
            session.seed(document)
            session.launch()
            session.call('wait', '10')

        session.seed({'schema': 1,
                      'modules': {'hud.text-gui': {'enabled': True},
                                  'combat.auto-clicker': {'enabled': True},
                                  'render.esp': {'enabled': True}},
                      'settings': {'hud.text-gui.sort': 'Alphabetical',
                                   'client.blur-background': False}})
        session.launch()
        session.call('wait', '10')
        playing = capture('playing')
        check('Text GUI lists AutoClicker and ESP in play',
              red_count(playing) > 20 and green_count(playing) > 20,
              [red_count(playing), green_count(playing)])

        # The HUD stays under the open GUI and the world keeps ticking.
        key('rshift')
        session.call('wait', '10')
        gui = capture('gui-open')
        check('the click GUI is open', session.call('state')['screen'] == 'paused')
        check('the vanilla hotbar stays under the GUI', rms(gui, playing, HOTBAR) < 12,
              rms(gui, playing, HOTBAR))
        check('no "Game Paused" title over the GUI', rms(gui, playing, TITLE) < 12,
              rms(gui, playing, TITLE))
        check('the Text GUI draws over the GUI',
              red_count(gui) > 20 and green_count(gui) > 20, [red_count(gui), green_count(gui)])

        # Click disable is off by default: no hover, no toggle.
        hover(ROW0)
        session.call('wait', '4')
        still = capture('click-disable-off-hover')
        check('without Click disable a hover keeps the colour',
              abs(red_green(still) - red_green(gui)) < 8, [red_green(gui), red_green(still)])
        click(ROW0)
        session.call('wait', '4')
        check('without Click disable a row click does nothing',
              enabled('combat.auto-clicker') and enabled('render.esp'))
        key('rshift')

        relaunch({'hud.text-gui.click-disable': True})
        key('rshift')
        hover((200, 280))
        session.call('wait', '4')
        base = capture('click-disable-open')
        base_left, base_green = red_left(base), red_green(base)
        hover(ROW0)
        session.call('wait', '4')
        hovered = capture('click-disable-hover')
        check('a hovered row blends toward white', red_green(hovered) - base_green > 20,
              [base_green, red_green(hovered)])

        click(ROW0)
        session.call('wait', '4')
        disabled = capture('click-disable-disabled')
        check('a row click disables its module',
              not enabled('combat.auto-clicker') and enabled('render.esp'))
        check('the disabled row stays listed with a longer suffix',
              red_count(disabled) > 20 and red_left(disabled) < base_left - 20,
              [base_left, red_left(disabled), red_count(disabled)])

        click(ROW0)
        session.call('wait', '4')
        again = capture('click-disable-reenabled')
        check('a second click enables it again', enabled('combat.auto-clicker'))
        check('the re-enabled row drops the Disabled suffix',
              abs(red_left(again) - base_left) <= 2, [base_left, red_left(again)])

        click(ROW0)
        session.call('wait', '4')
        check('a third click disables it', not enabled('combat.auto-clicker'))
        key('rshift')
        session.call('wait', '10')
        closed = capture('click-disable-closed')
        check('closing the GUI drops the held row',
              red_count(closed, TEXT_GUI) == 0 and green_count(closed) > 20,
              [red_count(closed, TEXT_GUI), green_count(closed)])

        relaunch({'hud.text-gui.color-mode': 'Match GUI color', 'client.gui-color': GUI_COLOR,
                  'hud.text-gui.click-disable': False})
        matched = capture('match-gui-color')
        want = colorsys.rgb_to_hsv(*(((GUI_COLOR >> s) & 0xff) / 255 for s in (16, 8, 0)))[0]
        got = text_hue(matched)
        check('Match GUI color paints rows with Gui Color',
              got is not None and abs(got - want) < 0.03 and red_count(matched, TEXT_GUI) == 0,
              [want, got])
        key('rshift')
        session.call('wait', '4')
        capture('match-gui-color-open')
        check('the Gui Color is saved as set',
              session.saved()['settings'].get('client.gui-color') == GUI_COLOR,
              session.saved()['settings'].get('client.gui-color'))
        key('rshift')
        session.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
