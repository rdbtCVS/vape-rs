#!/usr/bin/env python3
"""KillAura, SilentAura and AimAssist against real players over LAN, with
Target mode Armor and Threat ordering two guest players.

A fresh host world is opened to LAN on localhost and two fresh guest clients
join it (each process with its own isolated homes), all running vape-rs.
Everyone is in survival with Resistance V. The host stands at the origin
facing +z with a wall at its back; the guests stand in obsidian booths open
toward the host, so knockback presses them into a back wall:

  NEAR  (VapeGuest)   2.0 blocks straight ahead (yaw 0)
  SIDE  (VapeGuest2)  2.83 blocks, 45 degrees to the left

Distance and Yaw would both choose NEAR. The host's hurt log shows whom each
aura hit; the host's view shows whom AimAssist turned to.

* Host launch 1, every module in Target mode Armor (the lowest armour score
  first, `EntityEquipmentValueComparator`): with NEAR in diamond armour and
  SIDE bare, KillAura and SilentAura hit only SIDE and AimAssist turns to it,
  SilentAura without moving the host's view; swapping the armour sends
  KillAura and AimAssist to NEAR.
* Guest NEAR runs the three modules (Target mode Distance) against the host
  over the remote path: KillAura hits the host, SilentAura hits it from 25
  degrees off while the guest's view stays put (the host renders the guest's
  head turned by the motion packets' rotation), AimAssist turns the guest's
  view onto the host.
* Host launch 2 (the guests rejoin), every module in Target mode Threat (the
  held weapon's score, times 1.375 * amplifier under Strength,
  `EntityArmorValueComparator`): NEAR holds a diamond sword and SIDE a wooden
  one, so SIDE is chosen; SIDE given a netherite sword sends them to NEAR;
  Strength I on SIDE scores it 0 (the reference's amplifier quirk), back to
  SIDE; Strength II scores the netherite sword at 1.375 times its damage,
  past the diamond sword, back to NEAR.

Results go to <NEAR guest session>/result.json, measurements to
measurements.json.
"""
import argparse
import json
import math
import os
import subprocess
import time

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 99
KILL, SILENT, AIM = 'combat.kill-aura', 'combat.silent-aura', 'combat.aim-assist'
BINDS = {KILL: 'KeyK', SILENT: 'KeyJ', AIM: 'KeyH'}
HOST = (0.5, 0.5, 0.0)
NEAR = ('VapeGuest', 0.5, 2.5, 180.0)
SIDE = ('VapeGuest2', -1.5, 2.5, -135.0)
# Back walls (x, z), two blocks high, where each player's knockback pushes
# it: NEAR's (+z), SIDE's (the -x +z diagonal) and the host's (-z). Nothing
# stands between the host and either guest.
WALLS = [(0, 3), (-1, 3), (-2, 3), (-3, 3), (-3, 2), (0, -1), (-1, -1), (1, -1)]
ARMOR = {'head': 'diamond_helmet', 'chest': 'diamond_chestplate',
         'legs': 'diamond_leggings', 'feet': 'diamond_boots'}


def yaw_to(origin, x, z):
    return -math.degrees(math.atan2(x - origin[0], z - origin[1]))


def wrap(angle):
    return (angle + 180.0) % 360.0 - 180.0


NEAR_YAW = yaw_to(HOST[:2], NEAR[1], NEAR[2])
SIDE_YAW = yaw_to(HOST[:2], SIDE[1], SIDE[2])


def gpu_free():
    out = subprocess.run(['nvidia-smi', '--query-gpu=memory.free', '--format=csv,noheader,nounits'],
                         capture_output=True, text=True, check=False).stdout.split()
    return int(out[0]) if out else 0


def gpu_wait(minimum=3000, seconds=900):
    """Wait for the GPU memory a launch needs (the shared-host rule)."""
    deadline = time.time() + seconds
    while gpu_free() < minimum:
        if time.time() > deadline:
            raise RuntimeError(f'GPU memory stayed under {minimum} MiB: {gpu_free()}')
        time.sleep(20)


def settings(mode, **extra):
    out = {}
    for module, bind in BINDS.items():
        out[f'{module}.binds'] = [bind]
        out[f'{module}.target-mode'] = mode
    out.update({
        f'{KILL}.show-target': True,
        f'{SILENT}.show-target': True,
        f'{AIM}.require-mouse': False,
        f'{AIM}.horizontal-speed': 10.0,
    })
    out.update(extra)
    return out


