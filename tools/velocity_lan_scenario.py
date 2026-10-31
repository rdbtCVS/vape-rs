#!/usr/bin/env python3
"""Velocity over a real LAN connection.

A fresh host world is opened to LAN on localhost and a fresh guest client
runs vape-rs against it (each process with its own isolated homes). Velocity
runs on the guest; the host player, one block across and three ahead, is the
opponent whose view decides "Only when targeting" and Kite (`RotationUtil.H`:
the opponent looks at us; `RotationUtil.F`: we look at the opponent). The
players are off a shared axis: the reference's yaw-to helper leaves the angle
at 0 when either delta is exactly 0, so on one line the opponent's view would
read backwards (the port keeps that).

Knockback comes from the two remote server paths:

* motion: the guest (survival, Resistance V) is hurt by `/damage ... at`
  a point on its +x side (mob_attack: `generic` is in `no_knockback`), so the server sends the guest's own
  ClientboundSetEntityMotion pushing it along -x (perpendicular to both
  views);
* explosion: the guest in creative (not flying) is pushed by primed TNT on
  its -x side but never hurt, so the push arrives in the Explode packet's
  knockback field (the `remote_explode` hook). An obsidian wall shades the
  host from the blast.

Each case is the guest's largest horizontal displacement over 30 ticks.
Settings (Chance 100, Horizontal 0, Vertical 0, Ticks 0) change through a
guest relaunch that rejoins the same server:

* defaults: off pushes, on cancels both paths;
* Only when targeting: motion is kept when neither player looks at the
  other and cancelled when either does; the explosion push is kept when only
  the opponent looks at us and cancelled when neither does (the reference
  quirk of `applyBridgeVelocity`);
* Kite (200% horizontal): the opponent looking at us while we look away
  doubles both pushes; facing each other cancels the push again;
* Always Kite: facing each other doubles the push too.

Results go to <guest session>/result.json, measurements to
measurements.json.
"""
import argparse
import json
import math
import os

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 99
GUEST = (0.5, 0.5)
HOST = (1.5, 3.5)
BINDS = {'combat.velocity': 'KeyV'}
FULL = {'combat.velocity.chance': 100.0, 'combat.velocity.horizontal': 0.0,
        'combat.velocity.vertical': 0.0, 'combat.velocity.ticks': 0.0}
# Yaw 0 faces +z, 18 degrees off the host; yaw 180 turns the host 18 degrees
# off the guest. The other yaw of each is over 160 degrees off.
GUEST_YAW = {True: 0, False: 180}
HOST_YAW = {True: 180, False: 0}


