#!/usr/bin/env python3
"""InvCleaner, InventoryManager, AutoHotbar and ArmorSwitch in survival, on a
fresh home: each module's effect (items dropped, moved, sorted, condensed,
equipped) and its recovery when the player interferes: closing the
inventory mid-run, holding a stack on the cursor while an action is pending,
a full inventory, and an item picked up mid-run.

Every case checks conservation (inventory, armor, offhand, cursor and the
dropped item entities hold exactly the items the case started with: no
dupes, nothing lost), that the selected hotbar slot is unchanged, that the
screen ends closed, and that the on-key run turns the module off (no
repeated actions after the work is done)."""
import argparse
import base64
import json
from collections import Counter
from pathlib import Path
from native_session import NativeSession

IM = 'inventory.inventory-manager'
IC = 'inventory.inv-cleaner'
AH = 'inventory.auto-hotbar'
AS = 'inventory.armor-switch'


def b64(text):
    return base64.b64encode(text.encode()).decode()


PROFILE = {
    'name': 'Scenario',
    'armor_mode': 'Best armor',
    'slots': [
        {'slot': 0, 'itemFilter': 'swords', 'priority': 'weapon_damage'},
        {'slot': 1, 'itemFilter': 'minecraft:golden_apple'},
    ],
    'inventoryFilters': [
        {'itemFilter': 'minecraft:rotten_flesh', 'filterAction': 'Drop item'},
        {'itemFilter': 'minecraft:dirt', 'filterAction': 'Move into inventory'},
        {'itemFilter': 'minecraft:cobblestone', 'filterAction': 'Condense stacks'},
        # Custom item names (part C): a renamed stone is dropped, plain stone
        # is kept.
        {'itemFilter': 'minecraft:stone', 'filterAction': 'Drop item',
         'customRule': {'uuid': 'scenario-junk', 'name': 'Junk',
                        'conditions': [{'conditions': [
                            {'type': 'DISPLAY_NAME', 'operator': 'Contains',
                             'text': [b64('junk')]}]}]}},
    ],
}
HOTBAR_RULES = [
    {'slot': 0, 'item': 'minecraft:diamond_sword', 'anyMaterial': True, 'onlyBest': True},
    {'slot': 1, 'item': 'minecraft:golden_apple'},
    {'slot': 8, 'item': 'minecraft:cobblestone'},
]


