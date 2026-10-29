#!/usr/bin/env python3
"""Validate real session boundaries, held cleanup and retained preferences."""
import argparse
import json
from pathlib import Path
import re
import time
from native_session import NativeSession


def run(resources):
    with NativeSession(resources, 'lifecycle-') as session:
        session.seed({'schema': 1, 'modules': {'hud.coordinates': {'enabled': True}},
                      'settings': {'hud.fps.activation': 'hold'}})
        def held():
            session.call('key', 'f9', 'down')
            session.call('wait', '2')
            assert session.saved()['modules']['hud.fps']['enabled'] is True
        def released():
            assert session.saved()['modules']['hud.fps']['enabled'] is False
            assert session.saved()['modules']['hud.coordinates']['enabled'] is True
        def state_when(predicate):
            for _ in range(100):
                try:
                    state = session.call('state')
                    if predicate(state):
                        return state
                except RuntimeError as error:
                    if 'save is in progress' not in str(error):
                        raise
                time.sleep(0.1)
            raise AssertionError('session transition did not finish')
        session.launch()
        session.capture('started')
        held()
        session.call('chat', '/execute in minecraft:the_nether run tp @s 0 90 0')
        state_when(lambda s: s['dimension'] == 'minecraft:the_nether' and s['player'] is not None)
        session.call('wait-ready', '40')
        session.call('wait', '2')
        session.capture('dimension-changed')
        released()
        session.call('key', 'f9', 'up')
        held()
        session.call('chat', '/kill @s')
        state_when(lambda s: s['screen'] == 'dead')
        session.capture('dead')
        released()
        time.sleep(1.1)  # Vanilla's death-screen buttons wait before accepting input.
        session.call('click-at', '200', '157')
        state_when(lambda s: s['screen'] == 'playing' and s['player'] is not None)
        session.call('wait-ready', '40')
        session.call('wait', '2')
        session.capture('respawned')
        released()
        session.call('key', 'f9', 'up')
        held()
        session.call('key', 'esc', 'tap')
        session.capture('pause-menu')
        released()
        session.call('click-at', '200', '189')
        state_when(lambda s: s['screen'] == 'title')
        session.capture('world-closed')
        session.call('open', 'NextEpoch', '1', 'creative')
        session.call('wait-ready', '40')
        session.call('wait', '2')
        session.capture('next-world')
        released()
        session.call('key', 'f9', 'up')
        held()
        session.call('key', 'f9', 'up')
        session.call('wait', '2')
        released()
        entries = [(int(epoch), reason) for epoch, reason in re.findall(
            r'\[vape-rs\] session epoch (\d+): ([a-z-]+)', (session.root / 'game.log').read_text())]
        assert entries and [e for e, _ in entries] == sorted({e for e, _ in entries}), entries
        reasons = {r for _, r in entries}
        assert {'dimension', 'death', 'respawn', 'world-close', 'world-open', 'world-ready'} <= reasons, entries
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True,
            'checks': ['dimension invalidation', 'death/respawn cleanup', 'pause releases held input',
                       'world close/reopen cleanup', 'toggle preferences survive boundaries',
                       'held bindings work in next world', 'monotonic session generations'],
            'epochs': entries}, indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
