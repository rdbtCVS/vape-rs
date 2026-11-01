#!/usr/bin/env python3
"""Target Info head preview, pot count and enchantment-aware damage.

In a fresh isolated survival world a stationary (NoAI) zombie stands two
blocks in front of the player. The scenario:
- hits it with a plain diamond sword, then holds a Sharpness V one: the
  damage strip follows the held weapon (6, then 9);
- a dispenser fires a healing splash potion (someone else's pot: +1),
  then the player throws one of its own (-1 again);
- opens the GUI, whose preview shows the local player with its head.

The panel is read from screenshot crops; result.json holds the checks.
"""
import argparse
import os

from PIL import Image, ImageChops

from native_session import NativeSession
from smoke_scenario import Checks
from target_info_scenario import aim, health, hit, prepare, summon

HEAL = 'minecraft:splash_potion[potion_contents={potion:"minecraft:healing"}]'
SHARP = 'minecraft:diamond_sword[enchantments={sharpness:5}]'
# Screen pixels; the session's GUI scale is 2. The panel at (300, 200) GUI
# pixels is 110 wide with its stats row, so it is clamped to the right edge
# (x 580-800) and ends where the toasts begin.
PANEL = (580, 404, 800, 494)
# In its stats row: the pots-used counter and the damage strip. Our own pot
# also harms the (undead) zombie, so the hits, combo and health bar move too.
POTS = (664, 412, 706, 432)
DAMAGE = (758, 412, 792, 434)
# Someone else's pot: the pinned host keeps a summoned splash potion as an
# entity it does not simulate, so a powered dispenser throws it instead.
DISPENSER = (-3, 100, 5)


def crop(session, name, box=PANEL):
    return Image.open(session.root / (name + '.png')).convert('RGB').crop(box)


def differs(session, a, b, box, threshold=48):
    diff = ImageChops.difference(crop(session, a, box), crop(session, b, box))
    return sum(1 for p in diff.getdata() if sum(p) > threshold)


def skin_pixels(session, name):
    """Warm, opaque skin-coloured pixels: the default skin's face."""
    image = crop(session, name)
    return sum(1 for r, g, b in image.getdata() if r > 120 and r > b + 30 and g > 60 and r - g < 90)


def run(resources, allow_stale):
    with NativeSession(resources, 'hud-target-info-extras-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {'hud.target-info': {'enabled': True}},
                      'settings': {'hud.target-info.hits-comparator': True,
                                   'hud.target-info.pots-used-comparator': True,
                                   'hud.target-info.damage-comparator': True,
                                   'hud.target-info.x': 300, 'hud.target-info.y': 200}})
        session.launch(creative=False)
        prepare(session, checks)
        summon(session, checks)
        x, y, z = DISPENSER
        session.call('command', f'setblock {x} {y} {z} minecraft:dispenser[facing=up]'
                     '{Items:[{Slot:0b,id:"minecraft:splash_potion",count:1,'
                     'components:{"minecraft:potion_contents":{potion:"minecraft:healing"}}}]}')
        session.call('give', 'minecraft:diamond_sword', '1')
        session.call('command', f'give @s {SHARP} 1')
        session.call('command', f'give @s {HEAL} 2')
        inventory = session.call('inventory')
        checks.check('a sword, a Sharpness V sword and heal pots are held', True, inventory)

        session.call('hotbar', '1')
        aim(session)
        start = health(session)
        hit(session)
        checks.check('the plain sword lands', (health(session) or 99) < (start or 0),
                     [start, health(session)])
        session.capture('plain-sword')
        session.call('hotbar', '2')
        session.call('wait', '4')
        session.capture('sharpness-sword')
        checks.check('the damage strip follows the Sharpness V sword',
                     differs(session, 'plain-sword', 'sharpness-sword', DAMAGE) > 0,
                     differs(session, 'plain-sword', 'sharpness-sword', DAMAGE))
        session.call('hotbar', '1')
        session.call('wait', '4')
        session.capture('plain-sword-again')
        checks.check('back on the plain sword the strip matches again',
                     differs(session, 'plain-sword', 'plain-sword-again', DAMAGE) == 0,
                     differs(session, 'plain-sword', 'plain-sword-again', DAMAGE))

        session.call('command', f'setblock {x - 1} {y} {z} minecraft:redstone_block')
        session.call('wait', '10')
        session.capture('other-pot')
        checks.check('a heal pot from someone else counts for the target',
                     differs(session, 'plain-sword-again', 'other-pot', POTS) > 0,
                     differs(session, 'plain-sword-again', 'other-pot', POTS))

        session.call('hotbar', '3')
        aim(session)
        session.call('mouse', 'right', 'down')
        session.call('wait', '1')
        session.call('mouse', 'right', 'up')
        session.call('wait', '10')
        session.call('hotbar', '1')
        session.call('wait', '4')
        session.capture('own-pot')
        left = session.call('inventory')
        checks.check('our own heal pot was thrown', True, left)
        checks.check('our own heal pot counts against us, back to the start',
                     differs(session, 'plain-sword-again', 'own-pot', POTS) == 0,
                     differs(session, 'plain-sword-again', 'own-pot', POTS))

        aim(session, 180.0)
        session.call('wait', '80')  # over the 3 s timeout
        session.call('key', 'rshift', 'tap')
        session.call('wait', '4')
        session.capture('gui-preview-face')
        checks.check('the GUI preview shows the local player\'s head',
                     skin_pixels(session, 'gui-preview-face') >= 40,
                     skin_pixels(session, 'gui-preview-face'))
        session.call('key', 'escape', 'tap')
        session.call('wait', '4')
        session.capture('gui-closed')
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?', default=os.environ.get('HORIZON_RESOURCES'))
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run(args.resources, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