HOST_EXTRA = {f'{KILL}.max-angle': 360.0, f'{SILENT}.max-angle': 360.0, f'{AIM}.max-angle': 360.0}
# The guest faces the host; SIDE, 90 degrees to its side, is outside every
# module's angle.
GUEST_EXTRA = {f'{AIM}.max-angle': 90.0}


class Trio:
    def __init__(self, hs, near, side, checks):
        self.hs, self.ns, self.ss, self.checks = hs, near, side, checks
        self.host = Run(hs, checks, BINDS)
        self.near = Run(near, checks, BINDS)
        self.side = Run(side, checks, BINDS)
        self.address = None
        self.m = {}

    def command(self, text):
        self.host.call('command', text)

    def effects(self):
        self.command('effect give @a minecraft:resistance 100000 4 true')
        self.command('effect give @a minecraft:saturation 100000 4 true')

    def open_host(self, mode):
        r = self.host
        r.call('time', '6000')
        r.call('tp', HOST[0], FLOOR + 1, HOST[1])
        r.call('wait-ready', '40')
        floor = {}
        for _ in range(5):
            r.fill(-6, FLOOR - 2, -4, 6, FLOOR, 6, 'minecraft:obsidian')
            r.fill(-6, FLOOR + 1, -4, 6, FLOOR + 5, 6, 'minecraft:air')
            floor = r.call('blocks', -6, FLOOR, -4, 6, FLOOR, 6).get('counts', {})
            if floor.get('minecraft:obsidian') == 13 * 11:
                break
            r.wait(20)
        for x, z in WALLS:
            r.fill(x, FLOOR + 1, z, x, FLOOR + 2, z, 'minecraft:obsidian')
        walls = r.call('blocks', -3, FLOOR + 1, -1, 1, FLOOR + 2, 3).get('counts', {})
        self.checks.check(f'{mode}: obsidian floor and booths are in place',
                          floor.get('minecraft:obsidian') == 13 * 11
                          and walls.get('minecraft:obsidian') == 2 * len(WALLS), [floor, walls])
        self.command('gamerule mob_spawning false')
        self.command('difficulty peaceful')
        opened = r.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        self.checks.check(f'{mode}: host opened to LAN', bool(port), opened)
        self.address = f'localhost:{port}'

    def join(self, session, name, changes=None):
        if session.process is not None:
            session.call('quit')
            session.process.wait(timeout=15)
            session.log.close()
            session.process = None
            document = session.saved()
            document['settings'].update(changes or {})
            for module in BINDS:
                document['modules'].setdefault(module, {})['enabled'] = False
            session.seed(document)
        gpu_wait()
        session.launch(creative=False, world=None, extra=('--server', self.address, '--name', name))
        session.call('wait-ready', '60')

    def relaunch_host(self, changes):
        hs = self.hs
        hs.call('quit')
        hs.process.wait(timeout=15)
        hs.log.close()
        hs.process = None
        document = hs.saved()
        document['settings'].update(changes)
        for module in BINDS:
            document['modules'].setdefault(module, {})['enabled'] = False
        hs.seed(document)
        gpu_wait()
        hs.launch(creative=False, world='AuraHost')

    def place(self, host_yaw=HOST[2], near_yaw=NEAR[3]):
        for r in (self.host, self.near, self.side):
            r.release()
            r.call('mouse', 'left', 'up')
        for name, x, z, yaw in (NEAR, SIDE):
            self.command(f'tp {name} {x} {FLOOR + 1} {z} {yaw} 0')
        self.host.call('tp', HOST[0], FLOOR + 1, HOST[1])
        self.host.call('look', host_yaw, '0')
        self.near.wait(10)
        self.near.call('look', near_yaw, '0')
        self.side.call('look', SIDE[3], '0')
        self.near.wait(6)

    def hurts(self, ticks):
        """Each player's hurts the server logs over `ticks`, by role."""
        self.host.call('hurtlog')
        self.near.wait(ticks)
        out = {'host': [], 'near': [], 'side': []}
        for entry in self.host.call('hurtlog').get('players', []):
            role = {NEAR[0]: 'near', SIDE[0]: 'side'}.get(entry['name'], 'host')
            out[role] += entry.get('hurts', [])
        return out

    def wear(self, name, on):
        for slot, item in ARMOR.items():
            self.command(f'item replace entity {name} armor.{slot} with minecraft:{item if on else "air"}')

    def hold(self, name, item):
        self.command(f'item replace entity {name} weapon.mainhand with minecraft:{item}')

    def equipment(self, r):
        inv = r.call('inventory')
        return {'armor': [(a.get('item') if a else None) for a in inv.get('armor', [])],
                'held': (inv.get('held') or {}).get('item')}

    def optional(self, r, *args):
        try:
            return r.call(*args)
        except RuntimeError as error:
            return {'error': str(error)}

    def yaw(self, r):
        return r.call('state')['player']['yaw']

    def aura(self, label, r, module, want, ticks=60, capture=None):
        """`module` on `r` alone for `ticks`; only `want` is hurt."""
        view = self.yaw(r)
        r.toggle(module, True)
        if capture:
            r.wait(20)
            r.s.capture(capture)
        got = self.hurts(ticks)
        r.toggle(module, False)
        after = self.yaw(r)
        counts = {k: len(v) for k, v in got.items()}
        others = [k for k in counts if k != want]
        self.checks.check(f'{label}: hits only {want}',
                          counts[want] >= 1 and all(counts[k] == 0 for k in others), counts)
        self.m.setdefault('auras', []).append({'label': label, 'module': module, 'want': want,
                                               'hurts': counts, 'view': [view, after]})
        return counts, view, after

    def aim(self, label, r, want_yaw, other_yaw, start):
        r.call('look', start, '0')
        r.wait(4)
        r.toggle(AIM, True)
        yaws = []
        for _ in range(20):
            r.wait(10)
            yaws.append(self.yaw(r))
        r.toggle(AIM, False)
        settled = sum(yaws[-4:]) / 4
        error = abs(wrap(settled - want_yaw))
        self.checks.check(f'{label}: AimAssist settles on {want_yaw:.1f}',
                          error < 8.0 and error < abs(wrap(settled - other_yaw)),
                          {'settled': settled, 'want': want_yaw, 'other': other_yaw, 'trace': yaws[::4]})
        self.m.setdefault('aim', []).append({'label': label, 'settled': round(settled, 2),
                                             'want': round(want_yaw, 2), 'trace': yaws[::4]})
        return settled


