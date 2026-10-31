#!/usr/bin/env python3
"""Refill, ChestSteal and AutoTotem recovery through the bounded inventory
gate, in survival: a run that a player interrupts (closing the screen) ends
cleanly, frees the container claim, and a later run completes."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession


def ids(entries):
    return [e.get('item') for e in entries]


def run(resources):
    with NativeSession(resources, 'inv-rec-') as session:
        session.seed({'schema': 1, 'modules': {}, 'settings': {
            'inventory.refill.binds': ['KeyR'],
            'inventory.refill.type': 'soup',
            'inventory.refill.delay': [200, 200],
            'world.chest-steal.binds': ['KeyY'],
            'world.chest-steal.click-delay': [250, 250],
            'inventory.auto-totem.binds': ['KeyO'],
            'inventory.auto-totem.random-slot': True,
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

        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        session.call('fill', '-3', '99', '-3', '3', '99', '3', 'minecraft:stone')
        session.call('tp', '0.5', '100', '0.5')
        command('gamemode survival')
        command('clear @s')

        def soup_setup():
            command('clear @s')
            for slot in range(6):
                command(f'item replace entity @s hotbar.{slot} with minecraft:stone')
            for slot in range(5):
                command(f'item replace entity @s inventory.{slot} with minecraft:mushroom_stew')

        # Refill interrupted: closing the inventory it opened ends the run
        # (module off) after at most a click or two.
        soup_setup()
        session.call('key', 'r', 'tap')
        assert poll(lambda: screen() == 'inventory', 20), screen()
        session.call('wait', '3')
        session.call('key', 'escape', 'tap')
        assert poll(lambda: not enabled('inventory.refill'), 20)
        partial = ids(inventory()['hotbar']).count('minecraft:mushroom_stew')
        assert partial < 3, partial
        assert screen() == 'playing'
        checks['refill_interrupted'] = {'hotbar_soups': partial}
        # Refill completes: every empty hotbar slot is filled, the inventory
        # closes and the module disables itself.
        session.call('key', 'r', 'tap')
        assert poll(lambda: not enabled('inventory.refill') and screen() == 'playing', 120)
        hotbar = ids(inventory()['hotbar'])
        main = ids(inventory()['main']).count('minecraft:mushroom_stew')
        assert hotbar.count('minecraft:mushroom_stew') == 3, hotbar
        assert main == 2, main
        checks['refill_complete'] = {'hotbar': hotbar, 'main_soups': main}
        session.capture('refill')

        # ChestSteal: six stacks, closed after a click, then reopened.
        command('clear @s')
        command('setblock 0 100 2 minecraft:chest')
        for slot in range(6):
            command(f'item replace block 0 100 2 container.{slot} with minecraft:diamond 1')
        session.call('key', 'y', 'tap')
        session.call('wait', '2')
        session.call('interact', '0', '100', '2')
        assert poll(lambda: screen() == 'container', 20), screen()
        session.call('wait', '7')
        session.call('key', 'escape', 'tap')
        session.call('wait', '10')
        first = inventory()['counts'].get('minecraft:diamond', 0)
        assert 0 < first < 6, first
        assert screen() == 'playing'
        session.call('interact', '0', '100', '2')
        assert poll(lambda: inventory()['counts'].get('minecraft:diamond', 0) == 6, 120)
        assert poll(lambda: screen() == 'playing', 40), 'ChestSteal closes the emptied chest'
        checks['chest_steal'] = {'after_interrupt': first, 'after_reopen': 6}
        session.call('key', 'y', 'tap')

        # AutoTotem with Random slot: opens, equips one of two totems, closes.
        command('clear @s')
        command('item replace entity @s inventory.3 with minecraft:totem_of_undying')
        command('item replace entity @s inventory.20 with minecraft:totem_of_undying')
        session.call('key', 'o', 'tap')
        assert poll(lambda: (inventory()['offhand'].get('item') == 'minecraft:totem_of_undying'
                             and screen() == 'playing'), 80)
        left = ids(inventory()['main']).count('minecraft:totem_of_undying')
        assert left == 1, left
        checks['auto_totem'] = {'offhand': 'minecraft:totem_of_undying', 'main_totems': left}
        session.call('key', 'o', 'tap')
        session.close()
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': checks}, indent=2) + '\n')
        print(json.dumps(checks))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    run(parser.parse_args().resources)
