#!/usr/bin/env python3
"""ClientSettings "Blur background" and "Rainbow speed" in native play.

The click GUI blurs the world behind it by default and draws over a sharp
world once Blur background is off; a colour's popup has a Rainbow switch,
and a rainbow Text GUI colour cycles at Rainbow speed without being saved.

Coordinates are click GUI units (see gui_scenario.py). Arrows is the Render
frame's first row; its inline settings sort by id (Activation, Bindings,
Color), so the colour row centres at y=91 and its popup's fifth row, the
Rainbow switch, at y=164.
"""
import argparse
import json
import time
from pathlib import Path
import numpy as np
from PIL import Image, ImageFilter
from native_session import NativeSession

RENDER = (30, 58)
ARROWS_DOTS, ARROWS_COLOR, RAINBOW = (221, 36), (170, 91), (170, 164)
# Capture pixels (800x600) of open world right of every frame and toast.
WORLD = (480, 160, 790, 440)
# The Text GUI's right-anchored list.
TEXT_GUI = (560, 0, 800, 80)


def world(path, blur=0):
    """The world region's grey levels, optionally Gaussian-blurred."""
    region = Image.open(path).convert('L').crop(WORLD)
    if blur:
        region = region.filter(ImageFilter.GaussianBlur(blur))
    return np.asarray(region, dtype=float).ravel()


def misfit(shown, reference):
    """RMS error of the best linear fit of a GUI capture to a play capture.

    The fit absorbs the GUI's uniform dim, so what remains is how far the
    world behind the GUI differs from the reference picture."""
    basis = np.vstack([reference, np.ones_like(reference)]).T
    coefficients, *_ = np.linalg.lstsq(basis, shown, rcond=None)
    return float(np.sqrt(np.mean((basis @ coefficients - shown) ** 2)))


def blur_fit(gui, play):
    """How well a GUI capture matches play, sharp and blurred (radius 4)."""
    shown = world(gui)
    return {'sharp': misfit(shown, world(play)), 'blurred': misfit(shown, world(play, 4))}


def text_color(path):
    """The mean colour of the saturated Text GUI pixels."""
    pixels = np.asarray(Image.open(path).convert('RGB').crop(TEXT_GUI), dtype=int).reshape(-1, 3)
    pixels = pixels[(pixels.max(1) - pixels.min(1) > 80) & (pixels.max(1) > 120)]
    if not len(pixels):
        return None
    return tuple(round(c) for c in pixels.mean(0))


def run(resources):
    with NativeSession(resources, 'hud-blur-rainbow-') as session:
        def key(name):
            session.call('key', name, 'tap')

        def click(point):
            session.call('click-at', str(point[0]), str(point[1]))

        def capture(name):
            session.capture(name)
            return session.root / (name + '.png')

        checks = []

        def check(name, ok, detail=None):
            checks.append({'check': name, 'passed': bool(ok), 'detail': detail})
            if not ok:
                raise AssertionError(f'{name}: {detail}')

        session.seed({'schema': 1,
                      'modules': {'hud.text-gui': {'enabled': True},
                                  'combat.sprint': {'enabled': True},
                                  'render.fullbright': {'enabled': True}},
                      'settings': {'hud.text-gui.color-mode': 'Custom color',
                                   'hud.text-gui.color': 0xffce0707,
                                   'hud.text-gui.color.rainbow': True,
                                   'client.rainbow-speed': 10.0}})
        session.launch()
        session.call('wait', '10')
        first = capture('rainbow-text-gui-a')
        stored = session.saved()['settings']['hud.text-gui.color']
        session.call('wait', '10')  # 0.5 s: ten 50 ms steps of 10 hue units
        second = capture('rainbow-text-gui-b')
        a, b = text_color(first), text_color(second)
        check('a rainbow Text GUI colour cycles', a and b and a != b, [a, b])
        # A save stores the current hue, as the reference does, but the
        # advance alone does not save.
        check('the cycling colour is not saved by itself',
              session.saved()['settings']['hud.text-gui.color'] == stored,
              [stored, session.saved()['settings']['hud.text-gui.color']])

        # The click GUI pauses the game, so no tick waits while it is open.
        key('rshift')
        blurred = capture('gui-blur-on')
        key('rshift')
        session.call('wait', '5')
        playing = capture('gui-closed')

        key('rshift')
        click(RENDER)
        click(ARROWS_DOTS)
        click(ARROWS_COLOR)
        session.capture('arrows-color-popup')
        click(RAINBOW)
        session.capture('arrows-rainbow-on-a')
        check('the popup switch stores the rainbow toggle',
              session.saved()['settings'].get('render.arrows.color.rainbow') is True,
              session.saved()['settings'].get('render.arrows.color.rainbow'))
        time.sleep(1)  # frames, and the 50 ms rainbow steps, keep running
        session.capture('arrows-rainbow-on-b')
        click(RAINBOW)
        session.capture('arrows-rainbow-off')
        # Back at its default, so the profile drops the key.
        check('the switch turns it off again',
              not session.saved()['settings'].get('render.arrows.color.rainbow', False),
              session.saved()['settings'].get('render.arrows.color.rainbow'))
        click(RENDER)
        key('rshift')

        session.close()
        document = session.saved()
        document['settings']['client.blur-background'] = False
        session.seed(document)
        session.launch()
        session.call('wait', '10')
        playing_again = capture('gui-closed-2')
        key('rshift')
        sharp = capture('gui-blur-off')
        # Each launch shows its own scene, so each GUI capture is compared
        # with play in its session.
        on, off = blur_fit(blurred, playing), blur_fit(sharp, playing_again)
        check('Blur background blurs the world behind the GUI',
              on['blurred'] < 0.5 * on['sharp'], on)
        check('with it off the world behind the GUI stays as sharp as in play',
              off['sharp'] < 0.5 * off['blurred'], off)
        key('rshift')
        session.close()
        (session.root / 'assertions.json').write_text(
            json.dumps({'passed': True, 'checks': checks}, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