def host_armor(p):
    host = p.host
    p.wear(NEAR[0], True)
    p.wear(SIDE[0], False)
    p.near.wait(10)
    gear = {'near': p.equipment(p.near), 'side': p.equipment(p.side)}
    p.checks.check('Armor: NEAR wears diamond armour, SIDE none',
                   all(gear['near']['armor']) and not any(gear['side']['armor']), gear)
    p.m['armor_gear'] = gear
    p.place()
    p.aura('host Armor KillAura (NEAR armoured)', host, KILL, 'side', capture='host-killaura-armor')
    p.place()
    _, view, after = p.aura('host Armor SilentAura (NEAR armoured)', host, SILENT, 'side',
                            capture='host-silentaura-armor')
    p.checks.check('host SilentAura leaves the host view where it was', abs(wrap(after - view)) < 1e-3,
                   [view, after])
    p.place()
    p.aim('host Armor (NEAR armoured)', host, SIDE_YAW, NEAR_YAW, NEAR_YAW)
    host.s.capture('host-aimassist-armor')

    p.wear(NEAR[0], False)
    p.wear(SIDE[0], True)
    p.near.wait(10)
    gear = {'near': p.equipment(p.near), 'side': p.equipment(p.side)}
    p.checks.check('Armor swapped: SIDE wears diamond armour, NEAR none',
                   all(gear['side']['armor']) and not any(gear['near']['armor']), gear)
    p.place()
    p.aura('host Armor KillAura (SIDE armoured)', host, KILL, 'near')
    p.place()
    p.aim('host Armor (SIDE armoured)', host, NEAR_YAW, SIDE_YAW, SIDE_YAW)
    p.wear(SIDE[0], False)


def guest_auras(p):
    near = p.near
    to_host = yaw_to((NEAR[1], NEAR[2]), HOST[0], HOST[1])
    p.place()
    p.aura('guest KillAura (remote)', near, KILL, 'host', capture='guest-killaura')
    # SilentAura from 25 degrees off (away from SIDE, which stays outside
    # the 120 degree angle): the guest's view stays, the host sees its head
    # turned toward it.
    off = wrap(to_host + 25.0)
    p.place(near_yaw=off)
    view = p.yaw(near)
    near.toggle(SILENT, True)
    near.wait(20)
    p.hs.capture('host-sees-guest-silentaura')
    got = p.hurts(60)
    near.toggle(SILENT, False)
    after = p.yaw(near)
    counts = {k: len(v) for k, v in got.items()}
    p.checks.check('guest SilentAura (remote) hits only host, 25 degrees off its view',
                   counts['host'] >= 1 and counts['near'] == 0 and counts['side'] == 0, counts)
    p.checks.check("guest SilentAura leaves the guest's view where it was",
                   abs(wrap(after - view)) < 1e-3 and abs(wrap(view - off)) < 0.5, [view, after, off])
    p.m['guest_silent'] = {'hurts': counts, 'view': [view, after], 'off': off}
    p.place()
    p.aim('guest (remote)', near, to_host, wrap(to_host - 90.0), wrap(to_host + 30.0))
    near.s.capture('guest-aimassist')


