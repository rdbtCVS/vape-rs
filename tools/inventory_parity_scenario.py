#!/usr/bin/env python3
"""ChestSteal "Best only" and AutoTotem "Silent open" in survival, on a fresh
home.

ChestSteal: the player holds a diamond sword and wears an iron helmet; the
chest holds a stone sword (worse than the held one), a diamond helmet (an
upgrade) and cobblestone. Best only takes the helmet and the blocks, leaves
the sword in the chest and closes it.

AutoTotem: Silent open with a fixed 400 ms "Silent move delay". The player
walks with W held and the use key held over snowballs; the totem is swapped
into the offhand with no screen shown, movement and use are held for the
session plus one delay, then walking and throwing resume on their own."""
import argparse
import json
import math
import time
from pathlib import Path
from native_session import NativeSession

CS = 'world.chest-steal'
AT = 'inventory.auto-totem'
TOTEM = 'minecraft:totem_of_undying'


def run(resources):
    with NativeSession(resources, 'inv-parity-') as session:
        session.seed({'schema': 1, 'modules': {}, 'settings': {
            f'{CS}.binds': ['KeyY'],
            f'{CS}.best-only': True,
            f'{CS}.click-delay': [150, 150],
            f'{AT}.binds': ['KeyO'],
            f'{AT}.silent-open': True,
            f'{AT}.silent-move-delay': [400, 400],
            f'{AT}.delay': [200, 200],
            f'{AT}.extra-randomization': False,
            f'{AT}.random-slot': False,
        }})
        session.launch()
        checks = {}

        def command(text):
            session.call('command', text)
            session.call('wait', '2')

        def inventory():
            return session.call('inventory')

        def screen():
            return session.call('state')['screen']

        def enabled(module):
            return session.saved()['modules'].get(module, {}).get('enabled') is True

        def poll(predicate, ticks=80):
            for _ in range(ticks // 2):
                if predicate():
                    return True
                session.call('wait', '2')
            return predicate()

        def has(inv, item):
            return sum(e.get('count', 0) for e in inv['hotbar'] + inv['main'] + inv['armor']
                       + [inv['offhand']] if e.get('item') == item)

        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        session.call('fill', '-14', '99', '-6', '14', '99', '6', 'minecraft:stone')
        session.call('tp', '0.5', '100', '0.5')
        command('gamemode survival')
        command('clear @s')

        # 1. ChestSteal Best only. Survival moves go through the server.
        command('tp @s 0.5 100 0.5')
        session.call('look', '0', '30')
        command('item replace entity @s hotbar.0 with minecraft:diamond_sword')
        command('item replace entity @s armor.head with minecraft:iron_helmet')
        session.call('select', '1')
        command('setblock 0 100 2 minecraft:chest')
        command('item replace block 0 100 2 container.0 with minecraft:stone_sword')
        command('item replace block 0 100 2 container.4 with minecraft:diamond_helmet')
        command('item replace block 0 100 2 container.9 with minecraft:cobblestone 32')
        before_chest = session.call('container', '0', '100', '2')
        session.call('key', 'y', 'tap')
        session.call('wait', '2')
        session.call('interact', '0', '100', '2')
        assert poll(lambda: screen() == 'container', 20), screen()
        assert poll(lambda: screen() == 'playing', 120), 'Best only closes the chest'
        session.call('wait', '10')
        inv = inventory()
        after_chest = session.call('container', '0', '100', '2')
        assert has(inv, 'minecraft:diamond_helmet') == 1, inv['counts']
        assert has(inv, 'minecraft:cobblestone') == 32, inv['counts']
        assert has(inv, 'minecraft:stone_sword') == 0, inv['counts']
        assert has(inv, 'minecraft:diamond_sword') == 1 and has(inv, 'minecraft:iron_helmet') == 1
        assert inv['carried'] is None
        left = json.dumps(after_chest)
        assert 'minecraft:stone_sword' in left and 'diamond_helmet' not in left \
            and 'cobblestone' not in left, after_chest
        checks['chest_steal_best_only'] = {
            'chest_before': before_chest, 'chest_after': after_chest,
            'inventory_counts': inv['counts']}
        # Inspected captures: the player's inventory, then the chest with
        # ChestSteal off.
        session.call('key', 'e', 'tap')
        assert poll(lambda: screen() == 'inventory', 20), screen()
        session.capture('chest-steal-inventory')
        session.call('key', 'escape', 'tap')
        session.call('key', 'y', 'tap')
        assert poll(lambda: not enabled(CS), 20)
        session.call('interact', '0', '100', '2')
        assert poll(lambda: screen() == 'container', 20), screen()
        session.call('wait', '10')
        session.capture('chest-steal-chest-left')
        session.call('key', 'escape', 'tap')
        assert poll(lambda: screen() == 'playing', 20)

        # 2. AutoTotem Silent open while walking and throwing.
        command('setblock 0 100 2 minecraft:air')
        command('clear @s')
        command('item replace entity @s hotbar.0 with minecraft:snowball 16')
        command(f'item replace entity @s inventory.5 with {TOTEM}')
        session.call('select', '1')
        command('tp @s -12.5 100 0.5')
        session.call('look', '-90', '0')
        session.call('wait', '10')
        session.call('key', 'w', 'down')
        session.call('wait', '10')
        session.call('mouse', 'right', 'down')
        session.call('wait', '6')
        samples = []

        def sample(phase):
            state = session.call('state')
            p = state.get('player') or {}
            v = p.get('velocity') or [0, 0, 0]
            inv = inventory()
            samples.append({
                't': round(time.time() - start, 3), 'phase': phase,
                'screen': state.get('screen'),
                'speed': round(math.hypot(v[0], v[2]), 4),
                'x': round((p.get('position') or [0])[0], 3),
                'snowballs': inv['counts'].get('minecraft:snowball', 0),
                'offhand': inv['offhand'].get('item'),
                'totems_main': sum(1 for e in inv['main'] if e.get('item') == TOTEM)})

        start = time.time()
        for _ in range(6):
            sample('before')
        session.call('key', 'o', 'tap')
        deadline = time.time() + 3.0
        while time.time() < deadline:
            sample('after')
        session.call('mouse', 'right', 'up')
        session.call('key', 'w', 'up')
        session.call('wait', '10')
        (session.root / 'totem-samples.json').write_text(json.dumps(samples, indent=1) + '\n')

        before = [s for s in samples if s['phase'] == 'before']
        after = [s for s in samples if s['phase'] == 'after']
        assert all(s['speed'] > 0.1 for s in before), before
        assert before[-1]['snowballs'] < 16, 'the held use key throws before the session'
        assert all(s['screen'] == 'playing' for s in samples), \
            sorted({s['screen'] for s in samples})
        # The hold: a run of samples standing still with W held, during
        # which no snowball is thrown.
        still = [i for i, s in enumerate(after) if s['speed'] < 0.02]
        assert still, after
        first, last = still[0], still[-1]
        window = after[first:last + 1]
        assert all(s['speed'] < 0.05 for s in window), window
        assert len({s['snowballs'] for s in window}) == 1, window
        hold = window[-1]['t'] - window[0]['t']
        assert 0.4 <= hold <= 2.0, hold
        # The swap happened inside the session; walking and throwing resume.
        assert after[-1]['offhand'] == TOTEM and after[-1]['totems_main'] == 0, after[-1]
        resumed = after[last + 1:]
        assert resumed and max(s['speed'] for s in resumed) > 0.1, resumed
        assert resumed[-1]['snowballs'] < window[-1]['snowballs'], resumed
        equipped_at = next(s['t'] for s in after if s['offhand'] == TOTEM)
        assert window[0]['t'] - 0.25 <= equipped_at <= window[-1]['t'], (equipped_at, window)
        assert screen() == 'playing'
        session.capture('totem-silent-equipped')
        checks['auto_totem_silent_open'] = {
            'screens_seen': sorted({s['screen'] for s in samples}),
            'hold_seconds': round(hold, 3), 'hold_samples': len(window),
            'snowballs_during_hold': window[0]['snowballs'],
            'snowballs_end': after[-1]['snowballs'],
            'equipped_at': equipped_at, 'window': [window[0]['t'], window[-1]['t']],
            'resumed_speed': max(s['speed'] for s in resumed)}
        session.call('key', 'o', 'tap')
        assert poll(lambda: not enabled(AT), 20)
        session.close()
        (session.root / 'assertions.json').write_text(
            json.dumps({'passed': True, 'checks': checks}, indent=2) + '\n')
        print(json.dumps(checks))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    run(parser.parse_args().resources)
