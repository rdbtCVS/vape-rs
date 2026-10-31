#!/usr/bin/env python3
"""Freecam host scenarios in fresh isolated homes (single player, survival).

The player stands on a stone pad at (0.5, 100, 0.5) facing south (yaw 0)
between a red wool wall to the south (z=5) and a lime wool wall to the north
(z=-6). One fresh session per variant:

- Defaults (Spawn Fake on, Move Fake off): the bind frees the camera at the
  eye of the body left behind (as the reference's fake, the first frame is
  inside its head). Held S backs the camera off (the player stays) and the
  body shows in front of the red wall; a turn of 180 degrees shows the lime
  wall while the player's yaw stays 0. Held Space and W move the camera,
  not the player. The bind again restores the view (yaw 0, the start
  frame) and W walks the player, with no released key replayed.
- Spawn Fake off: the start view is the player's own and the backed-off
  view shows no body.
- Move Fake: the up arrow walks the player south while W still moves only
  the camera, and the player's yaw stays 0.

Results go to result.json in each session's home.
"""
import argparse
import os

from PIL import Image, ImageChops

from native_session import NativeSession
from pearl_scenario import FLOOR, call, pad
from smoke_scenario import Checks, enabled, state

ID = 'render.freecam'
BIND = 'KeyJ'


def toggle(session):
    session.call('key', BIND[3:].lower(), 'tap')
    session.call('wait', '3')


def pixels(image):
    return image.get_flattened_data() if hasattr(image, 'get_flattened_data') else image.getdata()


def changed(a, b, box=None, threshold=48):
    a, b = Image.open(a).convert('RGB'), Image.open(b).convert('RGB')
    if box is not None:
        a, b = a.crop(box), b.crop(box)
    return sum(1 for p in pixels(ImageChops.difference(a, b)) if sum(p) > threshold)


def colours(path, box=None):
    """Red-wool and lime-wool pixels (whatever the face shading), over the
    frame or over box (left, top, right, bottom)."""
    image = Image.open(path).convert('RGB')
    if box is not None:
        image = image.crop(box)
    red = lime = 0
    for r, g, b in pixels(image):
        if r > 60 and r > 1.8 * g and r > 1.8 * b:
            red += 1
        elif g > 80 and g > 1.3 * r and g > 1.8 * b:
            lime += 1
    return {'red': red, 'lime': lime}


def body(path, box):
    """Pixels in box that are neither red wool nor grey stone: the player
    model (skin, shirt, trousers) against the red wall and the pad."""
    count = 0
    for r, g, b in pixels(Image.open(path).convert('RGB').crop(box)):
        red = r > 60 and r > 1.8 * g and r > 1.8 * b
        grey = max(r, g, b) - min(r, g, b) < 24
        count += not red and not grey
    return count


# The middle of the 800x600 frame, above the HUD, where the body stands in
# the backed-off view.
MIDDLE = (300, 150, 500, 500)
# The world view without the toasts (top right, bottom right), the chat
# line and the HUD.
VIEW = (0, 70, 600, 480)


def player(session):
    return state(session)['player']


def hold(session, key, ticks):
    session.call('key', key, 'down')
    session.call('wait', str(ticks))
    session.call('key', key, 'up')
    session.call('wait', '3')


def moved(a, b):
    return max(abs(x - y) for x, y in zip(a['position'], b['position']))


def setup(session, checks, settings):
    session.seed({'schema': 1, 'modules': {}, 'settings': {f'{ID}.binds': [BIND], **settings}})
    session.launch(creative=False)
    session.call('time', '6000')
    session.call('tp', '0.5', str(FLOOR + 1), '0.5')
    session.call('wait-ready', '40')
    pad(session, checks)
    call(session, 'fill', -6, FLOOR + 1, 5, 6, FLOOR + 7, 5, 'minecraft:red_wool')
    call(session, 'fill', -6, FLOOR + 1, -6, 6, FLOOR + 7, -6, 'minecraft:lime_wool')
    session.call('tp', '0.5', str(FLOOR + 1), '0.5')
    session.call('look', '0', '0')
    session.call('wait', '20')
    start = player(session)
    checks.check('the player stands on the pad facing the red wall',
                 abs(start['position'][1] - (FLOOR + 1)) < 0.01 and abs(start['yaw']) < 0.01,
                 {'position': start['position'], 'yaw': start['yaw']})
    return start


