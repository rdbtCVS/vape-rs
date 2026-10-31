#!/usr/bin/env python3
"""Movement modules in a fresh isolated survival world on the integrated server.

One session covers Timer, Fly, NoFall, Speed, LongJump, Step, AntiFall,
NoSlowdown and Anti-AFK. Each module is seeded with its own bind and toggled
through it mid-run; every check reads the host's own state: client ticks
against the server's game time, player positions and motion, the server's
hurt log (fall damage) and the view rotation. The second phase relaunches the
same home with NoFall AntiCheat and Speed AntiCheat B. Results go to
result.json and measurements.json.
"""
import argparse
import json
import math
import os
import time

from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 149
BINDS = {
    'blatant.timer': 'KeyV',
    'blatant.fly': 'KeyK',
    'blatant.no-fall': 'KeyP',
    'blatant.speed': 'KeyO',
    'blatant.long-jump': 'KeyU',
    'blatant.step': 'KeyG',
    'blatant.anti-fall': 'KeyH',
    'blatant.no-slowdown': 'KeyJ',
    'world.anti-afk': 'KeyN',
}
SETTINGS = {
    'blatant.timer.speed': 2.0,
    'world.anti-afk.start-delay': [10.0, 10.0],
}


class Run:
    def __init__(self, session, checks, binds=None):
        self.s = session
        self.checks = checks
        self.binds = binds or BINDS
        self.m = {}

    def call(self, *args):
        return self.s.call(*map(str, args))

    def wait(self, ticks):
        self.call('wait', str(ticks))

    def player(self):
        return self.call('state')['player']

    def pos(self):
        return self.player()['position']

    def enabled(self, module):
        return self.s.saved()['modules'].get(module, {}).get('enabled', False)

    def toggle(self, module, want):
        if self.enabled(module) != want:
            self.call('key', self.binds[module][3:].lower(), 'tap')
            self.wait(2)
        ok = self.enabled(module) == want
        self.checks.check(f'{module} {"enabled" if want else "disabled"} through its bind', ok)
        return ok

    def release(self):
        for key in ('w', 'space', 'ctrl', 'shift'):
            self.call('key', key, 'up')
        self.call('mouse', 'right', 'up')

    def reset(self, x=0.5, z=0.5, yaw=0, y=FLOOR + 1):
        self.release()
        # A server-side teleport: the client tp is a move the server rejects
        # when it is long or passes through blocks.
        self.call('chat', f'/tp @s {x} {y} {z} {yaw} 0')
        self.wait(4)
        self.call('look', str(yaw), '0')
        self.wait(6)
        self.call('hurtlog')

    def give(self, item, count=1):
        """Give through the server and select it once it has arrived."""
        self.call('give', f'minecraft:{item}', count)
        for _ in range(20):
            self.wait(4)
            try:
                self.call('select', item)
                return True
            except RuntimeError:
                pass
        self.checks.check(f'{item} arrived in the inventory', False)
        return False

    def fill(self, *args):
        self.call('fill', *map(str, args))

    def arena(self):
        for _ in range(10):
            self.fill(-4, FLOOR, -30, 9, FLOOR, 30, 'minecraft:stone')
            self.fill(-4, FLOOR + 1, -30, 9, FLOOR + 11, 30, 'minecraft:air')
            # AntiFall's void: a column with nothing below the edge.
            for y0 in range(-64, FLOOR + 12, 100):
                self.fill(10, y0, -3, 14, min(y0 + 99, FLOOR + 11), 3, 'minecraft:air')
            counts = self.call('blocks', -4, FLOOR, -30, 9, FLOOR, 30).get('counts', {})
            void = self.call('blocks', 10, -64, -3, 14, FLOOR + 11, 3).get('counts', {})
            if counts.get('minecraft:stone') == 14 * 61 and set(void) <= {'minecraft:air'}:
                break
            self.wait(20)
        self.checks.check('arena and void column are in place',
                          counts.get('minecraft:stone') == 14 * 61 and set(void) <= {'minecraft:air'},
                          {'floor': counts, 'void': void})

    def walk(self, ticks, keys=('w',), step=2):
        """Hold keys for `ticks`; return (horizontal distance per `ticks`
        client ticks, y samples, end position). Control calls take wall time
        while the client keeps ticking (more under load), so the distance is
        scaled by the client ticks that really passed between the samples."""
        for key in keys:
            self.call('key', key, 'down')
        self.wait(1)
        t0, start = self.call('status')['ticks'], self.pos()
        ys = []
        for _ in range(0, ticks, step):
            self.wait(step)
            ys.append(self.pos()[1])
        end, t1 = self.pos(), self.call('status')['ticks']
        self.release()
        self.last_walk = {'ticks': t1 - t0, 'max_y': max(ys) if ys else None}
        raw = math.hypot(end[0] - start[0], end[2] - start[2])
        return raw * ticks / max(t1 - t0, 1), ys, end

    def falls(self):
        hurts = []
        for entry in self.call('hurtlog').get('players', []):
            hurts += [h for h in entry.get('hurts', []) if 'fall' in str(h.get('source', ''))]
        return hurts

    def heal(self):
        self.call('chat', '/effect give @s minecraft:instant_health 1 5 true')
        self.wait(4)