def item(name):
    return (name or '').removeprefix('minecraft:')


def host_threat(p):
    host = p.host
    rounds = [
        ('diamond vs wooden', ('diamond_sword', 'wooden_sword'), None, 'side'),
        ('SIDE netherite', ('diamond_sword', 'netherite_sword'), None, 'near'),
        ('SIDE netherite, Strength I', ('diamond_sword', 'netherite_sword'), 0, 'side'),
        ('SIDE netherite, Strength II', ('diamond_sword', 'netherite_sword'), 1, 'near'),
    ]
    for n, (label, (near_item, side_item), strength, want) in enumerate(rounds):
        p.hold(NEAR[0], near_item)
        p.hold(SIDE[0], side_item)
        if strength is not None:
            p.command(f'effect give {SIDE[0]} minecraft:strength 100000 {strength} true')
        p.near.wait(10)
        gear = {'near': p.equipment(p.near)['held'], 'side': p.equipment(p.side)['held'],
                'side_effects': p.optional(p.side, 'effects')}
        p.checks.check(f'Threat {label}: the guests hold {near_item} and {side_item}',
                       item(gear['near']) == near_item and item(gear['side']) == side_item,
                       gear)
        p.m.setdefault('threat_gear', []).append({'label': label, **gear})
        p.place()
        p.aura(f'host Threat KillAura ({label})', host, KILL, want,
               capture='host-killaura-threat' if n == 0 else None)
        if n == 0:
            p.place()
            p.aura(f'host Threat SilentAura ({label})', host, SILENT, want, capture='host-silentaura-threat')
        p.place()
        want_yaw, other = (SIDE_YAW, NEAR_YAW) if want == 'side' else (NEAR_YAW, SIDE_YAW)
        p.aim(f'host Threat ({label})', host, want_yaw, other, other)


def run(resources, allow_stale):
    with NativeSession(resources, 'aura-lan-host-', allow_stale=allow_stale) as hs, \
            NativeSession(resources, 'aura-lan-near-', allow_stale=allow_stale) as ns, \
            NativeSession(resources, 'aura-lan-side-', allow_stale=allow_stale) as ss:
        checks = Checks(ns.root)
        p = Trio(hs, ns, ss, checks)
        hs.seed({'schema': 1, 'modules': {}, 'settings': settings('Armor', **HOST_EXTRA)})
        gpu_wait()
        hs.launch(creative=False, world='AuraHost')
        p.open_host('launch 1')
        ns.seed({'schema': 1, 'modules': {}, 'settings': settings('Distance', **GUEST_EXTRA)})
        ss.seed({'schema': 1, 'modules': {}, 'settings': {}})
        p.join(ns, NEAR[0])
        p.join(ss, SIDE[0])
        p.near.wait(20)
        p.effects()
        p.place()
        for name, r in (('NEAR', p.near), ('SIDE', p.side)):
            found = r.call('entities', '4', 'player').get('entities', [])
            checks.check(f'{name} sees the other two players within 4 blocks', len(found) >= 2, found)

        host_armor(p)
        guest_auras(p)

        p.relaunch_host(settings('Threat', **HOST_EXTRA))
        p.open_host('launch 2')
        p.join(ns, NEAR[0])
        p.join(ss, SIDE[0])
        p.near.wait(20)
        p.effects()
        host_threat(p)

        hs.capture('host-final')
        for name, session in (('host', hs), ('NEAR', ns), ('SIDE', ss)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
        (ns.root / 'measurements.json').write_text(json.dumps(p.m, indent=2, default=str))
        for session in (ns, ss):
            session.call('disconnect')
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}', ns.root)
        for c in checks.results:
            if not c['ok']:
                print('FAILED', c['check'], json.dumps(c.get('detail'), default=str)[:400])
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
