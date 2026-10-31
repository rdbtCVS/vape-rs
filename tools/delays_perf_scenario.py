#!/usr/bin/env python3
"""Input-delay removals and a basic frame-time measurement in a fresh
isolated survival world on the integrated server.

* NoJumpDelay: under a two-block ceiling a held jump bonks and lands within
  a few ticks, then waits out the host's ten-tick post-jump gate; with the
  module the gate is cleared and the player jumps again on landing (more
  jumps in the same ticks). Off again, the gate is back.
* NoClickDelay: a click on air sets the host's ten-tick missed-attack
  cooldown; held for two ticks and released, a click on a cow right after is
  dropped without the module and lands on the server with it.
* Performance: the host's own frame statistics (ctl bench) for 10 s with no
  module on, then with twelve render and HUD modules on in front of 24 cows
  and 8 chests, then off again. The CPU time per frame must stay within
  3 ms of plain; the frame rate follows the compositor and is recorded only.

Results go to result.json and measurements.json.
"""
import argparse
import json
import os
import time

from combat_scenario import Combat
from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 149
BINDS = {
    'legit.no-jump-delay': 'KeyV',
    'legit.no-click-delay': 'KeyK',
    'hud.coordinates': 'KeyP',
    'hud.fps': 'KeyO',
    'hud.keystrokes': 'KeyU',
    'hud.armor': 'KeyG',
    'hud.potions': 'KeyH',
    'render.esp': 'KeyJ',
    'render.tracers': 'KeyN',
    'render.storage-esp': 'KeyM',
    'render.name-tags': 'KeyB',
    'render.item-esp': 'KeyI',
    'render.trajectories': 'KeyY',
    'render.fullbright': 'KeyR',
}
PERF = [m for m in BINDS if not m.startswith('legit.')]
SETTINGS = {'render.name-tags.animals': True}


def jumps(r, ticks=60):
    """Hold jump under the ceiling; count take-offs in `ticks` ticks."""
    r.reset(z=-20)
    r.call('key', 'space', 'down')
    count, last = 0, True
    for _ in range(ticks):
        r.wait(1)
        ground = r.player()['on_ground']
        if last and not ground:
            count += 1
        last = ground
    r.release()
    return count


def no_jump_delay(r):
    # A stone ceiling two blocks above the floor along the jump lane.
    r.fill(-2, FLOOR + 3, -23, 2, FLOOR + 3, -17, 'minecraft:stone')
    r.wait(6)
    off = jumps(r)
    r.toggle('legit.no-jump-delay', True)
    on = jumps(r)
    r.toggle('legit.no-jump-delay', False)
    after = jumps(r)
    r.fill(-2, FLOOR + 3, -23, 2, FLOOR + 3, -17, 'minecraft:air')
    r.m['no_jump_delay'] = {'jumps_60_ticks': [off, on, after]}
    r.checks.check('NoJumpDelay off: under a low ceiling the jump gate spaces jumps ten ticks apart', 3 <= off <= 9, off)
    r.checks.check('NoJumpDelay on: the player jumps again on landing (more jumps)', on >= off * 2, [off, on])
    r.checks.check('NoJumpDelay disabled: the jump gate is back', after <= 9 and on >= after * 2, [off, on, after])


def miss_then_hit(r, cow_id):
    """A click on the sky held two ticks, then a click on the cow: did the
    server log an attack on it?"""
    r.call('look', '0', '-90')
    r.wait(3)
    r.attacks()
    r.call('mouse', 'left', 'down')
    r.wait(2)
    r.call('mouse', 'left', 'up')
    r.call('look', '0', '20')
    r.call('mouse', 'left', 'click')
    r.wait(6)
    hit = [a for a in r.attacks() if a.get('id') == cow_id]
    r.wait(20)
    return bool(hit)


def no_click_delay(r):
    r.reset()
    # A wall behind the cow against the knockback.
    r.fill(-2, FLOOR + 1, 3, 3, FLOOR + 2, 3, 'minecraft:stone')
    cow = r.mob('cow', 0.5, 2.5)
    cid = cow and cow.get('id')
    off = [miss_then_hit(r, cid) for _ in range(3)]
    r.toggle('legit.no-click-delay', True)
    on = [miss_then_hit(r, cid) for _ in range(3)]
    r.toggle('legit.no-click-delay', False)
    after = [miss_then_hit(r, cid) for _ in range(3)]
    r.kill('cow')
    r.fill(-2, FLOOR + 1, 3, 3, FLOOR + 2, 3, 'minecraft:air')
    r.m['no_click_delay'] = {'cow': cow, 'off': off, 'on': on, 'after': after}
    r.checks.check('NoClickDelay off: a click right after a miss is dropped', not any(off), off)
    r.checks.check('NoClickDelay on: the click right after a miss lands on the server', all(on), on)
    r.checks.check('NoClickDelay disabled: the miss cooldown drops the click again', not any(after), after)


def bench(r, seconds=10):
    return r.s.call('bench', str(seconds), timeout=seconds + 30)


def performance(r):
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
    plain = bench(r)
    for module in PERF:
        r.toggle(module, True)
    r.wait(20)
    loaded = bench(r)
    for module in PERF:
        r.toggle(module, False)
    r.wait(20)
    after = bench(r)
    keep = ('frames', 'avg_fps', 'low_1pct_fps', 'p50_ms', 'p95_ms', 'p99_ms', 'max_ms', 'cpu', 'gpu')
    r.m['performance'] = {'modules': PERF, **{k: {x: v.get(x) for x in keep}
                                              for k, v in (('off', plain), ('on', loaded), ('after', after))}}
    r.checks.check('performance: frames were recorded in every bench', all(b.get('frames', 0) > 50 for b in
                                                                          (plain, loaded, after)),
                   [b.get('frames') for b in (plain, loaded, after)])
    # The frame rate depends on the machine's load, which the scenario does
    # not control, so it is recorded; the check is on the frame's own CPU time.
    cpu = [b.get('cpu', {}).get('avg_ms') or 0 for b in (plain, loaded, after)]
    r.checks.check('performance: twelve render/HUD modules add under 3 ms of CPU per frame',
                   cpu[1] <= cpu[0] + 3.0 and cpu[1] <= cpu[2] + 3.0,
                   {'cpu_avg_ms': cpu, 'avg_fps': [b.get('avg_fps') for b in (plain, loaded, after)]})
    r.call('chat', '/kill @e[type=!minecraft:player]')
    r.fill(-4, FLOOR + 1, 14, 4, FLOOR + 1, 14, 'minecraft:air')


def run(resources, allow_stale):
    with NativeSession(resources, 'delays-perf-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        # Off: vanilla's AFK limiter (30 fps a minute after the last input).
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
        for step in (no_jump_delay, no_click_delay, performance):
            try:
                step(r)
            except Exception as error:  # report and go on
                checks.check(f'{step.__name__} ran to completion', False, repr(error))
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
