#!/usr/bin/env python3
"""The worst-case loads of plan section 10 item 7 that perf_scenario.py does
not cover, each in fresh isolated homes:

* disabled: the same pinned host built without vape-rs
  (tools/plain_build.sh) against vape-rs with every module off,
  in perf_scenario's scene (24 cows, 8 chests) and in a dense one (300
  cows), host-only before and after (A-B-A, so the machine's drift shows).
  In the dense scene vape-rs then turns on ESP, Tracers and NameTags
  (animals), which walk every entity each frame.
* planners: a full 36-slot inventory of mixed gear, blocks, food and junk;
  InventoryManager, InvCleaner and AutoHotbar in toggle mode at their
  longest delays, so they plan every tick: while they work and once
  everything is arranged. The planners run in the client tick, so the
  check is on the frame's tick phase (the inventory screen they open has its
  own render cost).
* blink: a LAN guest holds its outgoing packets with Blink (outgoing only,
  breadcrumbs on, no auto send). Like the reference, which holds only what
  the client thread sends, keep-alive answers are not held: Horizon answers
  them on its network reader (horizon-client net.rs), outside Game::send, so
  the host's 30 s keep-alive watch never drops a blinking guest and nothing
  but the player bounds the queue. So: a 10 s hold while the guest walks a
  circle (a packet a tick, a breadcrumb every five) and a bench runs
  alongside, then the frames that send it all on disable, against the same
  window with Blink toggled on and straight off holding nothing; the guest
  must still be connected, the host must have seen it stand still during
  the hold and have it where it walked afterwards. Then the same for a 45 s
  hold, past the keep-alive limit: still connected, and the queue it built
  sent within the same bounds.
* backtrack: the inbound hold queue at its limit. A LAN guest with BackTrack
  at its longest latency (500 ms) attacks the host, which then paces away
  and back beside a fenced pen of 300 cows pushing each other (cramming off),
  so every packet the guest receives while the host steps away is held, up
  to the queue's 1024-packet limit (packets.rs; the reference's queue has
  none), replayed at once past it and on every turn back. Against the same
  paced scene with BackTrack off: the CPU per frame, the slow frames' tick
  (where packets are handled; a replay of the whole queue must fit one 60 Hz
  frame), and the guest's view of the host trailing it while held. Then the
  cows and their drops are removed and the same pacing must show the host
  500 ms behind (about 2 blocks of walking, not more: the pen's bursts once
  pushed the release times ahead of the clock).

Each bench is the host's own frame statistics (ctl bench) with vanilla's
AFK limiter off. Results go to result.json and measurements.json in each
session's home.
"""
import argparse
import json
import os
import subprocess
import threading
import time

from combat_scenario import Combat
from delays_perf_scenario import FLOOR
from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

IM = 'inventory.inventory-manager'
IC = 'inventory.inv-cleaner'
AH = 'inventory.auto-hotbar'
BLINK = 'blatant.blink'
BACKTRACK = 'blatant.backtrack'
ENTITY = ['render.esp', 'render.tracers', 'render.name-tags']
BINDS = {'render.esp': 'KeyJ', 'render.tracers': 'KeyN', 'render.name-tags': 'KeyB',
         IM: 'KeyI', IC: 'KeyK', AH: 'KeyO', BLINK: 'KeyM', BACKTRACK: 'KeyG'}
DENSE = 300
# Runtime with everything off against the host without the mod.
OFF_MS = 0.5
ENTITY_MS = 3.0
PLANNER_TICK_MS = 1.0
BLINK_MS = 3.0
FLUSH_MS = 50.0
FLUSH_TICK_MS = 5.0
HOLD_S = 10
PEN_COWS = 300
BACKTRACK_MS = 3.0
# One 60 Hz frame: a replay of the whole inbound queue must fit in it.
REPLAY_TICK_MS = 16.7
LONG_HOLD_S = 45

