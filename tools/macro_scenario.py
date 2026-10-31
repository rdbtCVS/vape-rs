#!/usr/bin/env python3
"""Validate macros in native survival play: item macros by id, display name
and custom name, command macros, chords, try-next fallthrough, a macro on a
module-bound key, re-trigger inheritance, the fishing rod double click and
screen gating."""
import argparse
import json
from pathlib import Path
import time
from native_session import NativeSession


def macro(name, binds, delay=(100, 100), double_click=False, double_click_delay=(100, 200)):
    return {'name': name, 'binds': binds,
            'delay': {'min_ms': delay[0], 'max_ms': delay[1]}, 'double_click': double_click,
            'double_click_delay': {'min_ms': double_click_delay[0], 'max_ms': double_click_delay[1]}}


MACROS = [
    macro('Snowball', ['KeyR']),
    # Matches the egg renamed "Bolt" by its custom name, ignoring case.
    macro('bolt', ['KeyV']),
    macro('/time set 6000', ['KeyN']),
    macro('snowball', ['KeyU', 'KeyI']),
    # The first macro on B has no item, so the next one bound to B runs.
    macro('ender_pearl', ['KeyB']),
    macro('minecraft:snowball', ['KeyB']),
    # Shares the Coordinates bind; both act on the press.
    macro('Egg', ['KeyG']),
    macro('Snowball', ['KeyY'], delay=(1000, 1000)),
    macro('fishing rod', ['KeyZ'], delay=(1000, 1000), double_click=True,
          double_click_delay=(300, 300)),
]


def run(resources):
    with NativeSession(resources, 'macros-') as session:
        session.seed({'schema': 1, 'modules': {}, 'macros': MACROS,
                      'settings': {'hud.coordinates.binds': ['KeyG']}})
        session.launch()
        checks = []

        def command(text):
            session.call('command', text)
            session.call('wait', '2')

        def counts():
            return session.call('inventory')['counts']

        def selected():
            return session.call('state')['selected']

        def settle(seconds=1.5):
            time.sleep(seconds)
            session.call('wait', '2')

        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        session.call('fill', '-3', '99', '-3', '3', '99', '3', 'minecraft:stone')
        session.call('tp', '0.5', '100', '0.5')
        command('gamemode survival')
        command('gamerule advance_time false')
        command('clear @s')
        command('give @s minecraft:snowball 16')
        command('give @s minecraft:egg[minecraft:custom_name="Bolt"] 8')
        command('give @s minecraft:fishing_rod 1')
        session.call('wait', '4')
        hotbar = [s and s.get('item') for s in session.call('inventory')['hotbar']]
        assert hotbar[:3] == ['minecraft:snowball', 'minecraft:egg', 'minecraft:fishing_rod'], hotbar
        session.call('look', '0', '-10')
        session.call('hotbar', '6')
        session.call('wait', '2')

        before = counts()
        session.call('key', 'r', 'tap')
        settle()
        after = counts()
        assert after['minecraft:snowball'] == before['minecraft:snowball'] - 1, (before, after)
        assert selected() == 6
        session.capture('item-macro-restored')
        checks.append('item macro selects, uses once and restores the slot')

        session.call('key', 'v', 'tap')
        settle()
        assert counts()['minecraft:egg'] == before['minecraft:egg'] - 1
        assert selected() == 6
        checks.append('custom-named item matched ignoring case')

        session.call('time', '1000')
        session.call('key', 'n', 'tap')
        settle()
        day = session.call('state')['day_time'] % 24000
        assert day == 6000, day
        checks.append('command macro runs once')

        before = counts()
        session.call('key', 'i', 'tap')
        settle(0.5)
        assert counts() == before
        session.call('key', 'u', 'down')
        session.call('key', 'i', 'tap')
        session.call('key', 'u', 'up')
        settle()
        assert counts()['minecraft:snowball'] == before['minecraft:snowball'] - 1
        checks.append('chord macro needs every key')

        before = counts()
        session.call('key', 'b', 'tap')
        settle()
        assert counts()['minecraft:snowball'] == before['minecraft:snowball'] - 1
        checks.append('macro without its item falls through to the next bound macro')

        before = counts()
        session.call('key', 'g', 'tap')
        settle()
        assert counts()['minecraft:egg'] == before['minecraft:egg'] - 1
        assert session.saved()['modules']['hud.coordinates']['enabled'] is True
        session.capture('module-and-macro-share-a-key')
        checks.append('a module bind and a macro both act on one press')

        # Y's snowball macro waits a second before restoring; V replaces it,
        # keeps its original slot and restores that one.
        before = counts()
        session.call('key', 'y', 'tap')
        time.sleep(0.4)
        assert selected() == 1
        session.call('key', 'v', 'tap')
        settle(2.0)
        after = counts()
        assert after['minecraft:snowball'] == before['minecraft:snowball'] - 1, (before, after)
        assert after['minecraft:egg'] == before['minecraft:egg'] - 1, (before, after)
        assert selected() == 6
        checks.append('a replacing macro inherits the first original slot')

        # The fishing rod macro casts, reels in on the double click while the
        # rod is still held, then restores the slot.
        trace = []
        session.call('key', 'z', 'tap')
        start = time.monotonic()
        while time.monotonic() - start < 3.0:
            state = session.call('state')
            trace.append((round(time.monotonic() - start, 3), state['selected'], state['fishing'] is not None))
        cast = [i for i, (_, slot, hook) in enumerate(trace) if slot == 3 and hook]
        assert cast, trace
        reeled = [t for t in trace[cast[0]:] if t[1] == 3 and not t[2]]
        assert reeled, trace
        assert trace[-1][1] == 6, trace
        checks.append('fishing rod double click casts, reels in and restores')

        # No macro starts while a screen is open.
        before = counts()
        session.call('key', 'e', 'tap')
        session.call('wait', '2')
        assert session.call('state')['screen'] == 'inventory'
        session.call('key', 'r', 'tap')
        settle()
        session.capture('inventory-blocks-macros')
        session.call('key', 'esc', 'tap')
        session.call('wait', '2')
        assert counts() == before
        checks.append('an open screen blocks macro presses')

        session.close()
        session.launch()
        assert session.saved()['macros'] == MACROS
        checks.append('macros persist across restart')
        (session.root / 'assertions.json').write_text(json.dumps(
            {'passed': True, 'checks': checks, 'fishing_trace': trace}, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
