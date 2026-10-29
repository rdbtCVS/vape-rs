#!/usr/bin/env python3
"""Capture a live chest with StorageESP enabled and disabled in an isolated world."""
import argparse
from pathlib import Path
from native_session import NativeSession


def run(resources):
    with NativeSession(resources, 'storage-esp-') as session:
        session.seed({'schema': 1, 'modules': {'render.storage-esp': {'enabled': True}},
                      'settings': {'render.storage-esp.binds': ['KeyJ']}})
        session.launch()
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        session.call('fill', '-2', '99', '-2', '2', '99', '5', 'minecraft:stone')
        session.call('fill', '0', '100', '3', '0', '100', '3', 'minecraft:chest')
        session.call('tp', '0.5', '100', '0.5')
        session.call('look', '0', '20')
        session.call('wait', '10')
        session.capture('enabled')
        session.call('key', 'j', 'tap')
        session.call('wait', '4')
        assert session.saved()['modules']['render.storage-esp']['enabled'] is False
        session.capture('disabled')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
