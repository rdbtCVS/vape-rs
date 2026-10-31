#!/usr/bin/env python3
"""Inventory Blur host scenario in a fresh isolated world.

Opens the survival inventory with Inventory Blur on and off, and the creative
inventory with it on, capturing each so the world behind the screen can be
compared (blurred with the module, sharp without). The module state is checked
through the saved settings. Results go to result.json.
"""
import argparse
import os

from native_session import NativeSession
from smoke_scenario import Checks
from target_info_scenario import prepare


def scene(session):
    session.call('time', '6000')
    session.call('fill', '-2', '100', '3', '2', '102', '3', 'minecraft:oak_planks')
    session.call('fill', '0', '100', '3', '0', '102', '3', 'minecraft:gold_block')
    session.call('tp', '0.5', '100', '0.5')
    session.call('look', '0', '0')
    session.call('wait', '10')


def inventory(session, label):
    session.call('key', 'e', 'tap')
    session.call('wait', '5')
    session.capture(label)
    session.call('key', 'escape', 'tap')
    session.call('wait', '5')


def run(resources, allow_stale):
    with NativeSession(resources, 'hud-inventory-blur-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {'render.inventory-blur': {'enabled': True}},
                      'settings': {}})
        session.launch(creative=False)
        prepare(session, checks)
        scene(session)
        session.capture('world-no-screen')
        inventory(session, 'survival-inventory-blurred')
        session.close()

        document = session.saved()
        checks.check('Inventory Blur was saved on',
                     document['modules'].get('render.inventory-blur', {}).get('enabled') is True,
                     document['modules'].get('render.inventory-blur'))
        document['modules']['render.inventory-blur'] = {'enabled': False}
        session.seed(document)
        session.launch(creative=False)
        session.call('wait-ready', '40')
        scene(session)
        inventory(session, 'survival-inventory-sharp-module-off')
        session.close()

        document = session.saved()
        document['modules']['render.inventory-blur'] = {'enabled': True}
        session.seed(document)
        session.launch(creative=False)
        session.call('wait-ready', '40')
        session.call('command', 'gamemode creative @s')
        session.call('wait', '10')
        checks.check('switched to creative', session.call('state').get('gamemode') == 'creative')
        scene(session)
        inventory(session, 'creative-inventory-blurred')
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?', default=os.environ.get('HORIZON_RESOURCES'))
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run(args.resources, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
