#!/usr/bin/env python3
"""Render modules on the real client, each captured on and off.

Two parts, each with fresh isolated homes:

* Single player (creative): StorageESP (chest + ender chest), Search
  (diamond ore), SpawnerFinder (zombie spawner), Explosions (lit TNT),
  Trajectories (drawn bow, loaded crossbow) and NameTags on an equipped mob.
* LAN: a host player in creative, holding a sword and wearing a helmet, seen
  by a guest running ESP 2D (health bar, name), Tracers, NameTags (players,
  equipment), MurdererFinder, Health, Arrows and PropHunt (a block update and
  a falling block from the host).

Each module is toggled by a bind; the check compares the on and off frames
(changed pixels, and pixels near the module's colour when it has one).
Screenshots stay in the session directory; result.json lists the checks.
"""
import argparse
from pathlib import Path

from PIL import Image, ImageChops

from native_session import NativeSession
from smoke_scenario import Checks

KEYS = iter('jknmbghyuiopzv')


def changed(a, b, threshold=48):
    diff = ImageChops.difference(Image.open(a).convert('RGB'), Image.open(b).convert('RGB'))
    return sum(1 for p in diff.getdata() if sum(p) > threshold)


def near(path, rgb, tolerance=40):
    image = Image.open(path).convert('RGB')
    return sum(1 for p in image.getdata() if all(abs(p[i] - rgb[i]) <= tolerance for i in range(3)))


class Toggler:
    def __init__(self, session, checks, modules):
        self.session, self.checks = session, checks
        self.binds = {m: next(KEYS) for m in modules}

    def settings(self):
        return {f'{m}.binds': ['Key' + k.upper()] for m, k in self.binds.items()}

    def tap(self, module):
        self.session.call('key', self.binds[module], 'tap')
        self.session.call('wait', '3')

    def compare(self, module, label, minimum=200, rgb=None, wait=6):
        """Capture with the module on, then off; check the frames differ."""
        self.tap(module)
        self.session.call('wait', str(wait))
        on = self.session.root / f'{label}-on.png'
        self.session.capture(f'{label}-on')
        state = self.session.saved()['modules'].get(module, {}).get('enabled', False)
        self.tap(module)
        self.session.call('wait', str(wait))
        off = self.session.root / f'{label}-off.png'
        self.session.capture(f'{label}-off')
        pixels = changed(on, off)
        detail = {'changed': pixels, 'enabled-when-captured': state}
        ok = state and pixels >= minimum
        if rgb is not None:
            detail['colour-on'], detail['colour-off'] = near(on, rgb), near(off, rgb)
            ok = ok and detail['colour-on'] > detail['colour-off'] + 50
        self.checks.check(f'{label}: drawn when on, gone when off', ok, detail)


def single_player(resources, allow_stale, results):
    modules = ['render.storage-esp', 'render.search', 'render.spawner-finder', 'render.explosions',
               'render.trajectories', 'render.name-tags']
    with NativeSession(resources, 'render-sp-', allow_stale=allow_stale) as s:
        checks = Checks(s.root)
        t = Toggler(s, checks, modules)
        s.seed({'schema': 1, 'modules': {}, 'settings': {
            **t.settings(),
            'render.search.blocks': ['minecraft:diamond_ore=FFFF00FF'],
            'render.name-tags.mobs': True,
            'render.name-tags.mobs.equipment': True,
        }})
        s.launch()
        s.call('time', '6000')
        s.call('command', 'gamerule advance_time false')
        s.call('command', 'gamerule advance_weather false')
        s.call('command', 'weather clear')
        s.call('tp', '0.5', '100', '0.5')
        s.call('wait-ready', '40')
        s.call('fill', '-8', '99', '-4', '8', '99', '16', 'minecraft:stone')
        s.call('fill', '-8', '100', '-4', '8', '108', '16', 'minecraft:air')
        s.call('tp', '0.5', '100', '0.5')
        s.call('look', '0', '15')
        s.call('wait', '10')

        s.call('fill', '1', '100', '4', '1', '100', '4', 'minecraft:chest')
        s.call('fill', '-1', '100', '4', '-1', '100', '4', 'minecraft:ender_chest')
        s.call('wait', '6')
        t.compare('render.storage-esp', 'storage-esp')

        # Behind a stone wall, so only Search shows it.
        s.call('fill', '-4', '100', '6', '-2', '102', '8', 'minecraft:stone')
        s.call('fill', '-3', '101', '7', '-3', '101', '7', 'minecraft:diamond_ore')
        s.call('wait', '6')
        t.compare('render.search', 'search', rgb=(255, 0, 255), wait=20)

        s.call('command', 'setblock 3 100 6 minecraft:spawner{SpawnData:{entity:{id:"minecraft:zombie"}}}')
        s.call('wait', '10')
        t.compare('render.spawner-finder', 'spawner-finder')

        s.call('command', 'summon tnt 0.5 100 9 {fuse:6000,NoGravity:1b}')
        s.call('wait', '6')
        t.compare('render.explosions', 'explosions')
        s.call('command', 'kill @e[type=tnt]')

        s.call('command', 'summon zombie 0.5 100 3 {NoAI:1b,PersistenceRequired:1b,'
               'equipment:{head:{id:"minecraft:diamond_helmet",count:1},'
               'mainhand:{id:"minecraft:iron_sword",count:1}}}')
        s.call('wait', '10')
        t.compare('render.name-tags', 'name-tags-mob')
        s.call('command', 'kill @e[type=zombie]')

        s.call('look', '0', '-10')
        s.call('give', 'minecraft:bow', '1')
        s.call('give', 'minecraft:arrow', '64')
        s.call('hotbar', '1')
        s.call('mouse', 'right', 'down')
        s.call('wait', '25')
        t.compare('render.trajectories', 'trajectories-bow', wait=4)
        s.call('mouse', 'right', 'up')
        s.call('wait', '10')

        s.call('command', 'give @p minecraft:crossbow[charged_projectiles=[{id:"minecraft:arrow",count:1}]]')
        s.call('wait', '4')
        inv = s.call('inventory')
        s.call('hotbar', '3')
        s.call('wait', '4')
        checks.check('a charged crossbow is held', 'crossbow' in str(s.call('state').get('selected')),
                     s.call('state').get('selected'))
        t.compare('render.trajectories', 'trajectories-crossbow')
        del inv
        results.extend(checks.results)
        return s.root


