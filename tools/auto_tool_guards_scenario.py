#!/usr/bin/env python3
"""Native guard, weapon and interrupted-deadline scenarios in disposable worlds."""
import argparse
import json
from pathlib import Path
import time
from native_session import NativeSession


def run(resources):
    def seed(session, settings):
        session.seed({'schema': 1, 'modules': {'world.auto-tool': {'enabled': True}},
                      'settings': {'world.auto-tool.binds': ['KeyJ'], **settings}})
    def command(session, text):
        session.call('command', text)
        session.call('wait', '2')
    def prepare(session, items=None):
        session.launch()
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        session.call('fill', '-2', '99', '-2', '2', '99', '5', 'minecraft:stone')
        session.call('tp', '0.5', '100', '0.5')
        command(session, 'gamemode survival')
        command(session, 'clear @s')
        for item in items or ['stick', 'diamond_pickaxe', 'golden_pickaxe', 'golden_apple']:
            session.call('give', 'minecraft:' + item, '1')
            session.call('wait', '2')
        session.call('hotbar', '1')
        session.call('fill', '0', '100', '3', '0', '102', '3', 'minecraft:obsidian')
        session.call('wait', '3')
        session.call('look-at', 'block', '0', '101', '3')
    def selected(session):
        return session.call('state')['selected']
    def passed(session, checks):
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': checks}, indent=2)+'\n')
    with NativeSession(resources, 'auto-tool-pending-') as session:
        seed(session, {'world.auto-tool.swap-to-delay': 500, 'world.auto-tool.swap-back': True})
        prepare(session)
        session.call('mouse', 'left', 'down')
        session.call('wait', '2')
        assert selected(session) == 1
        session.call('key', 'j', 'tap')
        session.call('wait', '15')
        assert selected(session) == 1
        session.capture('disabled-before-deadline')
        session.call('mouse', 'left', 'up')
        session.call('key', 'j', 'tap')
        session.call('mouse', 'left', 'down')
        session.call('wait', '2')
        session.call('key', 'rshift', 'tap')
        session.capture('paused-pending')
        session.call('key', 'rshift', 'tap')
        session.call('mouse', 'left', 'up')
        session.call('wait', '15')
        assert selected(session) == 1
        session.capture('menu-cancelled-deadline')
        # A cleared nether landing: the generated terrain would otherwise put
        # a block under the crosshair at random, and mining it is new work
        # whose own swap-back is correct.
        for text in ('forceload add -3 -3 7 7', 'fill -3 99 -3 3 104 7 minecraft:air',
                     'fill -3 98 -3 3 98 7 minecraft:obsidian'):
            command(session, 'execute in minecraft:the_nether run ' + text)
        session.call('mouse', 'left', 'down')
        session.call('wait', '2')
        session.call('command', 'execute in minecraft:the_nether run tp @s 0 100 0')
        for _ in range(100):
            state = session.call('state')
            if state['dimension'] == 'minecraft:the_nether' and state['player'] is not None:
                break
            time.sleep(0.05)
        else:
            raise AssertionError('dimension transition did not finish')
        session.call('wait-ready', '40')
        after = selected(session)
        session.call('mouse', 'left', 'up')
        session.call('wait', '20')
        assert selected(session) == after
        session.capture('next-dimension-no-replay')
        passed(session, ['disable cancels pending swap', 'menu cancels pending deadline', 'dimension transition drops old work and restoration'])
    with NativeSession(resources, 'auto-tool-guards-') as session:
        seed(session, {'world.auto-tool.swap-to-delay': 0, 'world.auto-tool.only-sneaking': True,
                       'world.auto-tool.not-use': True, 'world.auto-tool.not-combat': True})
        prepare(session)
        session.call('wait', '30')  # Let fixture travel/fall damage guard expire.
        session.call('mouse', 'left', 'down')
        session.call('wait', '4')
        assert selected(session) == 1
        session.call('key', 'shift', 'down')
        session.call('wait', '4')
        assert selected(session) == 2
        session.capture('sneaking-required')
        session.call('mouse', 'left', 'up')
        session.call('hotbar', '4')
        session.call('mouse', 'right', 'down')
        session.call('wait', '2')
        assert session.call('state')['using'] is not None
        session.call('mouse', 'left', 'down')
        session.call('wait', '4')
        assert selected(session) == 4
        session.capture('using-item-blocks-swap')
        session.call('mouse', 'left', 'up')
        session.call('mouse', 'right', 'up')
        session.call('hotbar', '1')
        command(session, 'damage @s 1 minecraft:generic')
        session.call('mouse', 'left', 'down')
        session.call('wait', '4')
        assert selected(session) == 1
        session.capture('recent-damage-blocks-swap')
        session.call('wait', '25')
        assert selected(session) == 2
        session.capture('combat-guard-expired')
        session.call('mouse', 'left', 'up')
        session.call('key', 'shift', 'up')
        passed(session, ['only while sneaking', 'actual item-use guard', 'recent damage guard', 'guard cooldown expires without threat'])
    with NativeSession(resources, 'auto-tool-no-mouse-') as session:
        seed(session, {'world.auto-tool.require-mouse': False})
        prepare(session)
        before = session.call('state')['swings']
        session.call('hotbar', '1')
        session.call('wait', '4')
        assert selected(session) == 2
        assert session.call('state')['swings'] == before
        session.capture('selection-without-attack')
        passed(session, ['require mouse down can be disabled without generating attacks'])
    for mode, settings, expected in [
        ('instant', {}, 4),
        ('disabled', {'world.auto-tool.swap-weapon': False}, 1),
        ('delayed', {'world.auto-tool.instant-swap': False}, 4),
        ('enchanted', {}, 3),
    ]:
        with NativeSession(resources, 'auto-tool-weapon-' + mode + '-') as session:
            seed(session, {'world.auto-tool.swap-to-delay': 500, **settings})
            prepare(session, ['stick', 'wooden_sword', 'diamond_sword', 'iron_axe'])
            if mode == 'enchanted':
                session.call('hotbar', '3')
                session.call('wait', '3')
                command(session, 'enchant @s minecraft:sharpness 5')
                assert 'sharpness' in session.call('inventory')['hotbar'][2].get('components', '')
                session.call('hotbar', '1')
            session.call('fill', '0', '100', '3', '0', '102', '3', 'minecraft:air')
            command(session, 'summon minecraft:armor_stand 0.5 100 3.0')
            session.call('look-at', '0.5', '101.1', '3.0')
            session.call('mouse', 'left', 'down')
            session.call('wait', '2')
            if mode == 'delayed':
                assert selected(session) == 1
                session.capture('waiting-for-weapon-delay')
            elif mode != 'disabled':
                assert selected(session) == expected
            session.call('wait', '12')
            assert selected(session) == expected
            session.capture('weapon-' + mode)
            session.call('mouse', 'left', 'up')
            passed(session, ['weapon selection ' + mode, 'host sword/tool damage and enchantment scoring'])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
