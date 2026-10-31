#!/usr/bin/env python3
"""Controlled LAN scenario for KeepSprint: a fresh host world opened to LAN on
localhost and a fresh guest client running vape-rs over the real protocol,
each in its own isolated home.

The guest sprints past the host player while KillAura (perfect swing, so each
hit is a full-strength sprint hit) attacks it. A hit on another player runs
the attacker's side of extra knockback on the client (`remote_player_attack`):
horizontal motion * 0.6 and a sprint stop. The guest's velocity is sampled
between ticks: without KeepSprint each landed hit shows the cut as a speed
dip; with KeepSprint at slowdown 1.0 the hits land (the host's hurt log) with
no dip. Results go to <session>/result.json via Checks.
"""
import argparse
import math
import time
from pathlib import Path

from native_session import NativeSession
from smoke_scenario import Checks, enabled

BINDS = {'combat.keep-sprint': 'KeyG', 'combat.kill-aura': 'KeyK'}


def toggle(session, module):
    session.call('key', BINDS[module][3:].lower(), 'tap')
    session.call('wait', '2')


def hurts(host):
    players = host.call('hurtlog').get('players', [])
    return sum(len(p.get('hurts', [])) for p in players if p.get('name') != 'VapeGuest')


def sprint_pass(host, guest, start_x, yaw):
    """One sprinting pass along x; velocity samples between ticks."""
    host.call('tp', '0.5', '100', '2.5')
    host.call('command', f'tp VapeGuest {start_x} 100 0.5 {yaw} 0')
    guest.call('wait', '10')
    guest.call('look', str(yaw), '0')
    guest.call('key', 'ctrl', 'down')
    guest.call('key', 'w', 'down')
    samples = []
    deadline = time.time() + 3.5
    last = None
    while time.time() < deadline:
        p = guest.call('state').get('player') or {}
        v = p.get('velocity') or [0, 0, 0]
        key = (tuple(p.get('position') or ()), tuple(v))
        if key != last:
            samples.append({'speed': math.hypot(v[0], v[2]), 'sprinting': p.get('sprinting'),
                            'x': (p.get('position') or [0])[0]})
            last = key
    guest.call('key', 'w', 'up')
    guest.call('key', 'ctrl', 'up')
    guest.call('wait', '10')
    return samples


def dips(passes):
    """Samples whose horizontal speed fell below 0.85 of the one before (the
    cut gives about 0.78), counted within each pass after its run-up."""
    out = 0
    for samples in passes:
        steady = run_up(samples)
        for a, b in zip(steady, steady[1:]):
            if a['speed'] > 0.1 and b['speed'] < 0.85 * a['speed']:
                out += 1
    return out


def run_up(samples):
    """The pass from its first sample at full sprint speed on."""
    start = next((i for i, s in enumerate(samples) if s['speed'] >= 0.145), len(samples))
    return samples[start:]


def speeds(passes):
    return [[round(s['speed'], 3) for s in samples] for samples in passes]


def run(resources, allow_stale):
    with NativeSession(resources, 'keep-sprint-host-', allow_stale=allow_stale) as host, \
            NativeSession(resources, 'keep-sprint-guest-', allow_stale=allow_stale) as guest:
        checks = Checks(guest.root)
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=False, world='LanHost')
        host.call('time', '6000')
        host.call('tp', '0.5', '100', '0.5')
        host.call('wait-ready', '40')
        host.call('fill', '-16', '96', '-6', '16', '99', '12', 'minecraft:stone')
        host.call('fill', '-16', '100', '-6', '16', '104', '12', 'minecraft:air')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        host.call('command', 'gamerule natural_regeneration false')
        host.call('command', 'effect give @s minecraft:resistance infinite 4 true')

        guest.seed({'schema': 1, 'modules': {},
                    'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()},
                                 'combat.kill-aura.perfect-swing': True,
                                 'combat.kill-aura.max-angle': 180.0,
                                 'combat.kill-aura.targets.players': True,
                                 'combat.keep-sprint.normal-slowdown': 1.0,
                                 'combat.keep-sprint.velocity-slowdown': 1.0}})
        guest.launch(creative=False, world=None,
                     extra=('--server', f'localhost:{port}', '--name', 'VapeGuest'))
        guest.call('wait-ready', '60')
        guest.call('wait', '20')
        joined = (guest.call('state').get('player') or {}).get('health', 0) > 0
        checks.check('guest joined the LAN world', joined)

        toggle(guest, 'combat.kill-aura')
        checks.check('guest KillAura enabled', enabled(guest, 'combat.kill-aura'))
        hurts(host)
        off = [sprint_pass(host, guest, -12.5, -90), sprint_pass(host, guest, 13.5, 90)]
        off_hits = hurts(host)
        checks.check('KillAura lands sprint hits on the host player', off_hits >= 2, off_hits)
        checks.check('without KeepSprint the hits cut the guest\'s speed', dips(off) >= 1,
                     {'dips': dips(off), 'hits': off_hits, 'speeds': speeds(off)})

        toggle(guest, 'combat.keep-sprint')
        checks.check('KeepSprint enabled through its bind', enabled(guest, 'combat.keep-sprint'))
        on = [sprint_pass(host, guest, -12.5, -90), sprint_pass(host, guest, 13.5, 90)]
        on_hits = hurts(host)
        checks.check('hits still land with KeepSprint', on_hits >= 2, on_hits)
        checks.check('KeepSprint at slowdown 1.0: no speed cut on hits', dips(on) == 0,
                     {'dips': dips(on), 'hits': on_hits, 'speeds': speeds(on)})
        checks.check('the guest keeps sprinting through the hits',
                     all(s['sprinting'] for samples in on for s in run_up(samples) if s['speed'] > 0.1),
                     [[s['sprinting'] for s in samples] for samples in on])

        toggle(guest, 'combat.keep-sprint')
        checks.check('KeepSprint disabled through its bind', not enabled(guest, 'combat.keep-sprint'))
        again = [sprint_pass(host, guest, -12.5, -90)]
        again_hits = hurts(host)
        checks.check('after disabling KeepSprint the cut returns',
                     again_hits == 0 or dips(again) >= 1, {'dips': dips(again), 'hits': again_hits})
        for name, session in (('host', host), ('guest', guest)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
        guest.call('disconnect')
        failed = [r['check'] for r in checks.results if not r['ok']]
        print(f'{len(checks.results) - len(failed)}/{len(checks.results)} checks passed', flush=True)
        return failed


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    raise SystemExit(1 if run(args.resources, args.allow_stale) else 0)
