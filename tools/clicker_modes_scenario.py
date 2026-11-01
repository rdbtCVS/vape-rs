#!/usr/bin/env python3
"""AutoClicker randomization modes in a fresh isolated survival world on the
integrated server.

Each mode (Normal, Extra, Extra+) runs in its own relaunch of the same home
with a fixed CPS, holding attack on a NoAI, Invulnerable cow (held by a wall)
while the server's attack log records the game tick of every attack. Normal
clicks at 1000/CPS with no pauses, so at 10 CPS its gaps are two ticks. Extra
adds the reference's 40-84 ms pauses to about one click in ten outside bursts
and a short 25/50 ms-slower phase, so it shows many more gaps of three ticks
or longer and a lower rate. Extra+ (the reference `AutoClickerTimingState`)
scales the CPS by 1.5 and adds log-normal noise, 0-40 ms micro jitter and
fatigue pauses, so its rate sits above Normal with one-tick gaps Normal never
has. The server keeps only 64 attacks, so the log is drained every two
seconds of the hold. Results go to result.json and measurements.json.

With --module right-clicker the same modes run RightClicker (the same click
engine on the use button) in creative, throwing snowballs straight up while
use is held: creative keeps the stack, every throw swings the arm, and the
client's swing counter sampled every tick gives the tick of each click.
"""
import argparse
import collections
import json
import os
import time

from combat_scenario import FLOOR, Combat
from movement_scenario import relaunch
from native_session import NativeSession
from smoke_scenario import Checks

MODULES = {'auto-clicker': ('combat.auto-clicker', 'KeyV'), 'right-clicker': ('combat.right-clicker', 'KeyB')}
MODULE, BIND = MODULES['auto-clicker']
BINDS = {MODULE: BIND}
HOLD_S = 30.0
DRAIN_S = 2.0


def settings(mode, cps):
    return {f'{MODULE}.randomization': mode, f'{MODULE}.cps': [float(cps), float(cps)]}


def target(r, kind, x, z):
    """A NoAI, Invulnerable mob: never hurt, pushed or killed (a cow with
    Resistance V still died to held clicks here), while the server still
    logs every attack on it."""
    r.kill(kind)
    r.call('chat', f'/summon minecraft:{kind} {x} {FLOOR + 1} {z} {{NoAI:1b,Invulnerable:1b}}')
    r.wait(8)
    found = r.call('entities', '12', kind).get('entities', [])
    return found[0] if found else None


def measure(r, mode, cps):
    r.reset()
    r.fill(-2, FLOOR + 1, 3, 3, FLOOR + 2, 3, 'minecraft:stone')
    cow = target(r, 'cow', 0.5, 2.5)
    r.call('look', '0', '20')
    r.wait(6)
    r.toggle('combat.auto-clicker', True)
    r.attacks()
    g0, w0 = r.call('simulation').get('game_time'), time.monotonic()
    logged, drains = [], []
    s0 = r.swings()
    r.call('mouse', 'left', 'down')
    while time.monotonic() - w0 < HOLD_S:
        time.sleep(DRAIN_S)
        batch = r.attacks()
        drains.append({'s': round(time.monotonic() - w0, 2), 'n': len(batch), 'swings': r.swings() - s0,
                       'last_t': max((a['t'] for a in batch), default=None)})
        logged += batch
    r.call('mouse', 'left', 'up')
    g1, w1 = r.call('simulation').get('game_time'), time.monotonic()
    r.wait(4)
    logged += r.attacks()
    hits = sorted(a['t'] for a in logged if cow and a.get('id') == cow.get('id'))
    mine = [a for a in logged if cow and a.get('id') == cow.get('id')]
    near = r.call('mobs', '16').get('near', [])
    after = {'cow': next((m for m in near if m.get('kind') == 'minecraft:cow'), None),
             'dealt': sum(a.get('dealt', 0) for a in mine), 'killed': any(a.get('killed') for a in mine),
             'landed': sum(1 for a in mine if a.get('landed')),
             'looking_at': (r.call('state').get('looking_at') or {}), 'player': r.pos()}
    r.toggle('combat.auto-clicker', False)
    r.kill('cow')
    gaps = [b - a for a, b in zip(hits, hits[1:])]
    span = (hits[-1] - hits[0]) / 20 if len(hits) > 1 else 0
    # The server's tick rate over the hold: a lagging server bunches packets.
    tick_rate = (g1 - g0) / (w1 - w0) if g0 is not None and g1 is not None else None
    interval = 20 / cps
    out = {
        'mode': mode, 'cps_setting': cps, 'cow': cow, 'attacks': len(hits),
        'server_cps': (len(gaps) / span) if span else 0,
        'gaps': dict(sorted(collections.Counter(gaps).items())),
        'mean_gap_ticks': sum(gaps) / len(gaps) if gaps else None,
        'interval_ticks': interval,
        # Gaps over a tick past the click interval: pauses and slow clicks.
        'long_gaps': sum(1 for g in gaps if g > interval + 0.5),
        'short_gaps': sum(1 for g in gaps if g < interval - 0.5),
        'spikes': sum(1 for g in gaps if g > interval + 1.5),
        'max_gap': max(gaps) if gaps else None,
        'server_tps': tick_rate,
        'drains': drains, 'hit_ticks': hits, 'game_time': [g0, g1], 'after': after,
    }
    r.m.setdefault('modes', []).append(out)
    return out


