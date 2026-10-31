#!/usr/bin/env python3
"""Per-module visibility (`Mod.visible`) through the real click GUI.

A hidden enabled module is switched off when the profile loads, the frame
header's edit button lists hidden modules behind a checkbox strip, a hidden
module refuses its bind with a warning toast, unhiding it removes the saved
key again, and bind-only modules refuse GUI clicks with a row status.

Coordinates are click GUI units (see gui_scenario.py): the navigation rows
are 20 apart from Combat at y=38 and a frame opened alone sits at (122, 6)
with a 6..26 header and 20-unit rows.
"""
import argparse
import json
from pathlib import Path
from native_session import NativeSession

RENDER, INVENTORY = (30, 58), (30, 118)
EDIT = (210, 16)  # the header button left of the collapse target
DONE = (208, 16)
HEADER = (150, 16)
# Edit mode lists every Render module: Fullbright is sixth after AntiDebuff,
# Arrows, Clear Water, ESP and Explosions; the strip is the row's left 20.
FULLBRIGHT_STRIP = (130, 136)
# Inventory without its three hidden modules: ArmorSwitch, AutoArmor,
# AutoTotem, InventoryFill, InventoryManager, Refill, Throwpot.
ARMOR_SWITCH, REFILL = (160, 36), (160, 136)


def run(resources):
    with NativeSession(resources, 'hud-visibility-') as session:
        def key(name):
            session.call('key', name, 'tap')

        def click(point):
            session.call('click-at', str(point[0]), str(point[1]))

        def modules():
            return session.saved()['modules']

        checks = []

        def check(name, ok, detail=None):
            checks.append({'check': name, 'passed': bool(ok), 'detail': detail})
            if not ok:
                raise AssertionError(f'{name}: {detail}')

        session.seed({'schema': 1,
                      'modules': {'combat.sprint': {'enabled': True, 'visible': False}},
                      # The floating search frame would cover the header's
                      # edit button of a frame in the first column.
                      'settings': {'client.search-bar': 'None',
                                   'render.fullbright.binds': ['KeyG'],
                                   'inventory.refill.binds': ['KeyV']}})
        session.launch()
        session.capture('hidden-disabled-on-load')
        check('a hidden enabled module is disabled on profile load',
              modules()['combat.sprint'].get('enabled') is False, modules()['combat.sprint'])

        key('rshift')
        click(RENDER)
        click(HEADER)
        session.capture('render-header-hover')
        click(EDIT)
        session.capture('render-edit-mode')
        click(FULLBRIGHT_STRIP)
        session.capture('fullbright-hidden-in-edit-mode')
        check('the checkbox stores visible=false',
              modules().get('render.fullbright', {}).get('visible') is False,
              modules().get('render.fullbright'))
        click(DONE)
        session.capture('render-frame-without-fullbright')

        key('rshift')
        key('g')
        session.call('wait', '10')
        session.capture('hidden-bind-warning')
        check('the bind of a hidden module is refused',
              not modules().get('render.fullbright', {}).get('enabled', False),
              modules().get('render.fullbright'))

        key('rshift')
        click(EDIT)
        click(FULLBRIGHT_STRIP)
        session.capture('fullbright-unhidden-in-edit-mode')
        click(DONE)
        check('unhiding removes the default visibility key',
              'visible' not in modules().get('render.fullbright', {}),
              modules().get('render.fullbright'))
        key('rshift')
        key('g')
        session.call('wait', '10')
        session.capture('unhidden-bind-toggles')
        check('the bind toggles again once visible',
              modules()['render.fullbright'].get('enabled') is True, modules()['render.fullbright'])

        key('rshift')
        click(RENDER)  # close Render so Inventory takes the first column
        click(INVENTORY)
        click(HEADER)
        session.capture('inventory-header-hidden-count')
        click(ARMOR_SWITCH)
        session.capture('armor-switch-must-be-bound')
        click(REFILL)
        session.capture('refill-use-via-bind')
        saved = modules()
        check('bind-only modules refuse GUI clicks',
              not saved.get('inventory.armor-switch', {}).get('enabled', False)
              and not saved.get('inventory.refill', {}).get('enabled', False),
              {k: saved.get(k) for k in ('inventory.armor-switch', 'inventory.refill')})
        key('rshift')
        session.close()
        session.launch()
        session.call('wait', '20')
        saved = modules()
        check('visibility and states survive a restart',
              saved['render.fullbright'].get('enabled') is True
              and 'visible' not in saved['render.fullbright']
              and saved['combat.sprint'].get('visible') is False,
              {k: saved.get(k) for k in ('render.fullbright', 'combat.sprint')})
        session.close()
        (session.root / 'assertions.json').write_text(
            json.dumps({'passed': True, 'checks': checks}, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    run(args.resources)
