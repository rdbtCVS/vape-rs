#!/usr/bin/env python3
"""Controlled LAN scenario for the inbound hold queue: Blink's Bi-directional
mode, Backtrack and FakeLag Repel, over the real protocol.

A fresh host world is opened to LAN on localhost and a fresh guest client runs
vape-rs against it; each process has its own isolated homes. The host talks
(`say`) and pushes a pig around, so the guest's inbound traffic is known:

* Blink Bi-directional holds chat lines (order, nothing early), releases them
  in order when Blink turns off, lets a teleport (connection-critical) flush
  everything held before it and pass, drops its queue on disconnect (nothing
  replays into the next connection) and holds again after reconnecting.
* Backtrack keeps the pig where it was while it moves away (guest view lags
  the host's true position), flushes on a teleport and on disable, and a held
  chat line behind the pig's updates stays in order.
* FakeLag Repel holds an attack on a target that is still hurt and sends it
  by the deadline, so the second hit lands late; it releases on disable.

Results go to <session>/result.json via Checks.
"""
import argparse
import json
import time
from pathlib import Path

from native_session import NativeSession
from smoke_scenario import Checks, enabled, poll, state

BINDS = {'blatant.blink': 'KeyN', 'blatant.backtrack': 'KeyM', 'world.fake-lag': 'KeyJ'}


def say(host, text):
    """A system-chat line to every client (`say` is not shown to LAN guests)."""
    host.call('command', 'tellraw @a {"text":"%s"}' % text)


def toggle(session, module):
    session.call('key', BINDS[module][3:].lower(), 'tap')
    session.call('wait', '2')


def messages(session):
    # `messages` lists chat newest first; oldest first reads naturally.
    lines = session.call('messages', '80').get('chat') or []
    return [m if isinstance(m, str) else json.dumps(m) for m in reversed(lines)]


def seen(session, tag):
    """Index of the first chat line that carries tag, or None."""
    for i, line in enumerate(messages(session)):
        if tag in line:
            return i
    return None


def pig(session):
    """The pig as the session sees it: a remote guest lists it under
    `entities`, the integrated-server host under `mobs`."""
    found = [e for e in session.call('entities', '40').get('entities', []) if e.get('type') == 'minecraft:pig']
    if not found:
        found = [m for m in session.call('mobs', '40').get('near', []) if m.get('kind') == 'minecraft:pig']
    return found[0] if found else None


def strike(host, guest):
    """Pull the pig in front of the guest and attack it; retry if it wandered."""
    for _ in range(4):
        host.call('command', 'tp @e[type=pig,limit=1] 0.5 100 2')
        guest.call('wait', '3')
        p = pig(guest)
        if not p:
            continue
        try:
            guest.call('look-at-entity', str(p['id']))
            guest.call('attack', str(p['id']))
            return p
        except RuntimeError:
            continue
    raise RuntimeError('could not reach the pig')