PROFILE = {
    'name': 'Load',
    'armor_mode': 'Best armor',
    'slots': [{'slot': 0, 'itemFilter': 'swords', 'priority': 'weapon_damage'},
              {'slot': 1, 'itemFilter': 'minecraft:golden_apple'}],
    'inventoryFilters': [{'itemFilter': 'minecraft:rotten_flesh', 'filterAction': 'Drop item'},
                         {'itemFilter': 'minecraft:dirt', 'filterAction': 'Move into inventory'},
                         {'itemFilter': 'minecraft:cobblestone', 'filterAction': 'Condense stacks'}],
}
HOTBAR = [{'slot': 0, 'item': 'minecraft:diamond_sword', 'anyMaterial': True, 'onlyBest': True},
          {'slot': 1, 'item': 'minecraft:golden_apple'},
          {'slot': 2, 'item': 'minecraft:iron_pickaxe', 'anyMaterial': True, 'onlyBest': True},
          {'slot': 3, 'item': 'minecraft:bow', 'onlyBest': True},
          {'slot': 8, 'item': 'minecraft:cobblestone'}]
FULL = ['wooden_sword', 'stone_sword', 'iron_sword', 'golden_apple 3', 'cobblestone 17', 'dirt 20',
        'rotten_flesh 5', 'bow', 'iron_pickaxe',
        'diamond_sword', 'golden_sword', 'stone_pickaxe', 'wooden_pickaxe', 'leather_helmet',
        'iron_helmet', 'chainmail_chestplate', 'iron_chestplate', 'leather_leggings', 'iron_leggings',
        'golden_boots', 'iron_boots', 'cobblestone 30', 'cobblestone 9', 'dirt 40', 'rotten_flesh 12',
        'bread 6', 'cooked_beef 8', 'arrow 32', 'golden_carrot 5', 'stone 64', 'oak_planks 33',
        'sand 21', 'gravel 7', 'flint 3', 'stick 11', 'cobblestone 41']
SETTINGS = {
    **{f'{m}.binds': [k] for m, k in BINDS.items()},
    'render.name-tags.animals': True,
    f'{IM}.trigger': 'toggle', f'{IM}.click-delay': [300, 300],
    f'{IM}.profiles': json.dumps([PROFILE]), f'{IM}.profile': 'Load',
    f'{IC}.trigger': 'toggle', f'{IC}.delay': [200, 200], f'{IC}.inventory-only': False,
    f'{AH}.trigger': 'toggle', f'{AH}.delay': 300, f'{AH}.hotbar': json.dumps(HOTBAR),
    f'{BLINK}.breadcrumbs': True,
}


def bench(r, seconds):
    b = r.s.call('bench', str(seconds), timeout=seconds + 30)
    cpu = b.get('cpu') or {}
    return {'frames': b.get('frames'), 'avg_fps': b.get('avg_fps'), 'p99_ms': b.get('p99_ms'),
            'cpu_avg_ms': cpu.get('avg_ms'), 'cpu_p99_ms': cpu.get('p99_ms'), 'cpu_max_ms': cpu.get('max_ms'),
            'phases_avg_ms': b.get('phases_avg_ms'), 'load': os.getloadavg()[0]}


def cpu(b):
    return b.get('cpu_avg_ms') or 0


def tick(b):
    return (b.get('phases_avg_ms') or {}).get('tick') or 0


def switch(r, modules, want):
    """Bind taps on the playing screen (module binds fire nowhere else). A
    planner with work opens the inventory on its first tick, so enabling runs
    on an empty inventory and disabling closes any open screen before each tap."""
    for module in modules:
        if r.enabled(module) != want:
            if r.call('state').get('screen') != 'playing':
                r.call('key', 'escape', 'tap')
                r.wait(1)
            r.call('key', r.binds[module][3:].lower(), 'tap')
            r.wait(2)
    states = {m: r.enabled(m) for m in modules}
    r.checks.check(f'{", ".join(modules)} {"enabled" if want else "disabled"} through their binds',
                   all(v == want for v in states.values()), states)


def nothing_on(r):
    on = [m for m, v in r.s.saved().get('modules', {}).items() if v.get('enabled')]
    r.checks.check('every module is off', not on, on)


