#!/usr/bin/env python3
"""Validate hotbar actions against real mining, guards and user interruption."""
import argparse
import json
from pathlib import Path
import time
from native_session import NativeSession


def run(resources, in_frame=False):
    with NativeSession(resources, 'auto-tool-', in_frame) as session:
        session.seed({'schema': 1, 'modules': {'world.auto-tool': {'enabled': True}},
                      'settings': {'world.auto-tool.swap-back': True,
                                   'world.auto-tool.swap-back-delay': 150,
                                   'world.auto-tool.binds': ['KeyJ']}})
        session.launch()
        def command(text):
            session.call('command', text)
            session.call('wait', '2')
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        session.call('fill', '-2', '99', '-2', '2', '99', '5', 'minecraft:stone')
        session.call('tp', '0.5', '100', '0.5')
        command('gamemode survival')
        command('clear @s')
        for item in ['stick', 'diamond_pickaxe', 'golden_pickaxe']:
            session.call('give', 'minecraft:' + item, '1')
            session.call('wait', '2')
        session.call('hotbar', '1')
        session.call('fill', '0', '100', '3', '0', '102', '3', 'minecraft:stone')
        session.call('wait', '3')
        session.call('look-at', 'block', '0', '101', '3')
        session.call('wait', '4')
        assert session.call('state')['selected'] == 1
        session.capture('require-mouse')
        result = session.call('break', '0', '101', '3', '--timeout', '8')
        assert result['drops'].get('minecraft:cobblestone', 0) >= 1, result
        assert result['ticks'] < 40, result
        session.call('wait', '5')
        inventory = session.call('inventory')
        assert inventory['selected'] == 1, inventory
        assert inventory['hotbar'][1].get('damage', 0) >= 1, inventory
        assert inventory['hotbar'][2].get('damage', 0) == 0, inventory
        session.capture('mined-and-restored')
        session.call('fill', '0', '100', '3', '0', '102', '3', 'minecraft:obsidian')
        session.call('wait', '3')
        session.call('look-at', 'block', '0', '101', '3')
        session.call('mouse', 'left', 'down')
        session.call('wait', '4')
        assert session.call('state')['selected'] == 2
        session.call('hotbar', '3')
        session.call('wait', '4')
        assert session.call('state')['selected'] == 3
        session.call('mouse', 'left', 'up')
        session.call('wait', '10')
        assert session.call('state')['selected'] == 3
        session.capture('manual-selection-wins')
        session.call('key', 'f10', 'tap')
        session.call('wait', '2')
        assert session.saved()['modules']['world.auto-tool']['enabled'] is False
        session.call('hotbar', '1')
        session.call('mouse', 'left', 'down')
        session.call('wait', '4')
        assert session.call('state')['selected'] == 1
        session.call('mouse', 'left', 'up')
        session.call('key', 'rshift', 'tap')
        session.capture('gui')
        session.call('click-at', '40', '156')
        session.capture('world-category')
        session.call('click-at', '150', '116')
        session.capture('enabled-in-gui')
        assert session.saved()['modules']['world.auto-tool']['enabled'] is True
        session.call('key', 'rshift', 'tap')
        session.close()
        session.launch()
        assert session.saved()['modules']['world.auto-tool']['enabled'] is True
        assert session.saved()['settings']['world.auto-tool.swap-back'] is True
        session.capture('after-restart')
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': [
            'require mouse down', 'actual stone mining produces cobblestone',
            'diamond tool used instead of penalized golden tool', 'delayed swap back',
            'manual hotbar choice cancels restore and wins', 'panic stops tool changes',
            'GUI enable', 'restart preferences'], 'mining': result}, indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--in-frame', action='store_true')
    args = parser.parse_args()
    run(args.resources, args.in_frame)