def timer(r):
    def rate():
        t0, g0 = r.call('status')['ticks'], r.call('simulation')['game_time']
        time.sleep(3.0)
        t1, g1 = r.call('status')['ticks'], r.call('simulation')['game_time']
        return (t1 - t0) / max(g1 - g0, 1), t1 - t0, g1 - g0
    off = rate()
    r.toggle('blatant.timer', True)
    on = rate()
    r.toggle('blatant.timer', False)
    after = rate()
    r.m['timer'] = {'off': off, 'on': on, 'after': after}
    r.checks.check('Timer off: client ticks follow the server', 0.85 < off[0] < 1.15, off)
    r.checks.check('Timer 2.0: the client runs about twice as many ticks', 1.7 < on[0] < 2.3, on)
    r.checks.check('Timer disabled: client rate restored', 0.85 < after[0] < 1.15, after)


def fly(r):
    r.reset(z=-20)
    r.toggle('blatant.fly', True)
    r.call('key', 'space', 'down')
    r.wait(10)
    r.call('key', 'space', 'up')
    risen = r.pos()[1]
    r.wait(4)
    a = r.pos()
    r.wait(30)
    b = r.player()
    hover = abs(b['position'][1] - a[1])
    distance, ys, _ = r.walk(20)
    r.toggle('blatant.fly', False)
    r.wait(30)
    fell = r.pos()[1]
    r.m['fly'] = {'risen': risen, 'hover_dy': hover, 'walk20': distance, 'fell_to': fell}
    r.checks.check('Fly: holding jump rises', risen > FLOOR + 2.5, risen)
    r.checks.check('Fly: no input holds altitude', hover < 0.1, {'dy': hover, 'velocity': b['velocity']})
    r.checks.check('Fly: forward input moves about Speed 0.5 per tick', 7.0 < distance < 12.0, distance)
    r.checks.check('Fly disabled: gravity returns', fell < risen - 1.5, {'risen': risen, 'now': fell})
    r.heal()


def no_fall(r, label):
    r.reset(z=10)
    r.call('chat', f'/tp @s 0.5 {FLOOR + 14} 10.5')
    r.wait(50)
    off = r.falls()
    r.heal()
    r.toggle('blatant.no-fall', True)
    r.reset(z=10)
    r.call('chat', f'/tp @s 0.5 {FLOOR + 14} 10.5')
    r.wait(50)
    on = r.falls()
    landed = r.player()
    r.toggle('blatant.no-fall', False)
    r.heal()
    r.m[f'no_fall_{label}'] = {'off': off, 'on': on}
    total = sum(h.get('taken', 0) for h in on)
    before = sum(h.get('taken', 0) for h in off)
    r.checks.check(f'NoFall {label}: a 13-block drop hurts without it', before >= 5, off)
    if label == 'Normal':
        r.checks.check('NoFall Normal: the same drop deals no fall damage',
                       total < 1.0, {'hurts': on, 'on_ground': landed['on_ground']})
    else:
        # The on-ground/off-ground status pair every 3 blocks makes the server
        # apply each 3.x-block segment (ceil(fall - 3) = 1, mostly inside the
        # hurt cooldown) instead of the whole drop, as in the reference.
        r.checks.check('NoFall AntiCheat: the same drop deals at most a fifth of the damage',
                       total <= before * 0.2, {'off': before, 'on': total, 'hurts': on})