class Pair:
    def __init__(self, hs, gs, checks, address):
        self.hs, self.gs, self.checks, self.address = hs, gs, checks, address
        self.host = Run(hs, checks, {})
        self.guest = Run(gs, checks, BINDS)
        self.m = {}

    def effects(self):
        self.host.call('command', 'effect give VapeGuest minecraft:resistance 100000 4 true')
        self.host.call('command', 'effect give @a minecraft:saturation 100000 4 true')

    def mode(self, creative):
        self.host.call('command', f'gamemode {"creative" if creative else "survival"} VapeGuest')
        self.guest.wait(6)

    def place(self, guest_looks, host_looks):
        """Guest at GUEST, host at HOST, both still, each looking at the
        other or straight away."""
        for r in (self.host, self.guest):
            r.release()
        gy, hy = GUEST_YAW[guest_looks], HOST_YAW[host_looks]
        self.host.call('command', f'tp VapeGuest {GUEST[0]} {FLOOR + 1} {GUEST[1]} {gy} 0')
        self.host.call('tp', HOST[0], FLOOR + 1, HOST[1])
        self.host.call('look', hy, '0')
        self.guest.wait(20)
        self.guest.call('look', gy, '0')
        self.guest.wait(12)

    def displacement(self, ticks=30, step=2):
        start = self.guest.pos()
        best, hurt = 0.0, False
        for _ in range(0, ticks, step):
            self.guest.wait(step)
            p = self.guest.player()
            hurt |= p.get('hurt_time', 0) > 0
            best = max(best, math.hypot(p['position'][0] - start[0], p['position'][2] - start[2]))
        return round(best, 3), hurt

    def knock(self, label, guest_looks, host_looks):
        self.place(guest_looks, host_looks)
        self.host.call('command', f'damage VapeGuest 1 minecraft:mob_attack at {GUEST[0] + 3} {FLOOR + 1} {GUEST[1]}')
        moved, hurt = self.displacement()
        self.m[label] = {'path': 'motion', 'moved': moved, 'hurt': hurt,
                         'guest_looks': guest_looks, 'host_looks': host_looks}
        return moved, hurt

    def blast(self, label, guest_looks, host_looks):
        self.place(guest_looks, host_looks)
        # The default 80-tick fuse (this server ignores a summoned fuse tag).
        self.host.call('command', f'summon minecraft:tnt {GUEST[0] - 3} {FLOOR + 1} {GUEST[1]}')
        self.guest.wait(66)
        moved, _ = self.displacement(40)
        tnt = self.host.call('entities', '16', 'tnt').get('entities', [])
        self.m[label] = {'path': 'explosion', 'moved': moved, 'tnt_left': len(tnt),
                         'guest_looks': guest_looks, 'host_looks': host_looks}
        return moved, not tnt

    def rejoin(self, changes):
        gs = self.gs
        gs.call('quit')
        gs.process.wait(timeout=15)
        gs.log.close()
        gs.process = None
        document = gs.saved()
        document['settings'].update(changes)
        gs.seed(document)
        gs.launch(creative=False, world=None, extra=('--server', self.address, '--name', 'VapeGuest'))
        gs.call('wait-ready', '60')
        self.guest.wait(20)
        self.effects()
        saved = gs.saved()['settings']
        self.checks.check(f'guest rejoined with {sorted(changes)}',
                          all(saved.get(k) == v for k, v in changes.items())
                          and self.guest.enabled('combat.velocity'), {k: saved.get(k) for k in changes})