def defaults(resources, allow_stale):
    with NativeSession(resources, 'freecam-defaults-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        start = setup(session, checks, {})
        root = session.root
        session.capture('before')
        toggle(session)
        checks.check('the bind enables Freecam', enabled(session, ID))
        session.call('wait', '3')
        session.capture('on')
        # The reference's fake stands where the player stands and the camera
        # starts at its eye: the first frame is inside the fake's head, whose
        # skin covers the view (its darker shades read as red to colours()).
        checks.check('Spawn Fake: the camera starts at the eye of the body left behind',
                     body(root / 'before.png', MIDDLE) < 2000 and body(root / 'on.png', MIDDLE) > 5000
                     and changed(root / 'before.png', root / 'on.png', VIEW) > 150000,
                     {'before': body(root / 'before.png', MIDDLE), 'on': body(root / 'on.png', MIDDLE),
                      'changed': changed(root / 'before.png', root / 'on.png', VIEW)})
        # S backs the camera off north (0.6 blocks a tick at Speed 3).
        hold(session, 's', 8)
        p = player(session)
        session.capture('backed')
        checks.check('held S moves the camera, not the player',
                     moved(p, start) < 0.01 and changed(root / 'before.png', root / 'backed.png') > 20000,
                     {'player_moved': moved(p, start),
                      'changed': changed(root / 'before.png', root / 'backed.png')})
        checks.check('Spawn Fake: the player body stands between the camera and the red wall',
                     body(root / 'backed.png', MIDDLE) > 2000,
                     {'body': body(root / 'backed.png', MIDDLE), 'middle': colours(root / 'backed.png', MIDDLE)})
        before = colours(root / 'before.png')
        session.call('turn', '180', '0')
        session.call('wait', '3')
        session.capture('turned')
        turned = colours(root / 'turned.png')
        p = player(session)
        checks.check('a turn turns the camera to the lime wall, not the player',
                     turned['lime'] > 20000 and turned['red'] < 2000 and before['red'] > 20000
                     and abs(p['yaw'] - start['yaw']) < 0.01 and abs(p['pitch']) < 0.01,
                     {'before': before, 'turned': turned, 'yaw': p['yaw'], 'pitch': p['pitch']})
        session.call('turn', '-180', '0')
        session.call('wait', '3')
        session.capture('back')
        hold(session, 'space', 6)
        p = player(session)
        session.capture('raised')
        checks.check('held Space raises the camera, the player does not jump',
                     moved(p, start) < 0.01
                     and changed(root / 'back.png', root / 'raised.png') > 5000,
                     {'player_moved': moved(p, start),
                      'changed': changed(root / 'back.png', root / 'raised.png')})
        hold(session, 'w', 8)
        p = player(session)
        checks.check('held W moves the camera, not the player', moved(p, start) < 0.01,
                     {'player_moved': moved(p, start), 'yaw': p['yaw']})
        toggle(session)
        session.call('wait', '3')
        p = player(session)
        session.capture('off')
        checks.check('the bind disables Freecam and the view returns to the player',
                     not enabled(session, ID) and abs(p['yaw'] - start['yaw']) < 0.01
                     and moved(p, start) < 0.01
                     and changed(root / 'before.png', root / 'off.png', VIEW) < 4000,
                     {'enabled': enabled(session, ID), 'yaw': p['yaw'], 'player_moved': moved(p, start),
                      'changed': changed(root / 'before.png', root / 'off.png', VIEW)})
        hold(session, 'w', 10)
        p = player(session)
        checks.check('after the disable W walks the player again (and nothing held replays)',
                     p['position'][2] - start['position'][2] > 1.0
                     and abs(p['position'][1] - start['position'][1]) < 0.01,
                     p['position'])
        checks.write()
        return checks


def no_fake(resources, allow_stale):
    with NativeSession(resources, 'freecam-nofake-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        start = setup(session, checks, {f'{ID}.spawn-fake': False})
        root = session.root
        session.capture('before')
        toggle(session)
        checks.check('the bind enables Freecam', enabled(session, ID))
        session.call('wait', '3')
        session.capture('on')
        checks.check('Spawn Fake off: enabling keeps the view at the eye, no body in it',
                     changed(root / 'before.png', root / 'on.png', VIEW) < 4000
                     and body(root / 'on.png', MIDDLE) < 2000,
                     {'changed': changed(root / 'before.png', root / 'on.png', VIEW),
                      'body': body(root / 'on.png', MIDDLE)})
        hold(session, 's', 8)
        p = player(session)
        session.capture('backed')
        checks.check('Spawn Fake off: no body between the camera and the red wall',
                     moved(p, start) < 0.01 and body(root / 'backed.png', MIDDLE) < 500,
                     {'player_moved': moved(p, start), 'body': body(root / 'backed.png', MIDDLE),
                      'middle': colours(root / 'backed.png', MIDDLE)})
        toggle(session)
        checks.check('the bind disables Freecam', not enabled(session, ID))
        checks.write()
        return checks


def move_fake(resources, allow_stale):
    with NativeSession(resources, 'freecam-movefake-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        start = setup(session, checks, {f'{ID}.move-fake': True})
        toggle(session)
        checks.check('the bind enables Freecam', enabled(session, ID))
        hold(session, 'w', 8)
        p = player(session)
        checks.check('Move Fake: held W still moves only the camera', moved(p, start) < 0.01,
                     {'player_moved': moved(p, start)})
        hold(session, 'up', 12)
        p = player(session)
        checks.check('Move Fake: the up arrow walks the player south, its yaw unchanged',
                     p['position'][2] - start['position'][2] > 1.0
                     and abs(p['position'][0] - start['position'][0]) < 0.05
                     and abs(p['yaw'] - start['yaw']) < 0.01,
                     {'position': p['position'], 'yaw': p['yaw']})
        toggle(session)
        checks.check('the bind disables Freecam', not enabled(session, ID))
        checks.write()
        return checks


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--resources', default=os.environ.get('HORIZON_RESOURCES'))
    parser.add_argument('--allow-stale', action='store_true')
    parser.add_argument('--only', choices=['defaults', 'no-fake', 'move-fake'])
    args = parser.parse_args()
    runs = {'defaults': defaults, 'no-fake': no_fake, 'move-fake': move_fake}
    failed = False
    for name, run in runs.items():
        if args.only and args.only != name:
            continue
        checks = run(args.resources, args.allow_stale)
        passed = sum(1 for c in checks.results if c['ok'])
        print(f'{name}: {passed}/{len(checks.results)} {checks.root}')
        failed |= passed != len(checks.results)
    raise SystemExit(1 if failed else 0)


if __name__ == '__main__':
    main()