def lan(resources, allow_stale, results):
    modules = ['render.esp', 'render.tracers', 'render.name-tags', 'render.murder-finder',
               'render.health', 'render.arrows', 'render.prop-hunt']
    with NativeSession(resources, 'render-host-', allow_stale=allow_stale) as host, \
            NativeSession(resources, 'render-guest-', allow_stale=allow_stale) as guest:
        checks = Checks(guest.root)
        t = Toggler(guest, checks, modules)
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=True, world='RenderHost')
        host.call('time', '6000')
        host.call('command', 'gamerule advance_time false')
        host.call('command', 'weather clear')
        host.call('tp', '0.5', '100', '0.5')
        host.call('wait-ready', '40')
        host.call('fill', '-12', '99', '-12', '12', '99', '20', 'minecraft:stone')
        host.call('fill', '-12', '100', '-12', '12', '108', '20', 'minecraft:air')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        guest.seed({'schema': 1, 'modules': {}, 'settings': {
            **t.settings(),
            'render.esp.mode': '2D',
            'render.esp.health-bar': True,
            'render.esp.name': True,
            'render.esp.show-background': True,
            'render.name-tags.players.equipment': True,
        }})
        guest.launch(creative=False, world=None, extra=('--server', f'localhost:{port}', '--name', 'VapeGuest'))
        guest.call('wait-ready', '60')
        host.call('command', 'tp VapeGuest 0.5 100 0.5 0 0')
        host.call('command', 'gamemode creative VapeGuest')
        host.call('give', 'minecraft:iron_sword', '1')
        host.call('give', 'minecraft:diamond_helmet', '1')
        host.call('hotbar', '1')
        host.call('tp', '0.5', '100', '6.5')
        host.call('command', 'item replace entity @s armor.head with minecraft:diamond_helmet')
        host.call('command', 'item replace entity @a[name=!VapeGuest] armor.head with minecraft:diamond_helmet')
        guest.call('look', '0', '10')
        guest.call('wait', '20')

        t.compare('render.esp', 'esp-2d')
        t.compare('render.tracers', 'tracers')
        t.compare('render.name-tags', 'name-tags-player')
        t.compare('render.murder-finder', 'murder-finder', minimum=50)
        t.compare('render.health', 'health', minimum=50)

        # Arrows: the host behind the guest.
        host.call('tp', '0.5', '100', '-8.5')
        guest.call('wait', '10')
        t.compare('render.arrows', 'arrows', minimum=50)

        # PropHunt: a block change (red box for 10 s) and a falling block.
        host.call('tp', '8.5', '100', '8.5')
        t.tap('render.prop-hunt')
        host.call('command', 'setblock 2 100 5 minecraft:oak_planks')
        host.call('command', 'summon falling_block -2.5 101 5.5 {BlockState:{Name:"minecraft:sand"},NoGravity:1b,Time:1}')
        guest.call('wait', '6')
        guest.capture('prop-hunt-on')
        on_state = guest.saved()['modules'].get('render.prop-hunt', {}).get('enabled', False)
        t.tap('render.prop-hunt')
        guest.call('wait', '4')
        guest.capture('prop-hunt-off')
        pixels = changed(guest.root / 'prop-hunt-on.png', guest.root / 'prop-hunt-off.png')
        checks.check('prop-hunt: block update and falling block marked when on', on_state and pixels > 200,
                     {'changed': pixels})
        results.extend(checks.results)
        return guest.root


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--allow-stale', action='store_true')
    parser.add_argument('--part', choices=['sp', 'lan', 'all'], default='all')
    args = parser.parse_args()
    results = []
    if args.part in ('sp', 'all'):
        print('single player:', single_player(args.resources, args.allow_stale, results))
    if args.part in ('lan', 'all'):
        print('lan:', lan(args.resources, args.allow_stale, results))
    failed = [r['check'] for r in results if not r['ok']]
    print(f'{len(results) - len(failed)}/{len(results)} checks passed', failed)
