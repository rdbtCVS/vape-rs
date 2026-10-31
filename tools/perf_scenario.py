#!/usr/bin/env python3
"""Per-module frame cost in a fresh isolated survival world on the
integrated server.

In front of 24 cows, 8 chests, an item and a held snowball, the host's own
frame statistics (ctl bench) are taken with no module on, then with each
render and HUD module on alone, then with all of them on, and with none on
again. The plain bench runs at the start and the end so the run's own noise
is visible next to each module's cost.

The host keeps vanilla's AFK frame limiter, which drops the window to
30 fps a minute after the last player input. The scratch options.txt turns
it off ("minimized" only), as the host's own benchmark mode does, so every
bench runs at the same frame cap.

Checks: every module alone adds under 1 ms of average CPU per frame, all of
them together under 3 ms, and none leaves a cost behind once off. The
frame rate and the tail (p99/max) are recorded; they follow the
compositor's pacing of the window, which the scenario does not control.

Results go to result.json and measurements.json.
"""
import argparse
import json
import os

from combat_scenario import Combat
from delays_perf_scenario import BINDS, FLOOR, PERF, SETTINGS, bench
from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

KEEP = ('frames', 'avg_fps', 'p50_ms', 'p99_ms', 'max_ms')
ALONE_MS = 1.0
ALL_MS = 3.0


def summary(b):
    cpu = b.get('cpu', {})
    return {**{k: b.get(k) for k in KEEP},
            'cpu_avg_ms': cpu.get('avg_ms'), 'cpu_p95_ms': cpu.get('p95_ms'),
            'cpu_p99_ms': cpu.get('p99_ms'), 'cpu_max_ms': cpu.get('max_ms'),
            'gpu_avg_ms': b.get('gpu', {}).get('avg_ms')}


def scene(r):
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


def measure(r, seconds):
    benches = {'off': summary(bench(r, seconds))}
    for module in PERF:
        r.toggle(module, True)
        r.wait(20)
        benches[module] = summary(bench(r, seconds))
        r.toggle(module, False)
        r.wait(10)
    for module in PERF:
        r.toggle(module, True)
    r.wait(20)
    benches['all'] = summary(bench(r, seconds))
    for module in PERF:
        r.toggle(module, False)
    r.wait(20)
    benches['after'] = summary(bench(r, seconds))
    r.m['benches'] = benches
    base = min(benches['off']['cpu_avg_ms'] or 0, benches['after']['cpu_avg_ms'] or 0)
    noise = abs((benches['off']['cpu_avg_ms'] or 0) - (benches['after']['cpu_avg_ms'] or 0))
    r.m['baseline_cpu_avg_ms'] = base
    r.m['baseline_noise_ms'] = noise
    r.checks.check('frames were recorded in every bench', all((b['frames'] or 0) > 50 for b in benches.values()),
                   {k: b['frames'] for k, b in benches.items()})
    for module in PERF:
        cost = (benches[module]['cpu_avg_ms'] or 0) - base
        r.checks.check(f'{module} alone adds under {ALONE_MS} ms of CPU per frame', cost < ALONE_MS,
                       {'added_ms': round(cost, 3), 'cpu_p99_ms': benches[module]['cpu_p99_ms']})
    cost = (benches['all']['cpu_avg_ms'] or 0) - base
    r.checks.check(f'all {len(PERF)} modules add under {ALL_MS} ms of CPU per frame', cost < ALL_MS,
                   {'added_ms': round(cost, 3), 'cpu_p99_ms': benches['all']['cpu_p99_ms']})
    fps = [b['avg_fps'] or 0 for b in benches.values()]
    r.checks.check('every bench ran at the same frame cap (no idle limiter)', min(fps) > 0.8 * max(fps),
                   {k: round(b['avg_fps'] or 0, 1) for k, b in benches.items()})
    r.checks.check('no cost is left once every module is off',
                   (benches['after']['cpu_avg_ms'] or 0) <= (benches['off']['cpu_avg_ms'] or 0) + 0.5,
                   [benches['off']['cpu_avg_ms'], benches['after']['cpu_avg_ms']])


def run(resources, allow_stale, seconds):
    with NativeSession(resources, 'perf-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        (session.root / 'game').mkdir(parents=True, exist_ok=True)
        (session.root / 'game' / 'options.txt').write_text('inactivityFpsLimit:"minimized"\n')
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
        try:
            scene(r)
            measure(r, seconds)
        except Exception as error:  # report and go on
            checks.check('the measurement ran to completion', False, repr(error))
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
    parser.add_argument('--seconds', type=int, default=10)
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run(args.resources, args.allow_stale, args.seconds) else 1)


if __name__ == '__main__':
    main()