def world(session, checks, plain=False):
    (session.root / 'game').mkdir(parents=True, exist_ok=True)
    (session.root / 'game' / 'options.txt').write_text('inactivityFpsLimit:"minimized"\n')
    if not plain:
        session.seed({'schema': 1, 'modules': {IC: {'visible': True}, AH: {'visible': True}}, 'settings': SETTINGS})
    session.launch(creative=True)
    for line in ('/gamemode survival', '/difficulty peaceful', '/gamerule advance_time false'):
        session.call('chat', line)
    session.call('time', '6000')
    r = Combat(session, checks, BINDS)
    r.wait(20)
    Run.arena(r)
    r.call('chat', '/effect give @s minecraft:resistance 100000 4 true')
    r.call('chat', '/effect give @s minecraft:saturation 100000 4 true')
    r.reset()
    return r


def scene(r, cows):
    """perf_scenario's scene with `cows` still cows: the first 24 as there,
    the rest in a grid behind them."""
    r.call('chat', '/kill @e[type=!minecraft:player]')
    for i in range(cows):
        if i < 24:
            x, z = -4 + (i % 6) * 1.5, 6 + (i // 6) * 1.5
        else:
            j = i - 24
            x, z = -3.5 + (j % 12), 15.5 + (j // 12) * 0.6
        r.call('chat', f'/summon minecraft:cow {x} {FLOOR + 1} {z} {{NoAI:1b}}')
    for i in range(8):
        r.call('chat', f'/setblock {-4 + i} {FLOOR + 1} 14 minecraft:chest')
    r.call('chat', '/summon minecraft:item 0.5 150 4.5 {Item:{id:"minecraft:diamond",count:1},PickupDelay:32767}')
    r.wait(40)
    found = len(r.call('entities', '64', 'cow').get('entities', []))
    r.reset()
    r.call('look', '0', '10')
    r.wait(20)
    return found


def host_only(resources, seconds, label, out):
    with NativeSession(resources, f'perf-plain-{label}-', plain=True) as session:
        checks = Checks(session.root)
        r = world(session, checks, plain=True)
        found = {}
        for name, cows in (('small', 24), ('dense', DENSE)):
            found[name] = scene(r, cows)
            out[f'plain-{label}-{name}'] = bench(r, seconds)
        out[f'plain-{label}-cows'] = found
        checks.check('the host without the mod ran both scenes', found == {'small': 24, 'dense': DENSE}, found)
        (session.root / 'measurements.json').write_text(json.dumps(out, indent=2))
        return all(c['ok'] for c in checks.results), str(session.root)


def disabled(resources, allow_stale, seconds):
    out = {}
    roots = {}
    ok_a, roots['plain-a'] = host_only(resources, seconds, 'a', out)
    with NativeSession(resources, 'perf-load-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        roots['vape'] = str(session.root)
        r = world(session, checks)
        nothing_on(r)
        found = {'small': scene(r, 24)}
        out['off-small'] = bench(r, seconds)
        found['dense'] = scene(r, DENSE)
        out['off-dense'] = bench(r, seconds)
        for module in ENTITY:
            r.toggle(module, True)
        r.wait(20)
        out['entities-dense'] = bench(r, seconds)
        for module in ENTITY:
            r.toggle(module, False)
        r.wait(20)
        out['after-dense'] = bench(r, seconds)
        ok_b, roots['plain-b'] = host_only(resources, seconds, 'b', out)
        out['roots'] = roots
        for name in ('small', 'dense'):
            a, b = cpu(out[f'plain-a-{name}']), cpu(out[f'plain-b-{name}'])
            base = (a + b) / 2
            out[f'plain-{name}-noise_ms'] = abs(a - b)
            cost = cpu(out[f'off-{name}']) - base
            out[f'off-{name}-added_ms'] = cost
            checks.check(f'{name} scene: vape-rs with every module off adds under {OFF_MS} ms of CPU per frame '
                         'over the host without it', cost < OFF_MS,
                         {'added_ms': round(cost, 3), 'plain_ms': [round(a, 3), round(b, 3)],
                          'vape_off_ms': round(cpu(out[f'off-{name}']), 3)})
        cost = cpu(out['entities-dense']) - min(cpu(out['off-dense']), cpu(out['after-dense']))
        out['entities-dense-added_ms'] = cost
        checks.check(f'{DENSE} cows: ESP, Tracers and NameTags add under {ENTITY_MS} ms of CPU per frame',
                     cost < ENTITY_MS, {'added_ms': round(cost, 3), 'cpu_p99_ms': out['entities-dense']['cpu_p99_ms']})
        checks.check('both scenes are in place', found == {'small': 24, 'dense': DENSE}, found)
        checks.check('the host-only runs passed', ok_a and ok_b)
        benches = [v for v in out.values() if isinstance(v, dict) and 'frames' in v]
        checks.check('frames were recorded in every bench', all((b['frames'] or 0) > 50 for b in benches),
                     [b['frames'] for b in benches])
        fps = [b['avg_fps'] or 0 for b in benches]
        checks.check('every bench ran at the same frame cap', min(fps) > 0.8 * max(fps), [round(f, 1) for f in fps])
        return finish(session, checks, out, 'disabled')


def fill(r, checks, check=True):
    r.call('command', 'clear @s')
    for i, entry in enumerate(FULL):
        slot = f'hotbar.{i}' if i < 9 else f'inventory.{i - 9}'
        item, *count = entry.split()
        r.call('command', f'item replace entity @s {slot} with minecraft:{item} {count[0] if count else 1}')
    r.call('hotbar', '5')
    r.wait(10)
    if check:
        inv = r.call('inventory')
        filled = sum(1 for e in inv['hotbar'] + inv['main'] if e.get('item'))
        checks.check('every inventory slot is filled', filled == 36, filled)


def planners(resources, allow_stale, seconds):
    with NativeSession(resources, 'perf-planners-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        r = world(session, checks)
        out = {}
        r.call('chat', '/kill @e[type=!minecraft:player]')
        fill(r, checks)
        nothing_on(r)
        out['full'] = bench(r, seconds)
        r.call('command', 'clear @s')
        r.wait(10)
        switch(r, (IM, IC, AH), True)
        # The planners start on the first slots while the rest are filled.
        fill(r, checks, check=False)
        out['working'] = bench(r, seconds)
        r.wait(200)
        out['arranged'] = bench(r, seconds)
        out['screen'] = r.call('state').get('screen')
        switch(r, (IM, IC, AH), False)
        r.wait(20)
        if r.call('state').get('screen') != 'playing':
            r.call('key', 'escape', 'tap')
            r.wait(10)
        out['after'] = bench(r, seconds)
        inv = r.call('inventory')
        out['hotbar_after'] = [(e.get('item'), e.get('count')) for e in inv['hotbar']]
        out['counts_after'] = inv.get('counts', {})
        out['armor_after'] = [e.get('item') for e in inv.get('armor', [])]
        base = min(tick(out['full']), tick(out['after']))
        for name in ('working', 'arranged'):
            cost = tick(out[name]) - base
            out[f'{name}-tick-added_ms'] = cost
            checks.check(f'three planners on a full inventory ({name}) add under {PLANNER_TICK_MS} ms '
                         'to the frame tick', cost < PLANNER_TICK_MS,
                         {'added_ms': round(cost, 3), 'tick_ms': round(tick(out[name]), 3),
                          'cpu_ms': round(cpu(out[name]), 3)})
        counts = out['counts_after']
        checks.check('the planners worked on the inventory: the best sword in slot 1 (AutoHotbar '
                     'or InventoryManager), the rotten flesh dropped, the best armor worn',
                     out['hotbar_after'][0][0] == 'minecraft:diamond_sword'
                     and not counts.get('minecraft:rotten_flesh')
                     and 'minecraft:iron_chestplate' in out['armor_after'],
                     {'hotbar': out['hotbar_after'], 'armor': out['armor_after'],
                      'rotten_flesh': counts.get('minecraft:rotten_flesh', 0)})
        return finish(session, checks, out, 'planners')


def raw(session, *args, timeout=30):
    """A control call outside the session's record, for the walker thread."""
    return subprocess.run([str(session.ctl), '--socket', str(session.socket), *map(str, args)],
                          cwd=session.root / 'game', env=session.env, capture_output=True,
                          text=True, timeout=timeout).returncode


def walk(guest, seconds, stop):
    """Walk a circle: forward, the yaw turning a full circle every four seconds
    of wall time (about 2.75 blocks across at walking speed, however slow the
    control calls are under load), until `seconds` have passed or `stop` is set."""
    raw(guest, 'key', 'w', 'down')
    t0 = time.monotonic()
    while not stop.is_set() and time.monotonic() - t0 < seconds:
        raw(guest, 'look', round((time.monotonic() - t0) * 90.0 % 360, 1), '10')
        # A sleep, not a control `wait`: the session runs one wait at a time.
        time.sleep(0.2)
    raw(guest, 'key', 'w', 'up')


def other(session):
    found = [e for e in session.call('entities', '64').get('entities', [])
             if e.get('type') == 'minecraft:player']
    return found[0] if found else None


def window(session, seconds_ticks, between=()):
    """Frame statistics over `seconds_ticks` ticks, after the given calls."""
    session.call('stats', 'reset')
    for f in between:
        f()
    session.call('wait', str(seconds_ticks))
    s = session.call('stats')
    cpu_ = s.get('cpu') or {}
    return {'frames': s.get('frames'), 'max_ms': s.get('max_ms'), 'cpu_max_ms': cpu_.get('max_ms'),
            'cpu_p99_ms': cpu_.get('p99_ms'), 'phases_avg_ms': s.get('phases_avg_ms'),
            'slow_phases_avg_ms': s.get('slow_phases_avg_ms')}


def floor(h, checks):
    """Run.arena's stone floor and cleared air, without its void column (a
    walk needs neither, and nearby water can flow into it)."""
    for _ in range(10):
        h.fill(-4, FLOOR, -30, 9, FLOOR, 30, 'minecraft:stone')
        h.fill(-4, FLOOR + 1, -30, 9, FLOOR + 11, 30, 'minecraft:air')
        counts = h.call('blocks', -4, FLOOR, -30, 9, FLOOR, 30).get('counts', {})
        if counts.get('minecraft:stone') == 14 * 61:
            break
        h.wait(20)
    checks.check('the arena floor is in place', counts.get('minecraft:stone') == 14 * 61, counts)


def blink(resources, allow_stale, seconds):
    with NativeSession(resources, 'perf-blink-host-', allow_stale=allow_stale) as host, \
            NativeSession(resources, 'perf-blink-guest-', allow_stale=allow_stale) as guest:
        checks = Checks(guest.root)
        for s in (host, guest):
            (s.root / 'game').mkdir(parents=True, exist_ok=True)
            (s.root / 'game' / 'options.txt').write_text('inactivityFpsLimit:"minimized"\n')
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=False, world='LanHost')
        host.call('time', '6000')
        h = Combat(host, checks, {})
        h.wait(20)
        floor(h, checks)
        host.call('command', 'gamerule advance_time false')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        guest.seed({'schema': 1, 'modules': {}, 'settings': {f'{BLINK}.binds': [BINDS[BLINK]],
                                                             f'{BLINK}.breadcrumbs': True}})
        guest.launch(creative=False, world=None, extra=('--server', f'localhost:{port}', '--name', 'VapeGuest'))
        guest.call('wait-ready', '60')
        # The host watches from the arena (it spawns wherever the world's spawn
        # is, out of entity range).
        host.call('tp', '2.5', str(FLOOR + 1), '-12.5')
        host.call('look', '0', '0')
        host.call('command', f'tp VapeGuest 2.5 {FLOOR + 1} 0.5 0 0')
        host.call('command', 'effect give VapeGuest minecraft:resistance 100000 4 true')
        g = Combat(guest, checks, {BLINK: BINDS[BLINK]})
        g.wait(40)
        out = {'guest-plain': bench(g, seconds)}
        # The same window as the flush below, with Blink on and straight off
        # again holding nothing: the toggle's own frame cost.
        out['control'] = window(guest, 36, (lambda: g.toggle(BLINK, True), lambda: g.toggle(BLINK, False)))

        hold(host, guest, g, checks, out, HOLD_S, seconds)
        # Past the host's keep-alive limit: the answers are not held, so the
        # guest stays connected and the queue holds the whole walk.
        hold(host, guest, g, checks, out, LONG_HOLD_S, seconds)
        return finish(guest, checks, out, 'blink')


def hold(host, guest, g, checks, out, hold_s, seconds):
    """Blink on while the guest walks for `hold_s` seconds with a bench
    alongside on its own control connection, then off: the host's view of the
    guest before and after, and the frames that send the queue."""
    name = f'{hold_s}s'
    host.call('command', f'tp VapeGuest 5.5 {FLOOR + 1} 0.5 0 0')
    g.wait(20)
    start = g.pos()
    g.toggle(BLINK, True)
    stop = threading.Event()
    walker = threading.Thread(target=walk, args=(guest, hold_s, stop))
    walker.start()
    out[f'blink-walking-{name}'] = bench(g, min(seconds, hold_s - 2))
    walker.join(timeout=hold_s * 3 + 30)
    stop.set()
    walker.join()
    client = g.pos()
    seen = other(host)
    held_view = seen and seen['pos']
    state = guest.call('state')
    out[f'held-{name}'] = {'screen': state.get('screen'), 'server_view': held_view}
    checks.check(f'the guest is still connected at the end of a {hold_s} s hold',
                 state.get('screen') == 'playing' and state.get('player') is not None, state.get('screen'))
    out[f'flush-{name}'] = window(guest, 36, (lambda: g.toggle(BLINK, False), lambda: g.wait(2)))
    g.wait(20)
    seen = other(host)
    server = seen and seen['pos']
    out[f'positions-{name}'] = {'start': start, 'client': client, 'server': server}
    walked = sum((a - b) ** 2 for a, b in zip(client, start)) ** 0.5
    held = None if not held_view else sum((a - b) ** 2 for a, b in zip(held_view, start)) ** 0.5
    apart = None if not server else sum((a - b) ** 2 for a, b in zip(client, server)) ** 0.5
    checks.check(f'the host saw the guest stand still through the {hold_s} s hold', held is not None and held < 0.5,
                 {'held_server_moved': held, 'client_walked': walked})
    checks.check(f'the host has the guest where it walked once Blink sent {hold_s} s of moves',
                 walked > 1.0 and apart is not None and apart < 1.0,
                 {'walked': walked, 'apart': apart, **out[f'positions-{name}']})
    cost = cpu(out[f'blink-walking-{name}']) - cpu(out['guest-plain'])
    out[f'blink-walking-{name}-added_ms'] = cost
    checks.check(f'walking under Blink with breadcrumbs adds under {BLINK_MS} ms of CPU per frame ({hold_s} s hold)',
                 cost < BLINK_MS, {'added_ms': round(cost, 3),
                                   'cpu_p99_ms': out[f'blink-walking-{name}']['cpu_p99_ms']})
    # The held packets go out in the client tick (outbound_tick), so the
    # tick phase of the window's slow frames carries the flush.
    flush = out[f'flush-{name}']
    flush_tick = (flush['slow_phases_avg_ms'] or {}).get('tick') or 0
    checks.check(f'sending {hold_s} s of held packets keeps the slow frames\' tick under {FLUSH_TICK_MS} ms',
                 flush_tick < FLUSH_TICK_MS, {'slow_tick_ms': round(flush_tick, 3), 'flush': flush,
                                              'control': out['control']})
    # The frame maximum also counts time outside the four phases (present,
    # the scheduler under load), and prepare spikes come and go with the
    # chunks in view either way, so the bound is on the slow frames' phases.
    spent = sum((flush['slow_phases_avg_ms'] or {}).values())
    control = sum((out['control']['slow_phases_avg_ms'] or {}).values())
    checks.check(f'the slow frames sending {hold_s} s of packets spend within {FLUSH_MS} ms of a toggle that '
                 'holds nothing', spent < control + FLUSH_MS,
                 {'slow_phases_ms': round(spent, 3), 'control_slow_phases_ms': round(control, 3),
                  'flush_max_ms': flush['max_ms'], 'control_max_ms': out['control']['max_ms']})


def pen(h, host, cows):
    """`cows` cows with their AI in a 5x5 fenced pen at z 15-19, cramming off:
    they push each other all the time, so each one's tracker sends moves."""
    host.call('command', 'gamerule max_entity_cramming 0')
    h.fill(-3, FLOOR + 1, 14, 3, FLOOR + 1, 20, 'minecraft:oak_fence')
    h.fill(-2, FLOOR + 1, 15, 2, FLOOR + 1, 19, 'minecraft:air')
    for i in range(cows):
        host.call('command', f'summon minecraft:cow {-1.5 + i % 5} {FLOOR + 1} {15.5 + (i // 5) % 5}')


def pace(host, stop):
    """The host walks 1.4 s away from the guest (towards the pen) and 1.4 s
    back, from the same spot each time, until `stop` is set."""
    while not stop.is_set():
        raw(host, 'tp', '0.5', FLOOR + 1, '7.0')
        raw(host, 'look', '0', '0')
        for key in ('w', 's'):
            raw(host, 'key', key, 'down')
            time.sleep(1.4)
            raw(host, 'key', key, 'up')


def lags(host, guest, samples=16):
    """How far the guest's view of the host trails the host's own position."""
    out = []
    for _ in range(samples):
        seen = guest.call('entities', '24', 'player').get('entities', [])
        me = (host.call('state').get('player') or {}).get('position')
        if seen and me:
            out.append(round(sum((a - b) ** 2 for a, b in zip(seen[0]['pos'], me)) ** 0.5, 3))
        time.sleep(0.2)
    return out


def backtrack(resources, allow_stale, seconds):
    with NativeSession(resources, 'perf-backtrack-host-', allow_stale=allow_stale) as host, \
            NativeSession(resources, 'perf-backtrack-guest-', allow_stale=allow_stale) as guest:
        checks = Checks(guest.root)
        for s in (host, guest):
            (s.root / 'game').mkdir(parents=True, exist_ok=True)
            (s.root / 'game' / 'options.txt').write_text('inactivityFpsLimit:"minimized"\n')
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=False, world='LanHost')
        host.call('time', '6000')
        h = Combat(host, checks, {})
        h.wait(20)
        floor(h, checks)
        host.call('command', 'gamerule advance_time false')
        pen(h, host, PEN_COWS)
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        guest.seed({'schema': 1, 'modules': {}, 'settings': {f'{BACKTRACK}.binds': [BINDS[BACKTRACK]],
                                                             f'{BACKTRACK}.latency': [500.0, 500.0]}})
        guest.launch(creative=False, world=None, extra=('--server', f'localhost:{port}', '--name', 'VapeGuest'))
        guest.call('wait-ready', '60')
        host.call('command', 'effect give @a minecraft:resistance 100000 4 true')
        host.call('command', f'tp VapeGuest 0.5 {FLOOR + 1} 4.5 0 0')
        host.call('tp', '0.5', str(FLOOR + 1), '7.0')
        g = Combat(guest, checks, {BACKTRACK: BINDS[BACKTRACK]})
        g.wait(60)
        cows = len(guest.call('entities', '48', 'cow').get('entities', []))
        checks.check(f'the guest sees the {PEN_COWS} cows in the pen', cows >= PEN_COWS - 10, cows)
        out = {'cows': cows}

        def paced(name, attack=False):
            host.call('tp', '0.5', str(FLOOR + 1), '7.0')
            g.wait(10)
            if attack:
                target = guest.call('entities', '8', 'player').get('entities', [])
                checks.check('the guest sees the host to attack', bool(target), target)
                if target:
                    guest.call('look-at-entity', str(target[0]['id']))
                    guest.call('attack', str(target[0]['id']))
            stop = threading.Event()
            walker = threading.Thread(target=pace, args=(host, stop))
            walker.start()
            try:
                out[name] = bench(g, seconds)
                out[f'{name}-lag'] = lags(host, guest)
                out[f'{name}-window'] = window(guest, 100)
            finally:
                stop.set()
                walker.join()
                raw(host, 'key', 'w', 'up')
                raw(host, 'key', 's', 'up')

        paced('bt-plain')
        g.toggle(BACKTRACK, True)
        paced('bt-holding', attack=True)
        host.call('command', 'kill @e[type=minecraft:cow]')
        g.wait(20)
        host.call('command', 'kill @e[type=minecraft:item]')
        g.wait(20)
        left = len(guest.call('entities', '48', 'cow').get('entities', []))
        checks.check('the pen is empty for the sparse hold', left == 0, left)
        paced('bt-sparse', attack=True)
        g.toggle(BACKTRACK, False)
        state = guest.call('state')
        checks.check('the guest is still connected after BackTrack held the pen\'s traffic',
                     state.get('screen') == 'playing' and state.get('player') is not None, state.get('screen'))
        # Unheld, the guest draws the host a couple of ticks behind; the
        # median skips the jump of each teleport back to the start.
        plain = sorted(out['bt-plain-lag'])[len(out['bt-plain-lag']) // 2] if out['bt-plain-lag'] else 0
        held, sparse = (max(out[f'{k}-lag'], default=0) for k in ('bt-holding', 'bt-sparse'))
        out['bt-hold-blocks'] = {'plain_median': plain, 'pen_max': held, 'sparse_max': sparse}
        checks.check('BackTrack held the host behind its position beside the pen',
                     bool(out['bt-holding-lag']) and held > plain + 0.3,
                     {'plain_median': plain, 'held_max': held, 'held': out['bt-holding-lag']})
        # 500 ms of walking (4.3 blocks/s) is about 2.2 blocks; a tick and the
        # sampling either way.
        checks.check('without the pen BackTrack held the host 500 ms behind, not longer',
                     bool(out['bt-sparse-lag']) and plain + 1.5 < sparse < plain + 3.0,
                     {'plain_median': plain, 'sparse_max': sparse, 'sparse': out['bt-sparse-lag']})
        cost = cpu(out['bt-holding']) - cpu(out['bt-plain'])
        out['bt-holding-added_ms'] = cost
        checks.check(f'holding {PEN_COWS} pushing cows\' traffic adds under {BACKTRACK_MS} ms of CPU per frame',
                     cost < BACKTRACK_MS, {'added_ms': round(cost, 3), 'cpu_p99_ms': out['bt-holding']['cpu_p99_ms']})
        # Holds, replays at the queue limit and the flush on each turn run in
        # the packet handling of the client tick.
        ticks = {k: (out[f'{k}-window']['slow_phases_avg_ms'] or {}).get('tick') or 0
                 for k in ('bt-plain', 'bt-holding', 'bt-sparse')}
        checks.check(f'replaying the full queue keeps the slow frames\' tick under {REPLAY_TICK_MS} ms',
                     ticks['bt-holding'] < REPLAY_TICK_MS,
                     {**{k: round(v, 3) for k, v in ticks.items()},
                      'max_ms': {k: round(out[f'{k}-window']['max_ms'] or 0, 1) for k in ('bt-plain', 'bt-holding')}})
        return finish(guest, checks, out, 'backtrack')


def finish(session, checks, out, name):
    log = (session.root / 'game.log').read_text(errors='replace')
    checks.check('no panics in the log', 'panicked' not in log and 'poisoned' not in log)
    (session.root / 'measurements.json').write_text(json.dumps(out, indent=2, default=str))
    ok = all(c['ok'] for c in checks.results)
    print('PASS' if ok else 'FAIL', name, f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}',
          session.root, flush=True)
    for k, v in out.items():
        if isinstance(v, dict) and 'cpu_avg_ms' in v:
            print(f'  {k:22} cpu {cpu(v):7.3f} ms  p99 {v["cpu_p99_ms"] or 0:7.3f}  tick {tick(v):6.3f}  '
                  f'fps {v["avg_fps"] or 0:6.1f}  load {v["load"]:.2f}', flush=True)
        elif isinstance(v, float):
            print(f'  {k:22} {v:+.3f}', flush=True)
    return ok


PARTS = {'disabled': disabled, 'planners': planners, 'blink': blink, 'backtrack': backtrack}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?', default=os.environ.get('HORIZON_RESOURCES'))
    parser.add_argument('--allow-stale', action='store_true')
    parser.add_argument('--seconds', type=int, default=8)
    parser.add_argument('--only', default='', help=f'comma-separated parts: {",".join(PARTS)}')
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    only = [p for p in args.only.split(',') if p] or list(PARTS)
    unknown = [p for p in only if p not in PARTS]
    if unknown:
        parser.error(f'unknown parts: {unknown}')
    results = {}
    for part in only:
        try:
            results[part] = PARTS[part](args.resources, args.allow_stale, args.seconds)
        except Exception as error:  # report and go on to the next part
            print('FAIL', part, repr(error), flush=True)
            results[part] = False
    print(results)
    raise SystemExit(0 if all(results.values()) else 1)


if __name__ == '__main__':
    main()