def measure_right(r, mode, cps):
    """RightClicker held for HOLD_S: the client tick of every throw."""
    r.reset()
    r.call('look', '0', '-80')
    r.wait(6)
    held0 = (r.call('state').get('held') or {}).get('count')
    r.toggle(MODULE, True)
    samples = []
    r.call('mouse', 'right', 'down')
    w0 = time.monotonic()
    while time.monotonic() - w0 < HOLD_S:
        s = r.call('state')
        samples.append((s.get('ticks'), s.get('swings', 0)))
    r.call('mouse', 'right', 'up')
    w1 = time.monotonic()
    r.wait(4)
    held1 = (r.call('state').get('held') or {}).get('count')
    r.toggle(MODULE, False)
    clicks, unsampled = [], 0
    for (t0, s0), (t1, s1) in zip(samples, samples[1:]):
        if t1 - t0 > 1 and s1 > s0:
            unsampled += 1
        clicks += [t1] * max(s1 - s0, 0)
    gaps = [b - a for a, b in zip(clicks, clicks[1:])]
    span = (clicks[-1] - clicks[0]) / 20 if len(clicks) > 1 else 0
    ticks = samples[-1][0] - samples[0][0] if samples else 0
    interval = 20 / cps
    out = {
        'mode': mode, 'cps_setting': cps, 'attacks': len(clicks),
        'server_cps': (len(gaps) / span) if span else 0,
        'gaps': dict(sorted(collections.Counter(gaps).items())),
        'mean_gap_ticks': sum(gaps) / len(gaps) if gaps else None,
        'interval_ticks': interval,
        'long_gaps': sum(1 for g in gaps if g > interval + 0.5),
        'short_gaps': sum(1 for g in gaps if g < interval - 0.5),
        'spikes': sum(1 for g in gaps if g > interval + 1.5),
        'max_gap': max(gaps) if gaps else None,
        'server_tps': ticks / (w1 - w0) if ticks else None,
        'samples': len(samples), 'unsampled_clicks': unsampled, 'held_counts': [held0, held1],
        'hit_ticks': clicks,
    }
    r.m.setdefault('modes', []).append(out)
    return out