def run(resources):
    with NativeSession(resources, 'inv-plan-') as session:
        session.seed({'schema': 1, 'modules': {}, 'settings': {
            f'{IC}.binds': ['KeyK'],
            f'{IC}.delay': [400, 400],
            f'{IM}.binds': ['KeyJ'],
            f'{IM}.click-delay': [150, 150],
            f'{IM}.profiles': json.dumps([PROFILE]),
            f'{IM}.profile': 'Scenario',
            f'{AH}.binds': ['KeyN'],
            f'{AH}.delay': 300,
            f'{AH}.hotbar': json.dumps(HOTBAR_RULES),
            f'{AS}.binds': ['KeyU'],
            f'{AS}.set-1': 'iron',
            f'{AS}.set-2': 'diamond',
            f'{AS}.delay': 150,
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

        def ground():
            out = Counter()
            for e in session.call('entities', '48')['entities']:
                if e.get('kind') == 'item':
                    out[e['item']['item']] += e['item']['count']
            return out

        def totals():
            return Counter(inventory()['counts']) + ground()

        def slots(inv=None):
            inv = inv or inventory()
            out = {}
            for entry in inv['hotbar'] + inv['main'] + inv['armor'] + [inv['offhand']]:
                if entry.get('item'):
                    out[entry['slot']] = (entry['item'], entry['count'])
            return out

        def stand(z, yaw):
            session.call('tp', '0.5', '100', z)
            session.call('look', yaw, '10')

        def setup(items):
            """`items`: (slot, item, count) with slot `hotbar.N`,
            `inventory.N` or `armor.X`."""
            command('kill @e[type=minecraft:item]')
            command('clear @s')
            stand('0.5', '0')
            for slot, item, count in items:
                command(f'item replace entity @s {slot} with {item} {count}')
            session.call('select', '3')
            before = totals()
            return before

        def finish(module, name, before, ticks=200):
            """Waits for the on-key run to end, steps away from the drops so
            they are not picked back up, and checks conservation."""
            assert poll(lambda: not enabled(module) and screen() == 'playing', ticks), (
                name, enabled(module), screen())
            stand('-9.5', '180')
            session.call('wait', '10')
            inv = inventory()
            after = Counter(inv['counts']) + ground()
            assert after == before, (name, dict(before), dict(after))
            assert inv['selected'] == 3, (name, inv['selected'])
            assert inv['carried'] is None, (name, inv['carried'])
            # Stable: no click after the run ended.
            session.call('wait', '20')
            assert slots() == slots(inv), name
            return inv

        def has(inv, item):
            return sum(c for i, c in slots(inv).values() if i == item)

        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        session.call('fill', '-12', '99', '-12', '12', '99', '12', 'minecraft:stone')
        command('gamemode survival')

        # 1. InvCleaner: worse sword, blacklisted stick, food and a poison
        # splash potion are dropped; the best sword, blocks and golden
        # apples stay.
        junk = [
            ('hotbar.0', 'minecraft:diamond_sword', 1),
            ('hotbar.1', 'minecraft:golden_apple', 3),
            ('inventory.0', 'minecraft:stone_sword', 1),
            ('inventory.1', 'minecraft:stick', 5),
            ('inventory.2', 'minecraft:cobblestone', 32),
            ('inventory.3', 'minecraft:rotten_flesh', 4),
            ('inventory.4',
             'minecraft:splash_potion[potion_contents={potion:"minecraft:poison"}]', 1),
        ]
        before = setup(junk)
        session.call('key', 'k', 'tap')
        inv = finish(IC, 'cleaner', before)
        dropped = ground()
        for item, count in [('minecraft:stone_sword', 1), ('minecraft:stick', 5),
                            ('minecraft:rotten_flesh', 4), ('minecraft:splash_potion', 1)]:
            assert dropped[item] == count and has(inv, item) == 0, (item, dict(dropped))
        for item, count in [('minecraft:diamond_sword', 1), ('minecraft:golden_apple', 3),
                            ('minecraft:cobblestone', 32)]:
            assert has(inv, item) == count, item
        checks['cleaner'] = {'dropped': dict(dropped)}

        # 2. InvCleaner interrupted: the player closes the inventory it
        # opened; the run ends at once, nothing more is thrown, and a
        # second run finishes the job.
        stand('0.5', '0')
        many = [(f'inventory.{i}', item, 1) for i, item in enumerate(
            ['minecraft:stick', 'minecraft:string', 'minecraft:flint', 'minecraft:feather',
             'minecraft:compass', 'minecraft:glass_bottle', 'minecraft:chest',
             'minecraft:anvil'])]
        before = setup(many)
        session.call('key', 'k', 'tap')
        assert poll(lambda: screen() == 'inventory', 20), screen()
        session.call('wait', '12')
        session.call('key', 'escape', 'tap')
        partial = finish(IC, 'cleaner-interrupted', before, 40)
        first = ground()
        thrown = sum(first.values())
        assert 0 <= thrown < 8, thrown
        # Clear the first drops so they cannot be picked back up.
        command('kill @e[type=minecraft:item]')
        stand('0.5', '0')
        session.call('key', 'k', 'tap')
        inv = finish(IC, 'cleaner-resumed', before - first)
        assert all(has(inv, item) == 0 for _, item, _ in many), slots(inv)
        assert sum(ground().values()) == 8 - thrown
        checks['cleaner_interrupted'] = {'thrown_before_close': thrown,
                                         'left_after_close': 8 - thrown}

        # 3. Inventory input during a pending run: the player picks the
        # last junk stack up onto the cursor and puts it in another slot.
        # Nothing is thrown while the cursor holds it; the stack is then
        # thrown from its new slot (no stale slot), and nothing else is
        # touched.
        before = setup([('inventory.0', 'minecraft:cobblestone', 16),
                        ('inventory.5', 'minecraft:stick', 3),
                        ('inventory.6', 'minecraft:string', 2),
                        ('inventory.7', 'minecraft:flint', 2)])
        session.call('key', 'k', 'tap')
        assert poll(lambda: screen() == 'inventory', 20), screen()
        session.call('click-slot', '16')
        held = inventory()['carried']
        session.call('wait', '4')
        held_from = ground()
        session.call('wait', '20')
        held_until = ground()
        assert held == {'item': 'minecraft:flint', 'count': 2}, held
        assert inventory()['carried'] == held and held_from == held_until, (
            dict(held_from), dict(held_until))
        session.call('click-slot', '30')
        inv = finish(IC, 'cleaner-cursor', before)
        assert has(inv, 'minecraft:flint') == 0 and ground()['minecraft:flint'] == 2
        assert has(inv, 'minecraft:stick') == 0 and has(inv, 'minecraft:string') == 0
        assert has(inv, 'minecraft:cobblestone') == 16
        checks['cleaner_cursor'] = {'held': held, 'ground_while_held': dict(held_until)}

        # 4. Item picked up mid-run: string lands next to the player while
        # the cleaner is working; it is picked up and thrown too.
        before = setup([(f'inventory.{i}', 'minecraft:stick', 1) for i in range(6)])
        session.call('key', 'k', 'tap')
        assert poll(lambda: screen() == 'inventory', 20), screen()
        command('summon minecraft:item ~ ~ ~ {Item:{id:"minecraft:string",count:3},'
                'PickupDelay:0}')
        before['minecraft:string'] += 3
        via = 'pickup'
        if not poll(lambda: totals()['minecraft:string'] == 3, 10):
            # The host's summon took no item data: hand the stack over
            # instead (still mid-run).
            via = 'give'
            session.call('give', 'minecraft:string', '3')
        picked = poll(lambda: inventory()['counts'].get('minecraft:string', 0) == 3, 10)
        inv = finish(IC, 'cleaner-pickup', before)
        assert has(inv, 'minecraft:string') == 0 and has(inv, 'minecraft:stick') == 0
        checks['cleaner_pickup'] = {'via': via, 'in_inventory_mid_run': picked, 'dropped': dict(ground())}

        # 5. InventoryManager: best sword to slot 1, golden apples to slot 2,
        # best armor worn, rotten flesh and the renamed stone dropped, dirt
        # moved off the hotbar, cobblestone condensed.
        before = setup([
            ('hotbar.0', 'minecraft:stone_sword', 1),
            ('hotbar.4', 'minecraft:dirt', 10),
            ('inventory.0', 'minecraft:diamond_sword', 1),
            ('inventory.1', 'minecraft:golden_apple', 2),
            ('inventory.2', 'minecraft:iron_chestplate', 1),
            ('inventory.3', 'minecraft:rotten_flesh', 3),
            ('inventory.4', 'minecraft:cobblestone', 20),
            ('inventory.5', 'minecraft:cobblestone', 20),
            ('inventory.6', 'minecraft:stone[custom_name="Junk Stone"]', 7),
            ('inventory.7', 'minecraft:stone', 9),
        ])
        session.call('key', 'j', 'tap')
        inv = finish(IM, 'manager', before)
        hotbar = {e['slot']: e.get('item') for e in inv['hotbar']}
        armor = {e['part']: e.get('item') for e in inv['armor']}
        assert hotbar[0] == 'minecraft:diamond_sword', hotbar
        assert hotbar[1] == 'minecraft:golden_apple', hotbar
        assert hotbar[4] is None and has(inv, 'minecraft:dirt') == 10, hotbar
        assert armor['chest'] == 'minecraft:iron_chestplate', armor
        stacks = [c for i, c in slots(inv).values() if i == 'minecraft:cobblestone']
        assert stacks == [40], stacks
        dropped = ground()
        assert dropped['minecraft:rotten_flesh'] == 3 and dropped['minecraft:stone'] == 7
        assert has(inv, 'minecraft:stone') == 9
        checks['manager'] = {'hotbar': hotbar, 'armor': armor, 'cobblestone': stacks,
                             'dropped': dict(dropped)}

        # 6. InventoryManager with a full inventory: the sword swap needs no
        # free slot; "Move into inventory" has nowhere to go and is skipped
        # (no repeated clicks), and the run ends.
        filler = [(f'inventory.{i}', 'minecraft:oak_planks', 64) for i in range(27)]
        filler[10] = ('inventory.10', 'minecraft:diamond_sword', 1)
        hot = [(f'hotbar.{i}', 'minecraft:oak_log', 64) for i in range(9)]
        hot[0] = ('hotbar.0', 'minecraft:stone_sword', 1)
        hot[5] = ('hotbar.5', 'minecraft:dirt', 64)
        before = setup(filler + hot)
        session.call('key', 'j', 'tap')
        inv = finish(IM, 'manager-full', before)
        hotbar = {e['slot']: e.get('item') for e in inv['hotbar']}
        assert hotbar[0] == 'minecraft:diamond_sword' and hotbar[5] == 'minecraft:dirt', hotbar
        assert slots(inv)[19] == ('minecraft:stone_sword', 1), slots(inv).get(19)
        assert not ground()
        checks['manager_full'] = {'hotbar0': hotbar[0], 'slot19': slots(inv)[19]}

        # 7. AutoHotbar: best sword (any material) to slot 1, golden apples
        # to slot 2, cobblestone to slot 9.
        before = setup([
            ('hotbar.0', 'minecraft:wooden_sword', 1),
            ('hotbar.8', 'minecraft:oak_planks', 16),
            ('inventory.0', 'minecraft:iron_sword', 1),
            ('inventory.1', 'minecraft:stone_sword', 1),
            ('inventory.2', 'minecraft:golden_apple', 5),
            ('inventory.3', 'minecraft:cobblestone', 30),
        ])
        session.call('key', 'n', 'tap')
        inv = finish(AH, 'hotbar', before)
        hotbar = {e['slot']: e.get('item') for e in inv['hotbar']}
        assert hotbar[0] == 'minecraft:iron_sword', hotbar
        assert hotbar[1] == 'minecraft:golden_apple', hotbar
        assert hotbar[8] == 'minecraft:cobblestone', hotbar
        assert has(inv, 'minecraft:oak_planks') == 16 and not ground()
        checks['hotbar'] = {'hotbar': hotbar}

        # 8. AutoHotbar with the cursor busy: the player holds the
        # cobblestone (the last rule's stack) and drops it into another
        # slot; the run waits, then moves it from where it now is.
        before = setup([('inventory.0', 'minecraft:iron_sword', 1),
                        ('inventory.2', 'minecraft:golden_apple', 5),
                        ('inventory.3', 'minecraft:cobblestone', 30)])
        session.call('key', 'n', 'tap')
        assert poll(lambda: screen() == 'inventory', 20), screen()
        session.call('click-slot', '12')
        held = inventory()['carried']
        session.call('wait', '10')
        session.call('click-slot', '20')
        inv = finish(AH, 'hotbar-cursor', before)
        hotbar = {e['slot']: e.get('item') for e in inv['hotbar']}
        assert held == {'item': 'minecraft:cobblestone', 'count': 30}, held
        assert hotbar[0] == 'minecraft:iron_sword', hotbar
        assert hotbar[1] == 'minecraft:golden_apple', hotbar
        assert hotbar[8] == 'minecraft:cobblestone', hotbar
        checks['hotbar_cursor'] = {'held': held, 'hotbar': hotbar}

        # 9. ArmorSwitch: iron worn, diamond stored, full inventory; the sets
        # trade places piece by piece, then switch back.
        sets = [('armor.head', 'minecraft:iron_helmet', 1),
                ('armor.chest', 'minecraft:iron_chestplate', 1),
                ('armor.legs', 'minecraft:iron_leggings', 1),
                ('armor.feet', 'minecraft:iron_boots', 1)]
        stored = [('inventory.3', 'minecraft:diamond_helmet', 1),
                  ('inventory.8', 'minecraft:diamond_chestplate', 1),
                  ('inventory.15', 'minecraft:diamond_leggings', 1),
                  ('inventory.26', 'minecraft:diamond_boots', 1)]
        used = {s for s, _, _ in stored}
        fill = [(f'inventory.{i}', 'minecraft:oak_planks', 64) for i in range(27)
                if f'inventory.{i}' not in used]
        fill += [(f'hotbar.{i}', 'minecraft:oak_log', 64) for i in range(9)]
        before = setup(sets + stored + fill)
        session.call('key', 'u', 'tap')
        inv = finish(AS, 'switch', before)
        armor = {e['part']: e.get('item') for e in inv['armor']}
        assert all(v.startswith('minecraft:diamond_') for v in armor.values()), armor
        main = slots(inv)
        for slot, piece in [(12, 'helmet'), (17, 'chestplate'), (24, 'leggings'),
                            (35, 'boots')]:
            assert main[slot] == (f'minecraft:iron_{piece}', 1), (slot, main.get(slot))
        stand('0.5', '0')
        session.call('key', 'u', 'tap')
        inv = finish(AS, 'switch-back', before)
        armor = {e['part']: e.get('item') for e in inv['armor']}
        assert all(v.startswith('minecraft:iron_') for v in armor.values()), armor
        checks['switch'] = {'there_and_back': True}

        # 10. ArmorSwitch interrupted by closing the inventory mid-swap
        # (room in the inventory): the carried piece goes back, nothing is
        # lost, and a second run completes the switch.
        before = setup(sets + stored)
        session.call('key', 'u', 'tap')
        assert poll(lambda: screen() == 'inventory', 20), screen()
        session.call('wait', '8')
        session.call('key', 'escape', 'tap')
        partial = finish(AS, 'switch-interrupted', before, 40)
        assert not ground()
        worn = {e['part']: e.get('item') for e in partial['armor']}
        assert None not in worn.values(), worn
        session.call('key', 'u', 'tap')
        inv = finish(AS, 'switch-resumed', before)
        armor = {e['part']: e.get('item') for e in inv['armor']}
        # The rerun reads the worn material from the first worn piece (the
        # helmet) and needs the whole other set in slots 9-44, as the
        # reference's hasEquippedArmor/findArmorSlots do: an untouched iron
        # set switches to diamond; after a completed helmet swap the iron
        # chest, legs and feet are worn, not stored, so it turns off
        # without a click.
        if all(v.startswith('minecraft:iron_') for v in worn.values()):
            assert all(v.startswith('minecraft:diamond_') for v in armor.values()), armor
        else:
            assert worn['head'].startswith('minecraft:diamond_'), worn
            assert armor == worn and slots(inv) == slots(partial), (armor, worn)
        checks['switch_interrupted'] = {'armor_after_interrupt': worn,
                                        'armor_after_rerun': armor}

        session.capture('final')
        (session.root / 'assertions.json').write_text(
            json.dumps({'passed': True, 'checks': checks}, indent=2) + '\n')
        print(json.dumps(checks))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    run(parser.parse_args().resources)
