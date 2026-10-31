#!/usr/bin/env python3
"""Throwpot, ThrowDebuff and AutoHeal (Legit) against real splash potions in
survival: each sequence selects, uses and restores the hotbar through the
host's ordinary input paths, and a manual slot change abandons a sequence."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession

HEALING = 'minecraft:splash_potion[minecraft:potion_contents={potion:"minecraft:healing"}]'
POISON = 'minecraft:splash_potion[minecraft:potion_contents={potion:"minecraft:poison"}]'
WEAKNESS = 'minecraft:splash_potion[minecraft:potion_contents={potion:"minecraft:weakness"}]'


def item_id(stack):
    if not stack:
        return None
    return stack.get('item') or stack.get('id')


def run(resources):
    with NativeSession(resources, 'inventory-throw-') as session:
        # ThrowDebuff is hidden by default; unhide it as the GUI would.
        session.seed({'schema': 1, 'modules': {'inventory.throw-debuff': {'visible': True}}, 'settings': {
            'inventory.throw-pot.binds': ['KeyK'],
            'inventory.throw-pot.type': 'pots',
            'inventory.throw-debuff.binds': ['KeyL'],
            'inventory.auto-heal.binds': ['KeyH'],
            'inventory.auto-heal.use-custom-items': True,
            'inventory.auto-heal.healing-items': ['minecraft:splash_potion'],
            'inventory.auto-heal.health': 12,
        }})
        session.launch()
        checks = {}

        def command(text):
            session.call('command', text)
            session.call('wait', '2')

        def hotbar():
            return [item_id(s) for s in session.call('inventory')['hotbar']]

        def health():
            return session.call('state')['player']['health']

        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        session.call('fill', '-3', '99', '-3', '3', '99', '3', 'minecraft:stone')
        session.call('tp', '0.5', '100', '0.5')
        command('gamemode survival')
        command('clear @s')
        command('effect clear @s')
        session.call('look', '0', '90')  # straight down: splashes hit the player
        # Throwpot dynamic: 10 HP, three Instant Health I pots (4 HP each);
        # the third would heal past 20 and is kept.
        for _ in range(3):
            command('give @s ' + HEALING + ' 1')
        command('give @s minecraft:stick 1')
        session.call('hotbar', '5')
        command('damage @s 10 minecraft:generic')
        before = health()
        initial = hotbar()
        session.call('key', 'k', 'tap')
        session.call('wait', '40')
        after = hotbar()
        state = session.call('state')
        assert state['player']['health'] > before, (before, state['player'])
        assert after.count('minecraft:splash_potion') == 1, (initial, after)
        assert state['selected'] == 5, state['selected']  # 1-based, as `hotbar`
        assert session.saved()['modules'].get('inventory.throw-pot', {}).get('enabled') is not True
        checks['throwpot'] = {'health_before': before, 'health_after': state['player']['health'],
                              'hotbar_before': initial, 'hotbar_after': after}
        session.capture('throwpot')
        # ThrowDebuff one-of-each: two poison and one weakness, looking up so
        # nothing lands on the player; one poison is kept.
        command('clear @s')
        for text in (POISON, POISON, WEAKNESS):
            command('give @s ' + text + ' 1')
        session.call('look', '0', '-60')
        session.call('hotbar', '6')
        session.call('key', 'l', 'tap')
        session.call('wait', '40')
        after = hotbar()
        assert after.count('minecraft:splash_potion') == 1, after
        assert session.call('state')['selected'] == 6
        checks['throwdebuff'] = {'hotbar_after': after}
        # AutoHeal Legit with splash potions as custom healing items: heals
        # under 12 HP, waits its Delay, then heals again.
        command('clear @s')
        command('effect clear @s')
        for _ in range(4):
            command('give @s ' + HEALING + ' 1')
        session.call('look', '0', '90')
        session.call('hotbar', '7')
        session.call('wait', '10')
        command('damage @s 14 minecraft:generic')
        low = health()
        session.call('key', 'h', 'tap')
        session.call('wait', '60')
        healed = health()
        left = hotbar().count('minecraft:splash_potion')
        assert healed > 12 or left <= 2, (low, healed, left)
        assert left < 4, left
        assert session.call('state')['selected'] == 7
        assert session.saved()['modules']['inventory.auto-heal']['enabled'] is True
        checks['autoheal'] = {'health_low': low, 'health_after': healed, 'pots_left': left}
        session.capture('autoheal')
        # Disabling stops it: no further pots are used at low health.
        session.call('key', 'h', 'tap')
        session.call('wait', '4')
        command('damage @s 8 minecraft:generic')
        kept = hotbar().count('minecraft:splash_potion')
        session.call('wait', '40')
        assert hotbar().count('minecraft:splash_potion') == kept
        checks['autoheal_disabled'] = {'pots_kept': kept}
        session.close()
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': checks}, indent=2) + '\n')
        print(json.dumps(checks))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    run(parser.parse_args().resources)