def run(resources, allow_stale):
    with NativeSession(resources, 'velocity-lan-host-', allow_stale=allow_stale) as hs, \
            NativeSession(resources, 'velocity-lan-guest-', allow_stale=allow_stale) as gs:
        checks = Checks(gs.root)
        hs.seed({'schema': 1, 'modules': {}, 'settings': {}})
        hs.launch(creative=False, world='VelocityHost')
        host = Run(hs, checks, {})
        host.call('time', '6000')
        host.call('tp', HOST[0], FLOOR + 1, HOST[1])
        host.call('wait-ready', '40')
        for _ in range(5):
            host.call('fill', '-12', str(FLOOR - 3), '-8', '12', str(FLOOR), '12', 'minecraft:obsidian')
            host.call('fill', '-12', str(FLOOR + 1), '-8', '12', str(FLOOR + 7), '12', 'minecraft:air')
            floor = host.call('blocks', '-12', str(FLOOR), '-8', '12', str(FLOOR), '12').get('counts', {})
            if floor.get('minecraft:obsidian') == 25 * 21:
                break
            host.wait(20)
        # Shades the host from the TNT at x -2.5: the lines to it cross z 2 at x -0.9 to -0.1.
        host.call('fill', '-4', str(FLOOR + 1), '2', '0', str(FLOOR + 3), '2', 'minecraft:obsidian')
        host.call('command', 'gamerule mob_spawning false')
        host.call('command', 'difficulty peaceful')
        host.call('command', 'effect give @s minecraft:resistance 100000 4 true')
        checks.check('obsidian floor is in place', floor.get('minecraft:obsidian') == 25 * 21, floor)
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        address = f'localhost:{port}'

        gs.seed({'schema': 1, 'modules': {},
                 'settings': {**{f'{k}.binds': [v] for k, v in BINDS.items()}, **FULL}})
        gs.launch(creative=False, world=None, extra=('--server', address, '--name', 'VapeGuest'))
        gs.call('wait-ready', '60')
        p = Pair(hs, gs, checks, address)
        p.guest.wait(20)
        p.effects()
        p.place(False, False)
        others = gs.call('entities', '12', 'player').get('entities', [])
        checks.check('guest joined and sees the host player', bool(others), p.guest.pos())

        # --- Defaults: off pushes, on cancels ----------------------------
        base, hurt = p.knock('motion_off', False, False)
        checks.check('motion off: the hurt pushes the guest', hurt and base >= 0.8, p.m['motion_off'])
        p.guest.toggle('combat.velocity', True)
        on, hurt = p.knock('motion_on', False, False)
        checks.check('motion on: Velocity cancels the push', hurt and on <= 0.25 * base, [on, base])
        p.mode(True)
        p.guest.toggle('combat.velocity', False)
        blast, done = p.blast('blast_off', False, False)
        checks.check('explosion off: the Explode packet pushes the creative guest', done and blast >= 0.8,
                     p.m['blast_off'])
        p.guest.toggle('combat.velocity', True)
        cut, done = p.blast('blast_on', False, False)
        checks.check('explosion on: Velocity cancels the Explode push', done and cut <= 0.25 * blast, [cut, blast])
        p.mode(False)

        # --- Only when targeting ------------------------------------------
        p.rejoin({'combat.velocity.only-targeting': True})
        kept, hurt = p.knock('targeting_motion_neither', False, False)
        checks.check('only-targeting motion: kept when neither player looks at the other',
                     hurt and kept >= 0.7 * base, [kept, base])
        mine, hurt = p.knock('targeting_motion_guest_looks', True, False)
        checks.check('only-targeting motion: cancelled while the guest looks at the host',
                     hurt and mine <= 0.25 * base, [mine, base])
        theirs, hurt = p.knock('targeting_motion_host_looks', False, True)
        checks.check('only-targeting motion: cancelled while the host looks at the guest',
                     hurt and theirs <= 0.25 * base, [theirs, base])
        p.mode(True)
        quirk, done = p.blast('targeting_blast_host_looks', False, True)
        checks.check('only-targeting explosion: kept when only the host looks (applyBridgeVelocity)',
                     done and quirk >= 0.7 * blast, [quirk, blast])
        neither, done = p.blast('targeting_blast_neither', False, False)
        checks.check('only-targeting explosion: cancelled when neither looks',
                     done and neither <= 0.25 * blast, [neither, blast])
        p.mode(False)

        # --- Kite -----------------------------------------------------------
        p.rejoin({'combat.velocity.only-targeting': False, 'combat.velocity.kite': True,
                  'combat.velocity.kite-horizontal': 200.0, 'combat.velocity.kite-vertical': 100.0})
        kite, hurt = p.knock('kite_motion_host_looks', False, True)
        checks.check('Kite motion: doubled while the host looks and the guest looks away',
                     hurt and kite >= 1.4 * base, [kite, base])
        facing, hurt = p.knock('kite_motion_facing', True, True)
        checks.check('Kite motion: cancelled while both face each other', hurt and facing <= 0.25 * base,
                     [facing, base])
        p.mode(True)
        kite_blast, done = p.blast('kite_blast_host_looks', False, True)
        checks.check('Kite explosion: doubled while the host looks and the guest looks away',
                     done and kite_blast >= 1.4 * blast, [kite_blast, blast])
        p.mode(False)

        p.rejoin({'combat.velocity.always-kite': True})
        always, hurt = p.knock('always_kite_motion_facing', True, True)
        checks.check('Always Kite motion: doubled even while both face each other',
                     hurt and always >= 1.4 * base, [always, base])

        p.guest.toggle('combat.velocity', False)
        gs.capture('final')
        alive = gs.call('state')
        checks.check('guest still connected (no kick)', alive.get('player') is not None, alive.get('screen'))
        for name, session in (('host', hs), ('guest', gs)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
        (gs.root / 'measurements.json').write_text(json.dumps(p.m, indent=2, default=str))
        gs.call('disconnect')
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}', gs.root)
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
