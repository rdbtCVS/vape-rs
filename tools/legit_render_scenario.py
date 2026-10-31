#!/usr/bin/env python3
"""Render and legit modules in a fresh isolated creative world on the
integrated server, measured from the host's own frames and state.

* Time Changer (hour 0): the client's time of day reads 18000 (midnight)
  while the server stays at noon, and the sky frame darkens; off, the
  server's time is back.
* Weather (Raining) under a clear server sky: the sky frame greys; off, it
  is clear again.
* Fullbright (Night Vision, then Gamma) in a sealed dark stone room at
  night: the frame brightens; off, it is dark again.
* Clear Water: with the camera under water the frame clears (the fog moves
  out); off, the water fog is back.
* AntiDebuff: slowness, nausea and blindness leave the client effect list
  and slowness its speed modifier (walking speed is normal again).
* ItemESP and NameTags (Render Animals): a dropped item behind a wall and a
  still cow gain tags in the frame; off, the frame matches the plain one.

Each module is toggled through its bind. Frames are compared by mean
luminance or mean absolute pixel difference. Results go to result.json and
measurements.json, frames next to them.
"""
import argparse
import json
import os
import time

from PIL import Image, ImageChops, ImageStat

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 149
BINDS = {
    'legit.time-changer': 'KeyV',
    'legit.weather': 'KeyK',
    'render.fullbright': 'KeyP',
    'render.clear-water': 'KeyO',
    'render.anti-debuff': 'KeyU',
    'render.item-esp': 'KeyG',
    'render.name-tags': 'KeyH',
}
SETTINGS = {
    'legit.time-changer.time': 0.0,
    'legit.weather.weather': 'Raining',
    'render.name-tags.animals': True,
    # Slowness is only removed with Remove Effects on (as in the reference).
    'render.anti-debuff.effects': True,
}


class Frames(Run):
    def frame(self, name, settle=10):
        self.wait(settle)
        path = self.s.root / f'{name}.png'
        path.unlink(missing_ok=True)
        self.call('screenshot', str(path))
        for _ in range(50):
            if path.exists() and path.stat().st_size > 0:
                try:
                    with Image.open(path) as img:
                        return img.convert('RGB').copy()
                except OSError:
                    pass
            time.sleep(0.1)
        raise RuntimeError(f'no screenshot {name}')


# The middle of the 800x600 frame: clear of the chat (bottom left), the
# tutorial toast (top right), the hand and the hotbar.
CENTRE = (100, 60, 700, 380)


# Around the crosshair, where the tags over the item and the cow are drawn
# (clear of the drifting clouds).
TAGS = (250, 120, 550, 340)


def luma(img, box=CENTRE):
    return ImageStat.Stat((img.crop(box) if box else img).convert('L')).mean[0]


def diff(a, b, box=CENTRE):
    if box:
        a, b = a.crop(box), b.crop(box)
    return sum(ImageStat.Stat(ImageChops.difference(a, b)).mean) / 3


def time_changer(r):
    r.reset()
    r.call('look', '0', '-45')
    off = r.frame('time-off')
    t_off = r.call('state')['time']['time_of_day']
    r.toggle('legit.time-changer', True)
    on = r.frame('time-on')
    t_on = r.call('state')['time']['time_of_day']
    server = r.call('simulation').get('game_time')
    r.toggle('legit.time-changer', False)
    r.wait(40)
    after = r.frame('time-after')
    t_after = r.call('state')['time']['time_of_day']
    sky = [luma(off), luma(on), luma(after)]
    r.m['time_changer'] = {'time_of_day': [t_off, t_on, t_after], 'sky_luma': sky, 'server_game_time': server}
    r.checks.check('Time Changer off: the client shows the server noon', abs(t_off - 6000) < 200, t_off)
    r.checks.check('Time Changer hour 0: the client time of day reads midnight (18000)', t_on == 18000, t_on)
    r.checks.check('Time Changer hour 0: the sky frame is much darker', sky[1] < sky[0] * 0.5, sky)
    r.checks.check('Time Changer disabled: the server time and bright sky are back',
                   abs(t_after - 6000) < 300 and sky[2] > sky[0] * 0.85, {'time': t_after, 'sky': sky})


