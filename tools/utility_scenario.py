#!/usr/bin/env python3
"""Inventory and utility modules in a fresh isolated survival world on the
integrated server, checked against the host's own inventory and screens.

* Refill: soup in the main inventory and free hotbar slots; enabling it opens
  the inventory, moves every soup into the hotbar, closes and turns itself
  off. Off, nothing moves.
* ChestSteal: a chest with three stacks; opened without the module nothing
  moves, with it every stack arrives in the inventory and the chest closes.
  Closing the chest mid-steal stops the clicks.
* AutoFish: a Lure III rod cast into a pool; without the module the bite
  passes and nothing is caught in 40 s, with it the rod is reeled in on the
  bite and a catch arrives.

Each module is toggled through its bind. Results go to result.json and
measurements.json.
"""
import argparse
import json
import os
import time

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 149
BINDS = {
    'inventory.refill': 'KeyV',
    'world.chest-steal': 'KeyK',
    'utility.auto-fish': 'KeyP',
}
CHEST = (0, FLOOR + 1, 3)
CLOSED = (None, 'playing')


class Utility(Run):
    def inv(self):
        return self.call('inventory')

    def counts(self):
        return self.inv().get('counts', {})

    def screen(self):
        return self.call('state').get('screen')

    def hotbar_items(self):
        return [(s or {}).get('item') or (s or {}).get('id') for s in self.inv().get('hotbar', [])]


def refill(r):
    r.reset()
    r.call('chat', '/clear @s')
    r.wait(4)
    # Hotbar slots 0-4 hold stone, 5-8 are free; four soups in the main
    # inventory.
    for slot in range(5):
        r.call('chat', f'/item replace entity @s hotbar.{slot} with minecraft:stone')
    for slot in range(4):
        r.call('chat', f'/item replace entity @s inventory.{slot * 3} with minecraft:mushroom_stew')
    r.wait(10)
    before = r.hotbar_items()
    r.wait(20)
    idle = r.hotbar_items()
    r.toggle('inventory.refill', True)
    t0 = time.monotonic()
    opened = False
    while time.monotonic() - t0 < 6:
        r.wait(2)
        opened = opened or r.screen() not in CLOSED
        if not r.enabled('inventory.refill') and r.screen() in CLOSED:
            break
    took = time.monotonic() - t0
    after = r.hotbar_items()
    stews = sum(1 for i in after if i and 'mushroom_stew' in i)
    r.m['refill'] = {'before': before, 'idle': idle, 'after': after, 'opened_screen': opened, 'seconds': took,
                     'screen_after': r.screen(), 'counts': r.counts()}
    r.checks.check('Refill off: the soups stay in the main inventory',
                   not any(i and 'mushroom_stew' in i for i in idle), idle)
    r.checks.check('Refill: all four soups move into the free hotbar slots', stews == 4, after)
    r.checks.check('Refill: the stone stays where it was', after[:5] == before[:5], {'before': before, 'after': after})
    r.checks.check('Refill: it closes the inventory and turns itself off',
                   not r.enabled('inventory.refill') and r.screen() in CLOSED,
                   {'screen': r.screen(), 'opened': opened})
    r.call('chat', '/clear @s')


def chest(r, items):
    x, y, z = CHEST
    r.call('chat', f'/setblock {x} {y} {z} minecraft:air')
    slots = ','.join(f'{{Slot:{i}b,id:"minecraft:{item}",count:{n}}}' for i, (item, n) in enumerate(items))
    r.call('chat', f'/setblock {x} {y} {z} minecraft:chest{{Items:[{slots}]}}')
    r.wait(6)


def open_chest(r):
    x, y, z = CHEST
    r.call('look-at-block', x, y, z)
    r.call('interact', x, y, z)
    for _ in range(20):
        r.wait(2)
        if r.screen() not in CLOSED:
            return r.screen()
    return None


def close(r):
    if r.screen() not in CLOSED:
        r.call('close')
        r.wait(4)