def run(resources, allow_stale):
    with NativeSession(resources, 'hold-host-', allow_stale=allow_stale) as host, \
            NativeSession(resources, 'hold-guest-', allow_stale=allow_stale) as guest:
        checks = Checks(guest.root)
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=False, world='HoldHost')
        host.call('time', '6000')
        host.call('tp', '0.5', '100', '0.5')
        host.call('wait-ready', '40')
        host.call('fill', '-12', '96', '-12', '12', '99', '40', 'minecraft:stone')
        host.call('fill', '-12', '100', '-12', '12', '106', '40', 'minecraft:air')
        host.call('command', 'gamerule mob_griefing false')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        address = f'localhost:{port}'

        guest.seed({'schema': 1, 'modules': {},
                    'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()},
                                 'blatant.blink.direction': 'Bi-directional',
                                 'blatant.backtrack.latency': [500.0, 500.0],
                                 'blatant.backtrack.render-server-pos': True,
                                 'world.fake-lag.mode': 'Repel',
                                 'world.fake-lag.delay': 600.0}})
        guest.launch(creative=False, world=None, extra=('--server', address, '--name', 'VapeGuest'))
        guest.call('wait-ready', '60')
        host.call('command', 'gamerule mob_griefing false')
        host.call('command', 'tp VapeGuest 0.5 100 0.5')
        # The host player stays out of the guest's crosshair.
        host.call('tp', '10.5', '100', '10.5')
        guest.call('wait', '20')
        s = state(guest)
        checks.check('guest joined the LAN world', s['player']['health'] > 0, s['player']['position'])

        # --- Blink, Bi-directional --------------------------------------
        say(host, 'before')
        guest.call('wait', '10')
        checks.check('chat arrives with Blink off', seen(guest, 'before') is not None, messages(guest)[-3:])
        toggle(guest, 'blatant.blink')
        checks.check('guest Blink is on', enabled(guest, 'blatant.blink'))
        for tag in ('m1', 'm2', 'm3'):
            say(host, tag)
        guest.call('wait', '30')
        held = [seen(guest, t) for t in ('m1', 'm2', 'm3')]
        checks.check('Bi-directional Blink holds inbound chat', held == [None] * 3, held)
        toggle(guest, 'blatant.blink')
        ok, _ = poll(guest, lambda _s: seen(guest, 'm3') is not None, 40)
        order = [seen(guest, t) for t in ('m1', 'm2', 'm3')]
        checks.check('turning Blink off releases the held chat in order',
                     ok and None not in order and order == sorted(order), order)

        # Critical packets flush what is held before them and pass.
        toggle(guest, 'blatant.blink')
        say(host, 'a1')
        say(host, 'a2')
        guest.call('wait', '10')
        early = [seen(guest, t) for t in ('a1', 'a2')]
        host.call('command', 'tp VapeGuest 4.5 100 0.5')
        ok, s = poll(guest, lambda s: s['player']['position'][0] > 3.5, 40)
        checks.check('a teleport passes while Blink holds', ok, s['player']['position'])
        late = [seen(guest, t) for t in ('a1', 'a2')]
        checks.check('chat held before the teleport was flushed ahead of it',
                     early == [None, None] and None not in late and late[0] < late[1], [early, late])
        say(host, 'a3')
        guest.call('wait', '20')
        checks.check('Blink goes on holding after a flush', seen(guest, 'a3') is None)

        # Cleanup on disconnect: nothing replays into the next connection.
        say(host, 'c1')
        guest.call('wait', '10')
        guest.call('disconnect')
        time.sleep(1)
        guest.call('connect', address)
        guest.call('wait-ready', '60')
        guest.call('wait', '20')
        checks.check('reconnected with Blink still enabled', enabled(guest, 'blatant.blink'))
        checks.check('held chat from the old connection is gone', seen(guest, 'c1') is None, messages(guest)[-5:])
        say(host, 'd1')
        guest.call('wait', '30')
        checks.check('a new connection holds again from empty', seen(guest, 'd1') is None)
        toggle(guest, 'blatant.blink')
        ok, _ = poll(guest, lambda _s: seen(guest, 'd1') is not None, 40)
        checks.check('and releases on disable', ok)
        checks.check('the old connection\'s lines never replayed', seen(guest, 'c1') is None)

        # --- Backtrack ----------------------------------------------------
        host.call('command', 'tp VapeGuest 0.5 100 0.5')
        guest.call('wait', '20')
        guest.call('look', '0', '0')
        host.call('command', 'kill @e[type=pig]')
        host.call('command', 'summon pig 0.5 100 2 {PersistenceRequired:1b}')
        guest.call('wait', '20')

        def lag_samples(ticks=30):
            samples = []
            for _ in range(ticks // 2):
                g, h = pig(guest), pig(host)
                if g and h:
                    samples.append((g['pos'][2], h['pos'][2]))
                guest.call('wait', '2')
            return samples

        def hit_and_watch():
            # The pig wanders; bring it back in front of us first.
            strike(host, guest)
            host.call('command', 'data merge entity @e[type=pig,limit=1] {Motion:[0.0,0.2,0.5]}')
            return lag_samples()

        base = hit_and_watch()
        base_lag = max((h - g for g, h in base), default=0)
        checks.check('without Backtrack the pig tracks the server position',
                     bool(base) and base_lag < 0.6, base_lag)
        guest.call('wait', '40')
        host.call('command', 'tp @e[type=pig,limit=1] 0.5 100 2')
        guest.call('wait', '20')
        toggle(guest, 'blatant.backtrack')
        checks.check('guest Backtrack is on', enabled(guest, 'blatant.backtrack'))
        held_lag = max((h - g for g, h in hit_and_watch()), default=0)
        checks.check('Backtrack keeps the pig behind its server position while it backs off',
                     held_lag > 0.8, held_lag)
        ok, _ = poll(guest, lambda _s: (pig(guest) and pig(host) and
                                        abs(pig(guest)['pos'][2] - pig(host)['pos'][2]) < 0.1), 60)
        checks.check('the held updates play out and the views converge', ok)

        # A chat line behind held pig updates keeps its place.
        host.call('command', 'data merge entity @e[type=pig,limit=1] {Motion:[0.0,0.2,0.5]}')
        say(host, 'b1')
        say(host, 'b2')
        guest.call('wait', '40')
        order = [seen(guest, 'b1'), seen(guest, 'b2')]
        checks.check('chat behind held updates arrives in order', None not in order and order[0] < order[1], order)

        # Disable flushes it all.
        guest.call('wait', '40')
        host.call('command', 'tp @e[type=pig,limit=1] 0.5 100 2')
        # The teleport of the tracked target is held like its moves (500 ms).
        guest.call('wait', '14')
        strike(host, guest)
        host.call('command', 'data merge entity @e[type=pig,limit=1] {Motion:[0.0,0.2,0.6]}')
        guest.call('wait', '6')

        def gap():
            g, h = pig(guest), pig(host)
            return (h['pos'][2] - g['pos'][2]) if g and h else None

        before = gap()
        toggle_key = BINDS['blatant.backtrack'][3:].lower()
        guest.call('key', toggle_key, 'tap')
        # The queue is due 500 ms (10 ticks) after each update; a view that
        # catches up within 9 ticks was released by the disable, not by time.
        after = []
        for _ in range(9):
            guest.call('wait', '1')
            after.append(gap())
        checks.check('disabling Backtrack releases what is held at once',
                     before is not None and before > 0.8 and min(abs(g) for g in after if g is not None) < 0.5,
                     [before, after])

        # --- FakeLag Repel ------------------------------------------------
        guest.call('wait', '40')
        host.call('command', 'tp VapeGuest 0.5 100 0.5')
        host.call('command', 'tp @e[type=pig,limit=1] 0.5 100 2')
        host.call('command', 'data merge entity @e[type=pig,limit=1] {NoAI:1b,Motion:[0.0,0.0,0.0]}')
        guest.call('wait', '20')
        guest.call('look', '0', '0')

        def pig_health():
            return pig(host)['health']

        p = pig(guest)
        guest.call('look-at-entity', str(p['id']))
        h0 = pig_health()
        guest.call('attack', str(p['id']))
        guest.call('wait', '6')
        h1 = pig_health()
        toggle(guest, 'world.fake-lag')
        checks.check('guest FakeLag is on', enabled(guest, 'world.fake-lag'))
        guest.call('wait', '24')
        # First hit on an unhurt target goes straight through, even with Repel.
        t0 = time.monotonic()
        guest.call('attack', str(p['id']))
        guest.call('wait', '4')
        first = pig_health()
        # Second hit while still hurt is held until the deadline or the next send.
        guest.call('attack', str(p['id']))
        guest.call('wait', '2')
        mid = pig_health()
        guest.call('wait', '40')
        last = pig_health()
        checks.check('the first Repel hit is not delayed', first < h1, [h0, h1, first])
        checks.check('the hurt-target attack is held, then lands by its deadline',
                     mid == first and last < first, [first, mid, last, round(time.monotonic() - t0, 1)])
        toggle(guest, 'world.fake-lag')

        for name, session in (('host', host), ('guest', guest)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
            checks.check(f'no replay failures in the {name} log', 'replaying held packets failed' not in log)
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
