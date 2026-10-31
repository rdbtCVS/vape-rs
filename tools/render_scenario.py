#!/usr/bin/env python3
"""Render modules on the real client, each captured on and off.

Two parts, each with fresh isolated homes:

* Single player (creative): StorageESP (chest + ender chest), Search
  (diamond ore with a tracer, emerald ore without), SpawnerFinder (zombie
  spawner), Explosions (lit TNT, two pulse phases), NameTags on an equipped
  mob, ItemESP groups (three items together, one apart), Block Overlay,
  BedPlates (a bed in a wool and end stone shell), Projectiles (an arrow and
  a pearl in flight), Indicators (an arrow from behind; Always and Hit Only
  after relaunches with the mode) and Trajectories (a drawn bow at a block
  and at a mob, a loaded crossbow).
* LAN: a host player in creative, holding a sword and wearing a helmet, seen
  by a guest running ESP 2D (health bar, name), Tracers, NameTags (players,
  equipment), MurdererFinder, Health, Arrows, PropHunt (a block update and a
  falling block from the host) and StorageESP on a chest the host holds open;
  then ESP Outline with Tracers' Enemy Only and the host as a friend, and
  ESP 3D, each after relaunching the guest with the setting.

Each module is toggled by a bind and captured on and off in four views: GUI
scale 2 and 1, near and far (the player, or the subject, moved). The check
compares each view's on and off frames (changed pixels, and pixels near the
module's colour when it has one). Screenshots stay in the session directory;
result.json lists the checks.
"""
import argparse
from pathlib import Path

from PIL import Image, ImageChops

from native_session import NativeSession
from smoke_scenario import Checks

KEYS = 'jknmbghyuiozv'


def pixels(image):
    """An image's pixels (`getdata` is deprecated from Pillow 12)."""
    return getattr(image, 'get_flattened_data', image.getdata)()


def changed(a, b, threshold=48, box=None):
    """Pixels that differ between two captures (inside `box`, if given)."""
    diff = ImageChops.difference(Image.open(a).convert('RGB'), Image.open(b).convert('RGB'))
    if box is not None:
        diff = diff.crop(box)
    return sum(1 for p in pixels(diff) if sum(p) > threshold)


def red(path, box):
    """Strongly red pixels inside `box`."""
    image = Image.open(path).convert('RGB').crop(box)
    return sum(1 for p in pixels(image) if p[0] > 200 and p[1] < 70 and p[2] < 70)


def near(path, rgb, tolerance=40):
    image = Image.open(path).convert('RGB')
    return sum(1 for p in pixels(image) if all(abs(p[i] - rgb[i]) <= tolerance for i in range(3)))


def crop(path, box, out, zoom=4):
    """A zoomed crop of a capture, for reading small text."""
    image = Image.open(path).convert('RGB').crop(box)
    image.resize((image.width * zoom, image.height * zoom), Image.NEAREST).save(out)


def enabled(session, module):
    return session.saved()['modules'].get(module, {}).get('enabled', False)