def chest_steal(r):
    items = [('diamond', 5), ('iron_ingot', 7), ('gold_ingot', 3)]
    r.reset()
    r.call('chat', '/clear @s')
    chest(r, items)
    shown = open_chest(r)
    r.wait(30)
    off = r.counts()
    close(r)
    r.toggle('world.chest-steal', True)
    r.reset()
    chest(r, items)
    shown_on = open_chest(r)
    t0 = time.monotonic()
    closed_at = None
    while time.monotonic() - t0 < 6:
        r.wait(2)
        if r.screen() in CLOSED:
            closed_at = time.monotonic() - t0
            break
    on = r.counts()
    close(r)
    # Interrupted mid-steal: a chest of many stacks closed by the player
    # after the first stack arrives (binds do not reach a menu screen). The
    # clicks stop with the screen and the rest stays in the chest.
    r.call('chat', '/clear @s')
    many = [('cobblestone', 1 + i) for i in range(12)]
    chest(r, many)
    open_chest(r)
    for _ in range(40):
        r.wait(1)
        if r.counts().get('minecraft:cobblestone', 0) > 0:
            break
    r.call('close')
    r.wait(2)
    mid = r.counts().get('minecraft:cobblestone', 0)
    r.wait(30)
    later = r.counts().get('minecraft:cobblestone', 0)
    still_on = r.enabled('world.chest-steal')
    r.toggle('world.chest-steal', False)
    r.call('chat', f'/setblock {CHEST[0]} {CHEST[1]} {CHEST[2]} minecraft:air')
    want = {f'minecraft:{i}': n for i, n in items}
    r.m['chest_steal'] = {'screen': [shown, shown_on], 'off': off, 'on': on, 'closed_after_s': closed_at,
                          'closed_mid': {'still_enabled': still_on, 'at_close': mid, 'later': later, 'total': sum(range(1, 13))}}
    r.checks.check('ChestSteal off: the open chest is left alone',
                   shown is not None and not any(off.get(k) for k in want), {'screen': shown, 'counts': off})
    r.checks.check('ChestSteal: every stack arrives in the inventory',
                   all(on.get(k) == n for k, n in want.items()), {'want': want, 'got': on})
    r.checks.check('ChestSteal: the chest is closed once empty', closed_at is not None, closed_at)
    r.checks.check('ChestSteal closed mid-steal: the clicks stop with the screen, the rest stays in the chest',
                   0 < mid < sum(range(1, 13)) and later == mid, r.m['chest_steal']['closed_mid'])


def pool(r):
    # A 5x5, 3-deep pool in front of the player, walled in stone.
    r.fill(-3, FLOOR - 3, 2, 3, FLOOR, 8, 'minecraft:stone')
    r.fill(-2, FLOOR - 2, 3, 2, FLOOR, 7, 'minecraft:water')
    r.wait(10)


def catches(r):
    return {k: v for k, v in r.counts().items() if k != 'minecraft:fishing_rod'}


def fish(r, seconds=40):
    """Cast once into the pool; (catch counts after, whether the rod was
    reeled, seconds until the first catch or None)."""
    r.reset()
    r.call('select', 'fishing_rod')
    r.call('look', '0', '35')
    r.wait(4)
    before = catches(r)
    r.call('mouse', 'right', 'click')
    t0 = time.monotonic()
    first = None
    while time.monotonic() - t0 < seconds:
        time.sleep(1)
        now = catches(r)
        if now != before:
            first = time.monotonic() - t0
            break
    after = catches(r)
    # Reel in whatever is out.
    hook = r.call('state').get('fishing') or r.call('state').get('hook')
    return after, before, first, hook


def auto_fish(r):
    pool(r)
    r.call('chat', '/clear @s')
    r.call('chat', '/give @s minecraft:fishing_rod[minecraft:enchantments={lure:3}]')
    r.wait(10)
    off_after, off_before, off_first, off_hook = fish(r)
    # Reel in the idle bobber before the next cast.
    r.call('mouse', 'right', 'click')
    r.wait(10)
    r.toggle('utility.auto-fish', True)
    on_after, on_before, on_first, on_hook = fish(r)
    r.toggle('utility.auto-fish', False)
    r.m['auto_fish'] = {'off': [off_before, off_after, off_first, off_hook],
                        'on': [on_before, on_after, on_first, on_hook]}
    r.checks.check('AutoFish off: a cast catches nothing in 40 s (the bite passes)', off_first is None,
                   {'before': off_before, 'after': off_after})
    r.checks.check('AutoFish: the bite is reeled in and a catch arrives', on_first is not None,
                   {'before': on_before, 'after': on_after, 'seconds': on_first})


def run(resources, allow_stale):
    with NativeSession(resources, 'utility-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {f'{m}.binds': [k] for m, k in BINDS.items()}})
        session.launch(creative=True)
        session.call('chat', '/gamemode survival')
        session.call('chat', '/difficulty peaceful')
        session.call('time', '6000')
        session.call('chat', '/gamerule advance_time false')
        r = Utility(session, checks, BINDS)
        r.wait(20)
        r.arena()
        r.call('chat', '/effect give @s minecraft:resistance 100000 4 true')
        r.call('chat', '/effect give @s minecraft:saturation 100000 4 true')
        for step in (refill, chest_steal, auto_fish):
            try:
                step(r)
            except Exception as error:  # report and go on with the next module
                checks.check(f'{step.__name__} ran to completion', False, repr(error))
        alive = session.call('state')
        checks.check('still in the world (no kick)', alive.get('player') is not None, alive.get('screen'))
        log = (session.root / 'game.log').read_text(errors='replace')
        checks.check('no panics in the log', 'panicked' not in log and 'poisoned' not in log)
        (session.root / 'measurements.json').write_text(json.dumps(r.m, indent=2, default=str))
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}',
              session.root)
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
