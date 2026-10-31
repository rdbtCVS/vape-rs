#!/usr/bin/env python3
"""Exercise live heading/biome HUD data and compass preferences in native play.

layouts: Armor Status, Potion Status, Coords, FPS and Compass from live
player state at GUI scale 2 and 1, with each one's settings seeded per
launch (detailed/compact armor, background on/off, horizontal/vertical
coordinates, positive/negative effects, a foreground colour), a broken piece
and every module turned off through its bind. Results go to result.json,
captures next to it.
"""
import argparse
import json
from pathlib import Path
from native_session import NativeSession
from smoke_scenario import Checks, inventory


def run(resources):
    with NativeSession(resources, 'hud-details-') as session:
        session.seed({'schema': 1, 'modules': {'hud.coordinates': {'enabled': True},
                      'hud.fps': {'enabled': True}, 'hud.compass': {'enabled': True}},
                      'settings': {'hud.coordinates.display': 'vertical',
                                   'hud.fps.y': 112, 'hud.compass.x': 46, 'hud.compass.y': 180,
                                   'hud.compass.binds': ['KeyC']}})
        session.launch()
        for yaw, label in [(-180, 'north'), (-90, 'east'), (0, 'south'), (90, 'west')]:
            session.call('look', str(yaw), '0')
            session.call('wait', '2')
            assert session.call('state')['player']['yaw'] == yaw
            session.capture(label)
        session.call('key', 'c', 'tap')
        session.call('wait', '2')
        session.capture('compass-disabled')
        assert session.saved()['modules']['hud.compass']['enabled'] is False
        session.call('key', 'rshift', 'tap')
        session.capture('gui')
        # HUD modules are listed by name in the HUD editor's centred frame.
        session.call('click-at', '107', '232')
        session.capture('legit-modules')
        session.call('click-at', '190', '96')  # Compass
        session.capture('compass-enabled-in-gui')
        assert session.saved()['modules']['hud.compass']['enabled'] is True
        session.call('click-at', '247', '116')  # Coords dots expand its settings
        session.capture('coordinate-settings')
        session.call('click-at', '200', '207')  # Display Type, fifth setting
        session.call('click-at', '180', '223')  # horizontal, first option
        session.capture('horizontal-selected')
        assert session.saved()['settings']['hud.coordinates.display'] == 'horizontal'
        session.call('click-at', '26', '287')  # BACK
        session.call('key', 'rshift', 'tap')
        session.capture('horizontal-biome')
        session.close()
        session.launch()
        session.capture('after-restart')
        assert session.saved()['modules']['hud.compass']['enabled'] is True
        assert session.saved()['settings']['hud.coordinates.display'] == 'horizontal'
        session.call('key', 'f1', 'tap')
        session.capture('hidden')
        session.call('key', 'f1', 'tap')
        session.capture('restored')
        (session.root / 'assertions.json').write_text(json.dumps({'passed': True, 'checks': [
            'actual yaw drives compass and coordinate arrows', 'live chunk biome snapshot',
            'compass physical binding', 'compass GUI toggle', 'coordinate layout setting',
            'preferences restored after restart', 'host HUD hide/restore captures']}, indent=2)+'\n')


LAYOUT_BINDS = {'hud.armor': 'KeyJ', 'hud.potions': 'KeyK', 'hud.coordinates': 'KeyN',
                'hud.fps': 'KeyM', 'hud.compass': 'KeyC'}
# Durability left: diamond 363, iron 240, leather 75, gold 91.
ARMOR = [('head', 'diamond_helmet', 300), ('chest', 'iron_chestplate', 0),
         ('legs', 'leather_leggings', 74), ('feet', 'golden_boots', 50)]
# The rings go green, then orange (at most half left), then red (a quarter).
EFFECTS = [('speed', '90', '1'), ('slowness', '12', '0'), ('night_vision', 'infinite', '0'),
           ('weakness', '6', '0')]
LAYOUT_CASES = [
    ('defaults', {}),
    ('compact-plain-vertical-negative', {
        'hud.armor.compact': True, 'hud.armor.background': False, 'hud.potions.background': False,
        'hud.coordinates.background': False, 'hud.fps.background': False,
        'hud.compass.background': False, 'hud.coordinates.display': 'vertical',
        'hud.potions.positive': False, 'hud.armor.color': 0xff4cc9ff, 'hud.potions.color': 0xff4cc9ff,
        'hud.coordinates.color': 0xff4cc9ff, 'hud.fps.color': 0xff4cc9ff, 'hud.compass.color': 0xff4cc9ff}),
]