def run(resources, allow_stale, cps_values, modes=('Normal', 'Extra', 'Extra+')):
    home = 'right-clicker-modes-' if MODULE == MODULES['right-clicker'][0] else 'clicker-modes-'
    with NativeSession(resources, home, allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        plan = [(m, c) for c in cps_values for m in modes]
        session.seed({'schema': 1, 'modules': {},
                      'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()}, **settings(*plan[0])}})
        session.launch(creative=True)
        right = MODULE == MODULES['right-clicker'][0]
        if not right:
            session.call('chat', '/gamemode survival')
        session.call('chat', '/difficulty peaceful')
        session.call('time', '6000')
        r = Combat(session, checks, BINDS)
        r.wait(20)
        r.arena()
        name = 'RightClicker' if right else 'AutoClicker'
        results = {}
        for index, (mode, cps) in enumerate(plan):
            if index:
                relaunch(session, settings(mode, cps))
                if right:
                    session.call('chat', '/gamemode creative')
                r.wait(10)
            saved = session.saved()['settings']
            checks.check(f'{name} {mode} at {cps} CPS is the saved randomization',
                         saved.get(f'{MODULE}.randomization') == mode, saved.get(f'{MODULE}.randomization'))
            r.call('chat', '/effect give @s minecraft:resistance 100000 4 true')
            if right:
                if not r.give('snowball', 16):
                    continue
                results[(mode, cps)] = measure_right(r, mode, cps)
                got = results[(mode, cps)]
                checks.check(f'{name} {mode} at {cps} CPS: creative keeps the snowballs while it throws',
                             got['held_counts'][0] == got['held_counts'][1] and got['attacks'] > 0,
                             got['held_counts'])
                checks.check(f'{name} {mode} at {cps} CPS: the swing counter is sampled every tick',
                             got['unsampled_clicks'] <= 0.05 * max(got['attacks'], 1),
                             [got['unsampled_clicks'], got['samples']])
            else:
                results[(mode, cps)] = measure(r, mode, cps)
            print(json.dumps({k: v for k, v in results[(mode, cps)].items() if k not in ('hit_ticks', 'cow')}), flush=True)
        for cps in cps_values if len(modes) == 3 and all((m, cps) in results for m in modes) else ():
            normal, extra, plus = (results[(m, cps)] for m in modes)
            prefix = f'{name} ' if right else ''
            checks.check(f'{prefix}Normal {cps} CPS: the {"client throws" if right else "server takes attacks"} near the setting',
                         0.8 * cps <= normal['server_cps'] <= 1.25 * cps, normal['server_cps'])
            gaps = max(normal['attacks'] - 1, 1)
            checks.check(f'{prefix}Normal {cps} CPS: fixed delay, under 10% of gaps a tick late and at most one later',
                         normal['long_gaps'] <= 0.1 * gaps and normal['spikes'] <= 1, normal['gaps'])
            checks.check(f'{prefix}Extra {cps} CPS: pauses lengthen at least 8 more gaps than Normal',
                         extra['long_gaps'] >= normal['long_gaps'] + 8, [extra['gaps'], normal['gaps']])
            checks.check(f'{prefix}Extra {cps} CPS: pauses lower the rate at least 1.5% below Normal',
                         extra['server_cps'] <= 0.985 * normal['server_cps'],
                         [extra['server_cps'], normal['server_cps']])
            checks.check(f'{prefix}Extra+ {cps} CPS: the 1.5x-scaled rate stays above Normal despite jitter',
                         plus['server_cps'] >= 1.05 * normal['server_cps'], [plus['server_cps'], normal['server_cps']])
            checks.check(f'{prefix}Extra+ {cps} CPS: varied gaps, with under-interval clicks Normal never makes',
                         plus['short_gaps'] >= 0.1 * max(plus['attacks'] - 1, 1) and len(plus['gaps']) >= 3
                         and normal['short_gaps'] <= 0.05 * gaps, [plus['gaps'], normal['gaps']])
        session.capture('final')
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
    parser.add_argument('--cps', type=int, nargs='+', default=[10])
    parser.add_argument('--modes', nargs='+', default=['Normal', 'Extra', 'Extra+'])
    parser.add_argument('--module', choices=sorted(MODULES), default='auto-clicker')
    args = parser.parse_args()
    global MODULE, BIND, BINDS
    MODULE, BIND = MODULES[args.module]
    BINDS = {MODULE: BIND}
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run(args.resources, args.allow_stale, args.cps, args.modes) else 1)


if __name__ == '__main__':
    main()
