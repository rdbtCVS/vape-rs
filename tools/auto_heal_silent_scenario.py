#!/usr/bin/env python3
"""AutoHeal Silent in survival on a fresh home.

1. Custom items (the reference's onTick): an Instant Health splash potion as
   a custom healing item in hotbar slot 4 while slot 1 (snowballs) stays
   selected. Nothing is used while the inventory screen is open; after it
   closes the potion is used once with no visible slot change, and the
   server's held slot is back on the snowballs (a right click throws one).
2. Potions with Replace (PreMotion/PostMotion): an Instant Health splash
   potion in the main inventory is swapped into "Slot" 6 and thrown while
   that motion packet looks down (pitch 88.99); the camera never moves."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession

AH = 'inventory.auto-heal'
HEALING = 'minecraft:splash_potion[minecraft:potion_contents={potion:"minecraft:healing"}]'
POTION = 'minecraft:splash_potion'


def run(resources):
    with NativeSession(resources, 'inv-heal-') as session:
        # AutoHeal is hidden by default; unhide it as the GUI would.
        session.seed({'schema': 1, 'modules': {AH: {'visible': True}}, 'settings': {
            f'{AH}.binds': ['KeyH'],
            f'{AH}.mode': 'silent',
            f'{AH}.use-custom-items': True,
            f'{AH}.healing-items': [POTION],
            f'{AH}.health': 14,
            f'{AH}.delay': 500,
        }})
        session.launch()
        checks = {}

        def command(text):
            session.call('command', text)
            session.call('wait', '2')

        def inventory():
            return session.call('inventory')

        def player():
            return session.call('state')['player']

        def screen():
            return session.call('state')['screen']

        def enabled():
            return session.saved()['modules'].get(AH, {}).get('enabled') is True

        def poll(predicate, ticks=80):
            for _ in range(ticks // 2):
                if predicate():
                    return True
                session.call('wait', '2')
            return predicate()

        def count(inv, item):
            return sum(e.get('count', 0) for e in inv['hotbar'] + inv['main'] + inv['armor']
                       + [inv['offhand']] if e.get('item') == item)

        def messages():
            lines = session.call('messages', '20').get('chat') or []
            return [m if isinstance(m, str) else json.dumps(m) for m in reversed(lines)]

        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        session.call('fill', '-4', '99', '-4', '4', '99', '4', 'minecraft:stone')
        session.call('tp', '0.5', '100', '0.5')
        command('gamemode survival')
        # Only the potions heal: health checks are not natural regeneration.
        command('gamerule natural_health_regeneration false')
        command('clear @s')
        command('effect clear @s')
        session.call('look', '0', '0')

        # 1. Silent custom item, held slot restored.
        command('item replace entity @s hotbar.0 with minecraft:snowball 16')
        command('item replace entity @s hotbar.3 with ' + HEALING)
        session.call('hotbar', '1')
        session.call('wait', '6')
        session.call('key', 'h', 'tap')
        assert poll(enabled, 20)
        session.call('key', 'e', 'tap')
        assert poll(lambda: screen() == 'inventory', 20), screen()
        command('damage @s 10 minecraft:generic')
        low = player()['health']
        assert low <= 10.5, low
        session.call('wait', '20')
        # A screen stops the onTick use, as in the reference.
        held_open = count(inventory(), POTION)
        assert held_open == 1, inventory()['counts']
        session.call('key', 'escape', 'tap')
        assert poll(lambda: screen() == 'playing', 20)
        assert poll(lambda: count(inventory(), POTION) == 0, 40), inventory()['counts']
        session.call('wait', '10')
        state = session.call('state')
        healed = state['player']['health']
        inv = inventory()
        assert healed >= low + 3.5, (low, healed)
        assert state['selected'] == 1, state['selected']
        assert inv['counts'].get('minecraft:snowball') == 16, inv['counts']
        assert inv['carried'] is None
        session.capture('silent-custom-used')
        # The server holds slot 1 again: a right click throws a snowball and
        # nothing else is used.
        session.call('look', '0', '-30')
        session.call('mouse', 'right', 'tap')
        assert poll(lambda: inventory()['counts'].get('minecraft:snowball') == 15, 20), \
            inventory()['counts']
        command('data get entity @s SelectedItemSlot')
        checks['silent_custom_item'] = {
            'health_low': low, 'health_after': healed,
            'potions_while_screen_open': held_open,
            'selected_after': state['selected'],
            'snowballs_after_right_click': 15,
            'server_reply': messages()[-3:]}

        # 2. Potions with Replace: main inventory potion to Slot 6, thrown
        # looking down by the motion packet only.
        # Relaunch the same home with the Potions settings.
        session.call('key', 'h', 'tap')
        assert poll(lambda: not enabled(), 20)
        session.call('quit')
        session.process.wait(timeout=15)
        session.log.close()
        session.process = None
        document = session.saved()
        document['settings'].update({
            f'{AH}.use-custom-items': False, f'{AH}.potions': True,
            f'{AH}.replace': True, f'{AH}.slot': 6, f'{AH}.health': 17})
        session.seed(document)
        session.launch()
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        command('gamemode survival')
        command('clear @s')
        command('effect clear @s')
        session.call('wait', '40')
        command('item replace entity @s hotbar.0 with minecraft:stick')
        command('item replace entity @s inventory.13 with ' + HEALING)
        session.call('hotbar', '1')
        session.call('look', '0', '-10')
        command('damage @s 10 minecraft:generic')
        low = player()['health']
        view = player()
        session.call('key', 'h', 'tap')
        assert poll(enabled, 20)
        samples = []
        for _ in range(30):
            p = player()
            samples.append({'pitch': p['pitch'], 'yaw': p['yaw'], 'health': p['health'],
                            'potions': count(inventory(), POTION),
                            'selected': session.call('state')['selected']})
            if samples[-1]['potions'] == 0 and p['health'] >= low + 3.5:
                break
            session.call('wait', '2')
        (session.root / 'potion-samples.json').write_text(json.dumps(samples, indent=1) + '\n')
        last = samples[-1]
        assert last['potions'] == 0 and last['health'] >= low + 3.5, samples
        assert all(abs(s['pitch'] - view['pitch']) < 0.5 and abs(s['yaw'] - view['yaw']) < 0.5
                   for s in samples), samples
        assert all(s['selected'] == 1 for s in samples), samples
        session.call('wait', '10')
        inv = inventory()
        assert inv['hotbar'][0].get('item') == 'minecraft:stick', inv['hotbar']
        assert count(inv, POTION) == 0 and inv['carried'] is None, inv['counts']
        session.capture('silent-potion-thrown')
        checks['silent_potions_replace'] = {
            'health_low': low, 'health_after': last['health'],
            'camera': [view['yaw'], view['pitch']], 'samples': len(samples),
            'selected': last['selected']}
        session.call('key', 'h', 'tap')
        assert poll(lambda: not enabled(), 20)
        session.close()
        (session.root / 'assertions.json').write_text(
            json.dumps({'passed': True, 'checks': checks}, indent=2) + '\n')
        print(json.dumps(checks))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    run(parser.parse_args().resources)