def weather(r):
    r.reset()
    r.call('look', '0', '-45')
    off = r.frame('weather-off')
    r.toggle('legit.weather', True)
    on = r.frame('weather-on', settle=20)
    raining = r.call('state')['weather']
    r.toggle('legit.weather', False)
    after = r.frame('weather-after', settle=20)
    m = {'sky_luma': [luma(off), luma(on), luma(after)], 'diff_on': diff(off, on), 'diff_after': diff(off, after),
         'server_weather_while_on': raining}
    r.m['weather'] = m
    r.checks.check('Weather Raining: the clear sky frame turns rainy (darker, changed)',
                   m['sky_luma'][1] < m['sky_luma'][0] * 0.9 and m['diff_on'] > 8, m)
    r.checks.check('Weather Raining: the server weather itself stays clear', not raining.get('raining'), raining)
    r.checks.check('Weather disabled: the clear sky is back', m['diff_after'] < m['diff_on'] * 0.3, m)


def fullbright(r, mode):
    # A sealed stone room at midnight; no light inside.
    r.call('time', '18000')
    r.fill(-3, FLOOR + 1, 6, 3, FLOOR + 5, 12, 'minecraft:stone')
    r.fill(-2, FLOOR + 2, 7, 2, FLOOR + 4, 11, 'minecraft:air')
    r.wait(10)
    r.call('chat', f'/tp @s 0.5 {FLOOR + 2} 9.5 0 0')
    r.wait(10)
    r.call('look', '0', '10')
    dark = r.frame(f'fullbright-{mode}-off', settle=30)
    r.toggle('render.fullbright', True)
    on = r.frame(f'fullbright-{mode}-on', settle=20)
    r.toggle('render.fullbright', False)
    back = r.frame(f'fullbright-{mode}-after', settle=20)
    lum = [luma(x) for x in (dark, on, back)]
    r.m[f'fullbright_{mode}'] = {'luma_off_on_after': lum}
    r.checks.check(f'Fullbright {mode}: the dark room brightens', lum[1] > lum[0] * 2 + 10, lum)
    r.checks.check(f'Fullbright {mode} disabled: the room is dark again', lum[2] < lum[0] * 1.2 + 3, lum)
    r.fill(-3, FLOOR + 1, 6, 3, FLOOR + 5, 12, 'minecraft:air')
    r.call('time', '6000')
    r.wait(10)


def clear_water(r):
    # An open-topped stone tank, so the water neither spreads nor flows.
    r.fill(-4, FLOOR + 1, -13, 4, FLOOR + 6, -3, 'minecraft:stone')
    r.fill(-3, FLOOR + 1, -12, 3, FLOOR + 6, -4, 'minecraft:water')
    r.wait(10)
    r.call('chat', f'/tp @s 0.5 {FLOOR + 1} -11.5 0 0')
    r.call('look', '0', '0')
    off = r.frame('water-off', settle=30)
    r.toggle('render.clear-water', True)
    on = r.frame('water-on', settle=20)
    r.toggle('render.clear-water', False)
    # Vanilla's underwater view widens the longer the camera is under water,
    # so the camera leaves the water and dives again before the frame after,
    # taken as long after the dive as the one before.
    r.call('chat', f'/tp @s 0.5 {FLOOR + 8} -11.5 0 0')
    r.wait(20)
    r.call('chat', f'/tp @s 0.5 {FLOOR + 1} -11.5 0 0')
    r.call('look', '0', '0')
    after = r.frame('water-after', settle=30)
    m = {'diff_on': diff(off, on), 'diff_after': diff(off, after), 'diff_after_on': diff(after, on),
         'luma': [luma(off), luma(on), luma(after)]}
    r.m['clear_water'] = m
    r.checks.check('Clear Water: the underwater frame changes (fog pushed out)', m['diff_on'] > 6, m)
    r.checks.check('Clear Water disabled: the water fog is back', m['diff_after'] < m['diff_on'] * 0.4, m)
    r.fill(-4, FLOOR + 1, -13, 4, FLOOR + 6, -3, 'minecraft:air')
    r.wait(20)


def anti_debuff(r):
    r.reset(z=-20)
    r.call('chat', '/gamemode survival')
    walk, _, _ = r.walk(30)

    def debuffed():
        r.call('chat', '/effect give @s minecraft:slowness 60 3 true')
        r.call('chat', '/effect give @s minecraft:nausea 60 0 true')
        r.call('chat', '/effect give @s minecraft:blindness 60 0 true')
        r.wait(6)
        ids = sorted(e['id'] for e in r.player().get('effects', []))
        r.reset(z=-20)
        d, _, _ = r.walk(30)
        return ids, d
    off_ids, off_d = debuffed()
    r.call('chat', '/effect clear @s')
    r.wait(4)
    r.toggle('render.anti-debuff', True)
    on_ids, on_d = debuffed()
    r.toggle('render.anti-debuff', False)
    r.call('chat', '/effect clear @s')
    r.wait(4)
    after_ids, after_d = debuffed()
    r.call('chat', '/effect clear @s')
    r.call('chat', '/gamemode creative')
    r.m['anti_debuff'] = {'walk': walk, 'off': [off_ids, off_d], 'on': [on_ids, on_d], 'after': [after_ids, after_d]}
    bad = {'slowness', 'nausea', 'blindness'}
    r.checks.check('AntiDebuff off: the three debuffs are on and slowness slows walking',
                   bad <= set(off_ids) and off_d < walk * 0.6, {'ids': off_ids, 'walk': walk, 'slowed': off_d})
    r.checks.check('AntiDebuff on: none of them stays in the client list, walking speed is normal',
                   not (bad & set(on_ids)) and on_d > walk * 0.9, {'ids': on_ids, 'walk': walk, 'on': on_d})
    r.checks.check('AntiDebuff disabled: new debuffs stay and slow again',
                   bad <= set(after_ids) and after_d < walk * 0.6, {'ids': after_ids, 'after': after_d})