def layouts(resources):
    with NativeSession(resources, 'hud-layouts-') as session:
        checks = Checks(session.root)
        document = {'schema': 1, 'modules': {m: {'enabled': True} for m in LAYOUT_BINDS},
                    'settings': {**{f'{m}.binds': [k] for m, k in LAYOUT_BINDS.items()},
                                 'hud.coordinates.x': 8, 'hud.coordinates.y': 8,
                                 'hud.fps.x': 8, 'hud.fps.y': 100, 'hud.compass.x': 20, 'hud.compass.y': 56,
                                 'hud.armor.x': 330, 'hud.armor.y': 90,
                                 'hud.potions.x': 8, 'hud.potions.y': 130}}
        # Chat and the tutorial toast hidden, so they stay clear of the panels.
        (session.root / 'game' / 'options.txt').write_text('chatVisibility:2\ntutorialStep:none\n')
        for index, (label, settings) in enumerate(LAYOUT_CASES):
            document['settings'].update(settings)
            session.seed(document)
            session.launch()
            session.call('command', 'gamerule advance_time false')
            session.call('time', '6000')
            session.call('command', 'fill -3 119 -3 3 119 3 minecraft:stone')
            session.call('command', 'tp @s 0.5 120 0.5')
            for slot, item, damage in ARMOR:
                session.call('command', f'item replace entity @s armor.{slot} with minecraft:{item}[damage={damage}]')
            session.call('look', '37', '20')
            session.call('wait', '140')  # the advancement and recipe toasts leave
            for effect, seconds, amplifier in EFFECTS:
                session.call('command', f'effect give @s minecraft:{effect} {seconds} {amplifier} true')
            session.call('wait', '6')
            effects = [e.get('id') for e in session.call('state')['player'].get('effects', [])]
            armor = inventory(session).get('armor')
            checks.check(f'{label}: the four damaged armor pieces and four effects are on the player',
                         len(effects) == 4 and all(item in json.dumps(armor) for _, item, _ in ARMOR),
                         {'effects': effects, 'armor': armor})
            for scale in ('2', '1'):
                session.call('options-set', 'guiScale', scale)
                session.call('wait', '4')
                session.capture(f'{label}-scale{scale}')
            session.call('options-set', 'guiScale', '2')
            session.call('look', '-90', '20')
            session.call('wait', '40')  # the effects count down
            session.capture(f'{label}-east-later')
            session.call('wait', '40')
            session.capture(f'{label}-east-latest')
            if index == 0:
                session.call('command', 'item replace entity @s armor.legs with minecraft:air')
                session.call('wait', '4')
                session.capture('defaults-legs-broken')
                session.call('wait', '100')  # over the 4 s broken display
                session.capture('defaults-legs-gone')
                for module, key in LAYOUT_BINDS.items():
                    session.call('key', key[3:].lower(), 'tap')
                session.call('wait', '4')
                session.capture('all-disabled')
                saved = {m: session.saved()['modules'][m]['enabled'] for m in LAYOUT_BINDS}
                checks.check('every HUD module turned off through its bind', not any(saved.values()), saved)
                for module, key in LAYOUT_BINDS.items():
                    session.call('key', key[3:].lower(), 'tap')
                session.call('wait', '4')
                session.capture('all-re-enabled')
                saved = {m: session.saved()['modules'][m]['enabled'] for m in LAYOUT_BINDS}
                checks.check('every HUD module back on through its bind', all(saved.values()), saved)
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'{label}: no panics in the log', 'panicked' not in log)
            session.close()
            saved = session.saved()
            checks.check(f'{label}: settings persisted',
                         all(saved['settings'].get(k) == v for k, v in settings.items()),
                         {k: saved['settings'].get(k) for k in settings})
            document = saved
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}',
              session.root)
        return ok


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--part', choices=('all', 'details', 'layouts'), default='all')
    args = parser.parse_args()
    if args.part in ('all', 'details'):
        run(args.resources)
    if args.part in ('all', 'layouts'):
        raise SystemExit(0 if layouts(args.resources) else 1)