def speed(r, label, walk_off):
    r.toggle('blatant.speed', True)
    r.reset(z=-25)
    distance, ys, _ = r.walk(40)
    r.toggle('blatant.speed', False)
    r.reset(z=-25)
    after, _, _ = r.walk(40)
    hops = sum(1 for a, b in zip(ys, ys[1:]) if b > a + 0.05)
    r.m[f'speed_{label}'] = {'on': distance, 'walk': walk_off, 'after': after, 'ys': ys}
    r.checks.check(f'Speed {label}: 40 forward ticks cover more than walking', distance > walk_off['walk'] * 1.3,
                   {'on': distance, **walk_off})
    if label == 'Bhop':
        r.checks.check('Speed Bhop: hops while moving', hops >= 2, hops)
    r.checks.check(f'Speed {label} disabled: walking distance restored', abs(after - walk_off['walk']) < 1.5,
                   {'after': after, 'walk': walk_off['walk']})


def baselines(r):
    r.reset(z=-25)
    walk, _, _ = r.walk(40)
    r.reset(z=-25)
    sprint, _, _ = r.walk(40, keys=('ctrl', 'w'))
    return {'walk': walk, 'sprint': sprint}


def first_jump(r, keys, jump_after=None):
    """Hold keys (and space after `jump_after` ticks); horizontal distance of
    the first airborne segment."""
    for key in keys:
        r.call('key', key, 'down')
    takeoff = landing = None
    last = r.player()
    for tick in range(80):
        if tick == jump_after:
            r.call('key', 'space', 'down')
        r.wait(1)
        p = r.player()
        if takeoff is None and last['on_ground'] and not p['on_ground']:
            takeoff = last['position']
        if takeoff is not None and p['on_ground']:
            landing = p['position']
            break
        last = p
    r.release()
    if takeoff is None or landing is None:
        return None
    return math.hypot(landing[0] - takeoff[0], landing[2] - takeoff[2])


def long_jump(r):
    r.reset(z=-25)
    jump = first_jump(r, ('ctrl', 'w'), jump_after=10)
    r.toggle('blatant.long-jump', True)
    r.reset(z=-25)
    boosted = first_jump(r, ('w',))
    r.wait(6)
    off_after = not r.enabled('blatant.long-jump')
    r.m['long_jump'] = {'sprint_jump': jump, 'long_jump': boosted}
    r.checks.check('LongJump: the first jump flies farther than a sprint jump',
                   jump and boosted and boosted > jump * 1.3, {'sprint_jump': jump, 'long_jump': boosted})
    r.checks.check('LongJump Toggle: turns itself off after landing', off_after)
    if not off_after:
        r.toggle('blatant.long-jump', False)


def step(r):
    r.fill(-4, FLOOR + 1, 5, 9, FLOOR + 1, 5, 'minecraft:stone')
    r.reset()
    _, _, off = r.walk(30)
    r.toggle('blatant.step', True)
    r.reset()
    _, ys, on = r.walk(30)
    climbed = max(ys)
    r.toggle('blatant.step', False)
    r.reset()
    _, _, after = r.walk(30)
    r.fill(-4, FLOOR + 1, 5, 9, FLOOR + 1, 5, 'minecraft:air')
    r.m['step'] = {'off': off, 'on': on, 'after': after}
    r.checks.check('Step off: a 1-block wall stops walking', off[2] < 5 and off[1] < FLOOR + 1.5, off)
    r.checks.check('Step on: walking climbs the 1-block wall', on[2] > 5.5 and climbed >= FLOOR + 1.9,
                   {'end': on, 'max_y': climbed})
    r.checks.check('Step disabled: the wall stops walking again', after[2] < 5 and after[1] < FLOOR + 1.5, after)


def anti_fall(r):
    """Walk off the platform edge over the cleared void column. The
    reference lagback raises the position by the fall distance and zeroes
    horizontal motion only (vertical motion stays); on an anticheat server
    the server's setback finishes it, while this vanilla server accepts the
    raised position, so the player bobs in a band below the edge instead of
    falling into the void."""
    def off_edge(ticks):
        r.reset(x=8.5, z=0.5, yaw=-90)
        r.call('key', 'w', 'down')
        r.wait(12)
        r.call('key', 'w', 'up')
        low = r.pos()[1]
        for _ in range(0, ticks, 2):
            r.wait(2)
            low = min(low, r.pos()[1])
            if low < FLOOR - 30:
                break
        return low
    off = off_edge(50)
    r.reset()
    r.heal()
    r.toggle('blatant.anti-fall', True)
    on = off_edge(50)
    end = r.pos()
    # Back on the platform before disabling, or the player drops into the void.
    r.reset()
    r.toggle('blatant.anti-fall', False)
    r.heal()
    r.m['anti_fall'] = {'off_lowest': off, 'on_lowest': on, 'on_end': end}
    r.checks.check('AntiFall off: walking off the edge falls into the void', off < FLOOR - 25, off)
    r.checks.check('AntiFall on: lagbacks hold the player within 10 blocks below the edge for 50 ticks',
                   on > FLOOR - 10, {'lowest': on, 'end': end})


