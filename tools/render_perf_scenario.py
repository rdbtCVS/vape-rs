#!/usr/bin/env python3
"""The per-module frame cost of delays_perf_scenario's twelve render and HUD
modules, in the same fresh isolated survival world and scene (24 cows,
8 chests, a dropped diamond, snowballs held): the host's frame statistics
(ctl bench) with no module on, with each module alone, with all twelve, and
with none again. Each bench's CPU time per frame and its spread are written
to measurements.json; the check is delays_perf_scenario's (all twelve within
3 ms of plain). `--only` benches some modules alone, and `--repeat` benches
each of them that many times, each after a fresh plain bench, and reports the
medians (a single bench drifts with the machine).
"""
import argparse
import json
import os
import statistics

from combat_scenario import Combat
from delays_perf_scenario import BINDS, FLOOR, PERF, SETTINGS, bench
from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

KEEP = ('frames', 'avg_fps', 'low_1pct_fps', 'p50_ms', 'p95_ms', 'p99_ms', 'max_ms', 'cpu',
        'phases_avg_ms', 'slow_phases_avg_ms')


def summary(b):
    return {k: b.get(k) for k in KEEP}


def cpu(b, k='avg_ms'):
    return (b.get('cpu') or {}).get(k) or 0


def run(resources, allow_stale, seconds, only=(), repeat=1):
    modules = [m for m in PERF if not only or m in only]
    with NativeSession(resources, 'render-perf-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()}, **SETTINGS}})
        session.launch(creative=True)
        session.call('chat', '/gamemode survival')
        session.call('chat', '/difficulty peaceful')
        session.call('time', '6000')
        session.call('chat', '/gamerule advance_time false')
        r = Combat(session, checks, BINDS)
        r.wait(20)
        Run.arena(r)
        r.call('chat', '/effect give @s minecraft:resistance 100000 4 true')
        r.call('chat', '/effect give @s minecraft:saturation 100000 4 true')
        r.reset()
        for i in range(24):
            r.call('chat', f'/summon minecraft:cow {-4 + (i % 6) * 1.5} {FLOOR + 1} {6 + (i // 6) * 1.5} {{NoAI:1b}}')
        for i in range(8):
            r.call('chat', f'/setblock {-4 + i} {FLOOR + 1} 14 minecraft:chest')
        r.call('chat', '/summon minecraft:item 0.5 150 4.5 {Item:{id:"minecraft:diamond",count:1},PickupDelay:32767}')
        r.give('snowball', 16)
        r.wait(40)
        r.call('look', '0', '10')
        r.wait(20)
        m = {'seconds': seconds, 'repeat': repeat, 'plain': summary(bench(r, seconds)), 'alone': {}}
        samples = {'plain': [cpu(m['plain'])]}
        for i in range(repeat):
            for module in modules:
                if i or module != modules[0]:
                    samples['plain'].append(cpu(bench(r, seconds)))
                r.toggle(module, True)
                r.wait(20)
                m['alone'][module] = summary(bench(r, seconds))
                samples.setdefault(module, []).append(cpu(m['alone'][module]))
                r.toggle(module, False)
                r.wait(10)
        for module in PERF:
            r.toggle(module, True)
        r.wait(20)
        m['all'] = summary(bench(r, seconds))
        for module in PERF:
            r.toggle(module, False)
        r.wait(20)
        m['after'] = summary(bench(r, seconds))
        benches = {'plain': m['plain'], 'all': m['all'], 'after': m['after'], **m['alone']}
        m['cpu_avg_ms'] = {k: cpu(v) for k, v in benches.items()}
        m['cpu_p99_ms'] = {k: cpu(v, 'p99_ms') for k, v in benches.items()}
        m['cpu_samples_ms'] = samples
        m['cpu_median_ms'] = {k: statistics.median(v) for k, v in samples.items()}
        checks.check('performance: twelve render/HUD modules add under 3 ms of CPU per frame',
                     cpu(m['all']) <= cpu(m['plain']) + 3.0 and cpu(m['all']) <= cpu(m['after']) + 3.0,
                     m['cpu_avg_ms'])
        log = (session.root / 'game.log').read_text(errors='replace')
        checks.check('no panics in the log', 'panicked' not in log and 'poisoned' not in log)
        (session.root / 'measurements.json').write_text(json.dumps(m, indent=2, default=str))
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}',
              session.root)
        for k, v in m['cpu_avg_ms'].items():
            phases = benches[k].get('phases_avg_ms') or {}
            print(f'{k:24} {v:7.3f} ms  p99 {m["cpu_p99_ms"][k]:7.3f} ms  phases {phases}')
        plain = m['cpu_median_ms']['plain']
        for k, v in m['cpu_median_ms'].items():
            print(f'median {k:24} {v:7.3f} ms  ({v - plain:+.3f} over plain, n={len(samples[k])})')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?', default=os.environ.get('HORIZON_RESOURCES'))
    parser.add_argument('--allow-stale', action='store_true')
    parser.add_argument('--seconds', type=int, default=6)
    parser.add_argument('--only', default='', help='comma-separated module ids to bench alone')
    parser.add_argument('--repeat', type=int, default=1)
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    only = [m for m in args.only.split(',') if m]
    unknown = [m for m in only if m not in PERF]
    if unknown:
        parser.error(f'not one of the twelve modules: {unknown}')
    raise SystemExit(0 if run(args.resources, args.allow_stale, args.seconds, only, max(1, args.repeat)) else 1)


if __name__ == '__main__':
    main()