def tags(r):
    r.reset()
    # A wall two blocks ahead with a dropped diamond behind it; a still cow
    # four blocks behind the player.
    r.fill(-3, FLOOR + 1, 3, 3, FLOOR + 3, 3, 'minecraft:stone')
    r.call('chat', f'/kill @e[type=minecraft:item]')
    r.call('chat', f'/summon minecraft:item 0.5 {FLOOR + 1} 6.5 {{Item:{{id:"minecraft:diamond",count:3}},PickupDelay:32767,Age:-32768}}')
    r.call('chat', f'/summon minecraft:cow 0.5 {FLOOR + 1} -4.5 {{NoAI:1b}}')
    r.wait(10)
    r.call('look', '0', '15')
    plain = r.frame('tags-off', settle=20)
    r.toggle('render.item-esp', True)
    esp = r.frame('item-esp-on')
    r.toggle('render.item-esp', False)
    esp_after = r.frame('item-esp-after')
    r.call('look', '180', '15')
    plain_cow = r.frame('name-tags-off')
    r.toggle('render.name-tags', True)
    named = r.frame('name-tags-on')
    r.toggle('render.name-tags', False)
    named_after = r.frame('name-tags-after')
    m = {'item_esp': [diff(plain, esp, TAGS), diff(plain, esp_after, TAGS)],
         'name_tags': [diff(plain_cow, named, TAGS), diff(plain_cow, named_after, TAGS)]}
    r.m['tags'] = m
    r.checks.check('ItemESP: a tag over the item behind the wall appears in the frame', m['item_esp'][0] > 0.3, m)
    r.checks.check('ItemESP disabled: the frame is plain again', m['item_esp'][1] < m['item_esp'][0] * 0.3, m)
    r.checks.check('NameTags (animals): a tag over the cow appears in the frame', m['name_tags'][0] > 0.3, m)
    r.checks.check('NameTags disabled: the frame is plain again', m['name_tags'][1] < m['name_tags'][0] * 0.3, m)
    r.fill(-3, FLOOR + 1, 3, 3, FLOOR + 3, 3, 'minecraft:air')
    r.call('chat', '/kill @e[type=!minecraft:player]')


def run(resources, allow_stale):
    with NativeSession(resources, 'legit-render-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()}, **SETTINGS}})
        session.launch(creative=True)
        session.call('time', '6000')
        session.call('chat', '/gamerule advance_time false')
        session.call('chat', '/gamerule advance_weather false')
        session.call('chat', '/weather clear')
        session.call('chat', '/difficulty peaceful')
        r = Frames(session, checks, BINDS)
        r.wait(20)
        r.arena()
        r.call('chat', '/effect give @s minecraft:saturation 100000 4 true')
        r.call('hotbar', '1')
        time_changer(r)
        weather(r)
        fullbright(r, 'night-vision')
        clear_water(r)
        anti_debuff(r)
        tags(r)
        # Gamma mode: the setting is only reachable through the GUI at
        # runtime, so relaunch the same home with it.
        session.call('quit')
        session.process.wait(timeout=15)
        session.log.close()
        session.process = None
        doc = session.saved()
        doc['settings']['render.fullbright.mode'] = 'gamma'
        session.seed(doc)
        session.launch(creative=True)
        r.wait(20)
        checks.check('gamma mode seeded', session.saved()['settings'].get('render.fullbright.mode') == 'gamma')
        fullbright(r, 'gamma')
        alive = session.call('state')
        checks.check('still in the world (no kick)', alive.get('player') is not None, alive.get('screen'))
        log = (session.root / 'game.log').read_text(errors='replace')
        checks.check('no panics in the log', 'panicked' not in log and 'poisoned' not in log)
        (session.root / 'measurements.json').write_text(json.dumps(r.m, indent=2, default=str))
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}',
              session.root)
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