def no_slowdown(r):
    r.give('shield')

    def blocking_walk():
        r.reset(z=-25)
        r.call('look', '0', '-90')
        r.call('mouse', 'right', 'down')
        r.wait(6)
        using = r.call('state').get('using')
        distance, _, _ = r.walk(30)
        r.call('mouse', 'right', 'up')
        return distance, using
    off, using_off = blocking_walk()
    r.toggle('blatant.no-slowdown', True)
    on, using_on = blocking_walk()
    r.toggle('blatant.no-slowdown', False)
    after, _ = blocking_walk()
    r.reset(z=-25)
    walk, _, _ = r.walk(30)
    r.m['no_slowdown'] = {'off': off, 'on': on, 'after': after, 'walk': walk,
                          'using': [using_off, using_on]}
    r.checks.check('NoSlowdown off: walking with a raised shield is slowed', bool(using_off) and off < walk * 0.4,
                   {'blocking': off, 'walk': walk, 'using': using_off})
    r.checks.check('NoSlowdown on: the raised shield keeps walking speed', bool(using_on) and on > walk * 0.85,
                   {'blocking': on, 'walk': walk, 'using': using_on})
    r.checks.check('NoSlowdown disabled: shield slowdown restored', after < walk * 0.4, after)


def anti_afk(r):
    r.reset()
    r.toggle('world.anti-afk', True)
    start = r.player()
    moved_at = None
    t0 = time.monotonic()
    while time.monotonic() - t0 < 25:
        time.sleep(1)
        p = r.player()
        if abs(p['yaw'] - start['yaw']) > 0.5 or abs(p['pitch'] - start['pitch']) > 0.5 or \
                math.hypot(p['position'][0] - start['position'][0], p['position'][2] - start['position'][2]) > 0.05:
            moved_at = time.monotonic() - t0
            break
    later = r.player()
    r.wait(40)
    end = r.player()
    drift = math.hypot(end['position'][0] - start['position'][0], end['position'][2] - start['position'][2])
    r.toggle('world.anti-afk', False)
    r.m['anti_afk'] = {'moved_after_s': moved_at, 'start': start, 'later': later, 'end': end}
    r.checks.check('Anti-AFK: an idle player is turned or moved after the 10 s start delay',
                   moved_at is not None and moved_at > 7, moved_at)
    r.checks.check('Anti-AFK: the player stays close to the start', drift < 4.0, drift)


def relaunch(session, changes):
    session.call('quit')
    session.process.wait(timeout=15)
    session.log.close()
    session.process = None
    document = session.saved()
    document['settings'].update(changes)
    session.seed(document)
    session.launch(creative=True)
    session.call('chat', '/gamemode survival')
    session.call('wait', '10')


def run(resources, allow_stale):
    with NativeSession(resources, 'movement-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        # The blatant modules are hidden by default; unhide them as the GUI would.
        hidden = ('blatant.timer', 'blatant.fly', 'blatant.speed')
        session.seed({'schema': 1, 'modules': {m: {'visible': True} for m in hidden},
                      'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()}, **SETTINGS}})
        session.launch(creative=True)
        session.call('chat', '/gamemode survival')
        session.call('time', '6000')
        session.call('chat', '/gamerule doDaylightCycle false')
        r = Run(session, checks)
        r.wait(20)
        r.arena()
        r.call('chat', '/effect give @s minecraft:saturation 100000 4 true')
        r.reset()
        timer(r)
        fly(r)
        no_fall(r, 'Normal')
        walk = baselines(r)
        r.m['baselines'] = walk
        speed(r, 'Bhop', walk)
        long_jump(r)
        step(r)
        anti_fall(r)
        no_slowdown(r)
        anti_afk(r)
        alive = session.call('state')
        checks.check('still in the world after phase 1 (no kick)', alive.get('player') is not None,
                     alive.get('screen'))

        relaunch(session, {'blatant.no-fall.mode': 'AntiCheat', 'blatant.speed.mode': 'AntiCheat B'})
        r.reset()
        r.wait(10)
        no_fall(r, 'AntiCheat')
        speed(r, 'AntiCheat B', walk)
        session.capture('final')
        alive = session.call('state')
        checks.check('still in the world after phase 2 (no kick)', alive.get('player') is not None,
                     alive.get('screen'))
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