class Toggler:
    def __init__(self, session, checks, modules):
        self.session, self.checks = session, checks
        self.binds = dict(zip(modules, KEYS, strict=False))
        assert len(self.binds) == len(modules), 'more modules than keys'

    def settings(self):
        return {f'{m}.binds': ['Key' + k.upper()] for m, k in self.binds.items()}

    def tap(self, module):
        self.session.call('key', self.binds[module], 'tap')
        self.session.call('wait', '3')

    def compare(self, module, label, views, minimum=200, rgb=None, wait=6):
        """In each view, capture with the module on, then off; check every
        view's frames differ (by a quarter as much in the far views)."""
        root, failed, details = self.session.root, [], {}
        for name, setup in views:
            frames, states = {}, {}
            for phase in ('on', 'off'):
                setup()
                self.tap(module)
                self.session.call('wait', str(wait))
                frames[phase] = root / f'{label}-{name}-{phase}.png'
                self.session.capture(frames[phase].stem)
                states[phase] = enabled(self.session, module)
            far = name.startswith('far')
            pixels = changed(frames['on'], frames['off'])
            detail = {'changed': pixels}
            ok = states['on'] and not states['off'] and pixels >= (max(20, minimum // 4) if far else minimum)
            if rgb is not None:
                detail['colour-on'], detail['colour-off'] = near(frames['on'], rgb), near(frames['off'], rgb)
                ok = ok and detail['colour-on'] > detail['colour-off'] + (10 if far else 50)
            details[name] = detail
            if not ok:
                failed.append(name)
        self.checks.check(f'{label}: drawn when on, gone when off, in every view', not failed,
                          {'failed': failed, **details})
        return details


def views(session, target, near_at, far_at, setup=None):
    """GUI scale 2 and 1 with the player at `near_at` and `far_at`, facing
    `target`; `setup` runs first in each (fresh projectiles)."""
    out = []
    for distance, at in (('near', near_at), ('far', far_at)):
        for scale in (2, 1):
            def run(at=at, scale=scale, distance=distance):
                session.call('options-set', 'guiScale', str(scale))
                session.call('tp', *map(str, at))
                session.call('look-at', *map(str, target))
                if setup is not None:
                    setup(distance)
                session.call('wait', '2')
            out.append((f'{distance}-gui{scale}', run))
    return out


def subject_views(viewer, mover, target, near_at, far_at):
    """LAN views: GUI scale 2 and 1 with the subject (`mover`) at `near_at`
    and `far_at`; the viewer faces `target(at)`."""
    out = []
    for distance, at in (('near', near_at), ('far', far_at)):
        for scale in (2, 1):
            def run(at=at, scale=scale):
                viewer.call('options-set', 'guiScale', str(scale))
                mover.call('tp', *map(str, at))
                viewer.call('look-at', *map(str, target(at)))
                viewer.call('wait', '4')
            out.append((f'{distance}-gui{scale}', run))
    return out


SP_MODULES = ['render.storage-esp', 'render.search', 'render.spawner-finder', 'render.explosions',
              'render.trajectories', 'render.name-tags', 'render.item-esp', 'hud.block-overlay',
              'render.bed-plates', 'render.projectiles', 'render.indicators']
NEAR, FAR = (0.5, 100, 0.5), (0.5, 100, -10.5)


def single_player(resources, allow_stale, results):
    with NativeSession(resources, 'render-sp-', allow_stale=allow_stale) as s:
        checks = Checks(s.root)
        t = Toggler(s, checks, SP_MODULES)
        settings = {
            **t.settings(),
            'render.storage-esp.enderchests': True,
            'render.search.blocks': ['minecraft:diamond_ore=FFFF00FF+tracer', 'minecraft:emerald_ore=FF00FF00'],
            'render.search.use-tracers': True,
            'render.name-tags.mobs': True,
            'render.name-tags.mobs.equipment': True,
            'render.item-esp.group-items': True,
            'render.indicators.show-distance': True,
        }
        s.seed({'schema': 1, 'modules': {}, 'settings': settings})
        s.launch()
        s.call('time', '6000')
        s.call('command', 'gamerule advance_time false')
        s.call('command', 'gamerule advance_weather false')
        s.call('command', 'weather clear')
        s.call('tp', *map(str, NEAR))
        s.call('wait-ready', '40')
        s.call('fill', '-12', '99', '-16', '12', '99', '20', 'minecraft:stone')
        s.call('fill', '-12', '100', '-16', '12', '110', '20', 'minecraft:air')
        s.call('tp', *map(str, NEAR))
        s.call('look', '0', '15')
        s.call('wait', '10')

        def clear(*box):
            s.call('fill', *map(str, box), 'minecraft:air')

        s.call('fill', '1', '100', '4', '1', '100', '4', 'minecraft:chest')
        s.call('fill', '-1', '100', '4', '-1', '100', '4', 'minecraft:ender_chest')
        s.call('wait', '6')
        t.compare('render.storage-esp', 'storage-esp', views(s, (0.5, 100.5, 4.5), NEAR, FAR))
        clear(-1, 100, 4, 1, 100, 4)

        # Behind a stone wall, so only Search shows them; the diamond ore
        # has a tracer, the emerald ore not.
        s.call('fill', '-4', '100', '6', '-2', '102', '8', 'minecraft:stone')
        s.call('fill', '-3', '101', '7', '-3', '101', '7', 'minecraft:diamond_ore')
        s.call('fill', '-3', '100', '7', '-3', '100', '7', 'minecraft:emerald_ore')
        s.call('wait', '6')
        t.compare('render.search', 'search', views(s, (-2.5, 101, 7.5), NEAR, FAR), rgb=(255, 0, 255), wait=20)
        clear(-4, 100, 6, -2, 102, 8)

        s.call('command', 'setblock 3 100 6 minecraft:spawner{SpawnData:{entity:{id:"minecraft:zombie"}}}')
        s.call('wait', '10')
        t.compare('render.spawner-finder', 'spawner-finder', views(s, (3.5, 100.5, 6.5), NEAR, FAR))
        clear(3, 100, 6, 3, 100, 6)

        s.call('command', 'summon zombie 0.5 100 4 {NoAI:1b,PersistenceRequired:1b,'
               'equipment:{head:{id:"minecraft:diamond_helmet",count:1},'
               'mainhand:{id:"minecraft:iron_sword",count:1}}}')
        s.call('wait', '10')
        t.compare('render.name-tags', 'name-tags-mob', views(s, (0.5, 101.8, 4), NEAR, FAR))
        s.call('command', 'kill @e[type=zombie]')

        # ItemESP with Group Items: three items together are one tag of three
        # lines; a stick apart has its own.
        for item, x, z in (('diamond', 2.3, 5.3), ('emerald', 2.7, 5.6), ('gold_ingot', 2.5, 5.9), ('stick', -3.5, 6.5)):
            s.call('command', f'summon item {x} 100 {z} {{Item:{{id:"minecraft:{item}",count:3}},PickupDelay:32767}}')
        s.call('wait', '20')
        t.compare('render.item-esp', 'item-esp-groups', views(s, (0, 100.5, 6), NEAR, FAR))
        s.call('command', 'kill @e[type=item]')

        # Block Overlay on a stair (its whole shape's box), within reach.
        s.call('command', 'setblock 0 100 4 minecraft:oak_stairs[facing=south]')
        s.call('wait', '4')

        def aim(_distance):
            s.call('look-at', 'block', '0', '100', '4')

        t.compare('hud.block-overlay', 'block-overlay',
                  views(s, (0.5, 100.5, 4.5), (0.5, 100, 2), (0.5, 100, 0), setup=aim), rgb=(255, 0, 0), minimum=100)
        clear(0, 100, 4, 0, 100, 4)

        # A bed in a wool shell inside an end stone one.
        s.call('fill', '2', '100', '8', '6', '102', '13', 'minecraft:end_stone')
        s.call('fill', '3', '100', '9', '5', '101', '12', 'minecraft:white_wool')
        s.call('command', 'setblock 4 100 10 minecraft:red_bed[facing=north,part=head]')
        s.call('command', 'setblock 4 100 11 minecraft:red_bed[facing=north,part=foot]')
        s.call('wait', '20')
        t.compare('render.bed-plates', 'bed-plates', views(s, (4.5, 101.5, 10.5), NEAR, FAR), minimum=100)
        clear(2, 100, 8, 6, 102, 13)

        # Projectiles: an arrow and a pearl in a high arc, fresh in each frame.
        def throw(_distance):
            s.call('command', 'kill @e[type=arrow]')
            s.call('command', 'kill @e[type=ender_pearl]')
            s.call('command', 'summon arrow -6 101 10 {Motion:[0.35d,1.0d,0.0d],pickup:0b}')
            s.call('command', 'summon ender_pearl 6 101 12 {Motion:[-0.35d,1.0d,0.0d]}')

        t.compare('render.projectiles', 'projectiles', views(s, (0.5, 104, 10), NEAR, FAR, setup=throw),
                  rgb=(255, 0, 0), wait=2)
        s.call('command', 'kill @e[type=arrow]')
        s.call('command', 'kill @e[type=ender_pearl]')

        # Indicators (Threat): an arrow from behind that hits the player.
        def from_behind(distance):
            s.call('command', 'kill @e[type=arrow]')
            s.call('look', '0', '0')
            z = -12 if distance == 'near' else -20
            s.call('command', f'summon arrow 0.5 101.2 {z} {{Motion:[0.0d,0.3d,1.5d],pickup:0b}}')

        t.compare('render.indicators', 'indicators-threat', views(s, (0.5, 101.6, 10), NEAR, NEAR, setup=from_behind),
                  minimum=60, wait=1)
        s.call('command', 'kill @e[type=arrow]')

        # Trajectories: a drawn bow aiming at the floor (aiming white, its
        # cross on the block), then at a mob (trajectory red, target blue).
        s.call('give', 'minecraft:bow', '1')
        s.call('give', 'minecraft:arrow', '64')
        s.call('select', '1')
        s.call('tp', *map(str, NEAR))
        s.call('mouse', 'right', 'down')
        s.call('wait', '25')
        # The command feedback in chat covers the lower screen, where the
        # near bow arc runs; hide chat for the bow captures.
        s.call('options-set', 'chatVisibility', '2')

        def floor(distance):
            s.call('look-at', '0.5', '100', '5.5' if distance == 'near' else '14.5')

        t.compare('render.trajectories', 'trajectories-bow-block',
                  views(s, (0.5, 99, 5), NEAR, NEAR, setup=floor), rgb=(255, 255, 255), wait=4)
        # A carved pumpkin (never damaged) keeps the zombies from burning in
        # the daylight while they are captured.
        s.call('command', 'summon zombie 0.5 100 9 {NoAI:1b,PersistenceRequired:1b,'
               'equipment:{head:{id:"minecraft:carved_pumpkin",count:1}}}')
        s.call('wait', '6')
        zombie = next((e for e in s.call('entities', '16', 'zombie').get('entities', []) if 'id' in e), None)

        def at_zombie(_distance):
            s.call('look-at', '0.5', '101.2', '9')

        details = t.compare('render.trajectories', 'trajectories-bow-mob',
                            views(s, (0.5, 101.2, 9), NEAR, NEAR, setup=at_zombie), wait=4)
        blue = {name: (near(s.root / f'trajectories-bow-mob-{name}-on.png', (0, 0, 255)),
                       near(s.root / f'trajectories-bow-mob-{name}-off.png', (0, 0, 255)))
                for name in details}
        checks.check('trajectories: the cross is target blue on a mob',
                     all(on > off + 5 for on, off in blue.values()), {'zombie': zombie, 'blue': blue})
        s.call('mouse', 'right', 'up')
        s.call('options-set', 'chatVisibility', '0')
        s.call('command', 'kill @e[type=zombie]')
        s.call('command', 'kill @e[type=arrow]')
        s.call('command', 'kill @e[type=item]')
        # The trajectory colour: facing a near mob the path runs along the
        # view axis under the cross, so a snowball thrown 10 degrees up at a
        # zombie 27 blocks off (inside the 2-chunk render distance's entity
        # tracking, about 32 blocks) instead: its arc drops about 11 degrees
        # onto it, below the cross. Any pitch from -7 to -13 hits the zombie's
        # box, where a bow's steep lob had to land within a tenth of a degree.
        s.call('command', 'summon zombie 0.5 100 12.5 {NoAI:1b,PersistenceRequired:1b,'
               'equipment:{head:{id:"minecraft:carved_pumpkin",count:1}}}')
        s.call('give', 'minecraft:snowball', '16')
        s.call('wait', '30')
        s.call('command', 'kill @e[type=arrow]')
        s.call('select', 'snowball')
        s.call('wait', '4')
        held = s.call('state').get('held')
        checks.check('a snowball is held', 'snowball' in str(held), held)
        back = (0.5, 100, -14.5)
        t.compare('render.trajectories', 'trajectories-snowball-far-mob',
                  views(s, (0.5, 101, 12.5), back, back, setup=lambda _d: s.call('look', '0', '-10')),
                  rgb=(255, 0, 0), wait=4)
        s.call('command', 'kill @e[type=zombie]')
        s.call('command', 'clear @p minecraft:snowball')
        s.call('wait', '10')

        s.call('command', 'give @p minecraft:crossbow[charged_projectiles=[{id:"minecraft:arrow",count:1}]]')
        s.call('wait', '4')
        s.call('select', 'crossbow')
        s.call('wait', '4')
        held = s.call('state').get('held')
        checks.check('a charged crossbow is held', 'crossbow' in str(held), held)
        t.compare('render.trajectories', 'trajectories-crossbow', views(s, (0.5, 99, 14), NEAR, NEAR, setup=floor))

        # Indicators' other modes, each after a relaunch with it: Always
        # shows an arrow passing 8 blocks aside; Hit Only hides one passing 3
        # aside and shows one that hits.
        def relaunch(mode):
            s.close()
            doc = s.saved()
            doc['settings']['render.indicators.alert-type'] = mode
            s.seed(doc)
            s.launch()
            s.call('wait-ready', '40')
            s.call('options-set', 'guiScale', '2')
            s.call('tp', *map(str, NEAR))
            s.call('look', '0', '0')
            s.call('command', 'kill @e[type=item]')
            s.call('wait', '10')

        def passing(aside):
            def run(_distance):
                s.call('command', 'kill @e[type=arrow]')
                s.call('look', '0', '0')
                s.call('command', f'summon arrow {0.5 + aside} 101.2 -12 {{Motion:[0.0d,0.3d,1.5d],pickup:0b}}')
            return run

        relaunch('Always')
        t.compare('render.indicators', 'indicators-always-aside',
                  views(s, (0.5, 101.6, 10), NEAR, NEAR, setup=passing(8)), minimum=60, wait=1)
        relaunch('Hit Only')
        details = t.compare('render.indicators', 'indicators-hit-only-hit',
                            views(s, (0.5, 101.6, 10), NEAR, NEAR, setup=passing(0)), minimum=60, wait=1)
        for phase in ('on', 'off'):
            passing(3)('near')
            t.tap('render.indicators')
            s.call('wait', '1')
            s.capture(f'indicators-hit-only-miss-{phase}')
        # The indicator's red arrow, around the crosshair only: clouds drift
        # and dropped items bob between the two frames.
        ring = (200, 150, 600, 420)
        miss = {phase: red(s.root / f'indicators-hit-only-miss-{phase}.png', ring) for phase in ('on', 'off')}
        hit = red(s.root / 'indicators-hit-only-hit-near-gui2-on.png', ring)
        checks.check('indicators (Hit Only): an arrow passing 3 aside is not shown',
                     miss['on'] <= miss['off'] + 5 and hit > miss['on'] + 50, {'red': miss, 'red-on-hit': hit})
        s.call('command', 'kill @e[type=arrow]')

        # Last: the pinned host's selectors cannot target primed TNT, so the
        # fuse-6000 TNT stays (flashing) for the rest of the world's life.
        # The sphere is depth-tested: the floor hides its lower half.
        s.call('command', 'summon tnt 0.5 100 9 {fuse:6000s,NoGravity:1b}')
        s.call('wait', '6')
        t.compare('render.explosions', 'explosions', views(s, (0.5, 100.5, 9), NEAR, (0.5, 100, -8.5)))
        # The pulse: frames differ more with the module on than off. It is
        # solid for five ticks and fades in over five, so two frames half a
        # period apart can both be nearly solid; four frames three ticks apart
        # always include a solid one and one under 0.36 alpha.
        s.call('options-set', 'guiScale', '2')
        s.call('tp', *map(str, NEAR))
        s.call('look-at', '0.5', '100.5', '9')
        pulse = {}
        steps = ('a', 'b', 'c', 'd')
        for phase in ('on', 'off'):
            t.tap('render.explosions')
            for step in steps:
                s.capture(f'explosions-pulse-{phase}-{step}')
                s.call('wait', '3')
            frames = [s.root / f'explosions-pulse-{phase}-{step}.png' for step in steps]
            pulse[phase] = max(changed(a, b) for i, a in enumerate(frames) for b in frames[i + 1:])
        checks.check('explosions: the sphere pulses between frames', pulse['on'] > pulse['off'] + 500, pulse)
        results.extend(checks.results)
        return s.root


LAN_MODULES = ['render.esp', 'render.tracers', 'render.name-tags', 'render.murder-finder',
               'render.health', 'render.arrows', 'render.prop-hunt', 'render.storage-esp']


def lan(resources, allow_stale, results):
    with NativeSession(resources, 'render-host-', allow_stale=allow_stale) as host, \
            NativeSession(resources, 'render-guest-', allow_stale=allow_stale) as guest:
        checks = Checks(guest.root)
        t = Toggler(guest, checks, LAN_MODULES)
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=True, world='RenderHost', extra=('--name', 'VapeHost'))
        host.call('time', '6000')
        host.call('command', 'gamerule advance_time false')
        host.call('command', 'weather clear')
        host.call('tp', '0.5', '100', '0.5')
        host.call('wait-ready', '40')
        host.call('fill', '-12', '99', '-20', '12', '99', '20', 'minecraft:stone')
        host.call('fill', '-12', '100', '-20', '12', '108', '20', 'minecraft:air')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        settings = {
            **t.settings(),
            'render.esp.mode': '2D',
            'render.esp.health-bar': True,
            'render.esp.name': True,
            'render.esp.show-background': True,
            'render.name-tags.players.equipment': True,
        }

        def join(doc):
            guest.seed(doc)
            guest.launch(creative=False, world=None, extra=('--server', f'localhost:{port}', '--name', 'VapeGuest'))
            guest.call('wait-ready', '60')
            host.call('command', 'tp VapeGuest 0.5 100 0.5 0 0')
            host.call('command', 'gamemode creative VapeGuest')
            guest.call('wait', '20')

        join({'schema': 1, 'modules': {}, 'settings': settings})
        host.call('give', 'minecraft:iron_sword', '1')
        host.call('select', '1')
        # Enchanted, so the equipment row shows the helmet's glint.
        host.call('command', 'item replace entity @s armor.head with minecraft:diamond_helmet[minecraft:enchantments={protection:1}]')
        host.call('tp', '0.5', '100', '6.5')
        guest.call('wait', '20')

        def body(at):
            return (at[0], at[1] + 1, at[2])

        ahead = subject_views(guest, host, body, (0.5, 100, 6.5), (0.5, 100, 16.5))
        t.compare('render.esp', 'esp-2d', ahead)
        # Tracers end at the eyes: facing the subject the line is a few
        # pixels, so the guest looks 4 blocks to its side.
        aside = subject_views(guest, host, lambda at: (at[0] - 4, at[1] + 1, at[2]),
                              (0.5, 100, 6.5), (0.5, 100, 16.5))
        t.compare('render.tracers', 'tracers', aside, rgb=(0, 150, 255))
        t.compare('render.name-tags', 'name-tags-player', ahead)
        t.compare('render.murder-finder', 'murder-finder', ahead, minimum=50)
        t.compare('render.health', 'health', ahead, minimum=50)
        for scale in (2, 1):
            # The stack's label, 10 below the centre (GUI units).
            u = scale
            crop(guest.root / f'health-near-gui{scale}-on.png',
                 (400 - 40 * u, 300 + 6 * u, 400 + 40 * u, 300 + 22 * u),
                 guest.root / f'health-heart-gui{scale}.png', zoom=8 // u)

        # Arrows: the host behind the guest, which keeps facing south.
        behind = subject_views(guest, host, lambda at: (0.5, 101.6, 10), (0.5, 100, -5.5), (0.5, 100, -15.5))
        t.compare('render.arrows', 'arrows', behind, minimum=50)

        # PropHunt: a block change (red box for 10 s) and a falling block.
        def props(_distance):
            host.call('command', 'setblock 2 100 5 minecraft:air')
            host.call('command', 'setblock 2 100 5 minecraft:oak_planks')
            host.call('command', 'kill @e[type=falling_block]')
            host.call('command', 'summon falling_block -2.5 101 5.5 {BlockState:{Name:"minecraft:sand"},NoGravity:1b,Time:1}')

        host.call('tp', '8.5', '100', '-8.5')
        prop_views = []
        for distance, at in (('near', (0.5, 100, 0.5)), ('far', (0.5, 100, -8.5))):
            for scale in (2, 1):
                def run(at=at, scale=scale, distance=distance):
                    guest.call('options-set', 'guiScale', str(scale))
                    host.call('command', f'tp VapeGuest {at[0]} {at[1]} {at[2]}')
                    guest.call('wait', '4')
                    guest.call('look-at', '0', '101', '5.5')
                    props(distance)
                    guest.call('wait', '2')
                prop_views.append((f'{distance}-gui{scale}', run))
        t.compare('render.prop-hunt', 'prop-hunt', prop_views)
        host.call('command', 'setblock 2 100 5 minecraft:air')
        host.call('command', 'kill @e[type=falling_block]')
        # The sand the falling blocks dropped bobs between later frames.
        host.call('command', 'kill @e[type=item]')
        host.call('command', 'tp VapeGuest 0.5 100 0.5 0 0')

        # StorageESP follows the lid of a chest the host holds open.
        host.call('command', 'setblock 3 100 4 minecraft:chest[facing=north]')
        host.call('tp', '3.5', '100', '2.5')
        host.call('wait', '4')
        screen = host.call('interact', '3', '100', '4')
        guest.call('wait', '20')
        checks.check('host opened the chest', 'chest' in str(screen).lower() or 'container' in str(screen).lower(), screen)
        storage_views = []
        for distance, at in (('near', (0.5, 100, 0.5)), ('far', (0.5, 100, -8.5))):
            for scale in (2, 1):
                def run(at=at, scale=scale):
                    guest.call('options-set', 'guiScale', str(scale))
                    host.call('command', f'tp VapeGuest {at[0]} {at[1]} {at[2]}')
                    guest.call('wait', '4')
                    guest.call('look-at', '3.5', '100.5', '4.5')
                    guest.call('wait', '2')
                storage_views.append((f'{distance}-gui{scale}', run))
        t.compare('render.storage-esp', 'storage-esp-open', storage_views)
        host.call('close')
        host.call('wait', '20')
        storage_views[0][1]()
        t.tap('render.storage-esp')
        guest.call('wait', '6')
        guest.capture('storage-esp-closed-near-gui2-on')
        t.tap('render.storage-esp')
        host.call('command', 'setblock 3 100 4 minecraft:air')
        host.call('command', 'tp VapeGuest 0.5 100 0.5 0 0')
        host.call('tp', '0.5', '100', '6.5')

        # ESP Outline, with Tracers' Enemy Only skipping the host as a
        # friend; then ESP 3D. Each relaunches the guest with the setting.
        guest.close()
        doc = guest.saved()
        doc['settings']['render.esp.mode'] = 'Outline'
        doc['settings']['render.tracers.enemy-only'] = True
        doc['friends'] = [{'name': 'VapeHost'}]
        join(doc)
        guest.call('wait', '20')
        t.compare('render.esp', 'esp-outline', ahead)
        guest.call('options-set', 'guiScale', '2')
        host.call('tp', '0.5', '100', '6.5')
        guest.call('look-at', '-3.5', '101', '6.5')
        # Until the join's teleport message has faded from chat; the sand
        # PropHunt's falling blocks dropped bobs between frames.
        guest.call('wait', '220')
        host.call('command', 'kill @e[type=item]')
        host.call('command', 'kill @e[type=falling_block]')
        guest.call('wait', '10')
        host.call('command', 'kill @e[type=item]')
        guest.call('wait', '10')
        friend = {}
        for phase in ('on', 'off'):
            t.tap('render.tracers')
            guest.call('wait', '6')
            guest.capture(f'tracers-enemy-only-friend-{phase}')
            friend[phase] = enabled(guest, 'render.tracers')
        # Between the view centre and the host (at the left), below the
        # drifting clouds and right of the host's idle sway: where a tracer
        # to the host would run.
        pixels = changed(guest.root / 'tracers-enemy-only-friend-on.png', guest.root / 'tracers-enemy-only-friend-off.png',
                         box=(160, 230, 800, 600))
        checks.check('tracers (Enemy Only): no tracer to a friend', friend['on'] and pixels < 40,
                     {'changed': pixels, **friend})

        guest.close()
        doc = guest.saved()
        doc['settings']['render.esp.mode'] = '3D'
        doc['settings']['render.murder-finder.callout'] = True
        join(doc)
        guest.call('wait', '20')
        t.compare('render.esp', 'esp-3d', ahead)
        # MurdererFinder's Callout, last so its chat line is in no capture:
        # the guest names the host (sword in hand) in the server's chat.
        t.tap('render.murder-finder')
        guest.call('wait', '40')
        said = host.call('chat-log', '10').get('chat', [])
        heard = guest.call('chat-log', '10').get('chat', [])
        guest.capture('murder-finder-callout')
        t.tap('render.murder-finder')
        callouts = ('VapeHost is the murderer!', 'i saw that VapeHost!')
        checks.check('murder finder (Callout): the guest names the host in chat',
                     any(c in line for line in said for c in callouts), {'host': said, 'guest': heard})
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
