#!/usr/bin/env python3
"""Post-merge smoke scenario for Sprint, Parkour, AutoTotem, AutoArmor and Panic.

Runs in a fresh isolated survival world on the integrated server and
checks host-observable results through the control socket: the canonical key
set, sprint state, position, offhand, armor and screen. It also exercises the
interactions the merge round left untested: controller sprint while the
module releases its key, menus, a no-op inventory click, Panic restore, and
death/respawn. Results are written to result.json in the session directory.
"""
import argparse
import json
from pathlib import Path
import time

from native_session import NativeSession

BINDS = {
    'combat.sprint': 'KeyK',
    'world.parkour': 'KeyP',
    'inventory.auto-totem': 'KeyO',
    'inventory.auto-armor': 'KeyU',
}


class Checks:
    def __init__(self, root):
        self.root = root
        self.results = []

    def check(self, name, ok, detail=None):
        self.results.append({'check': name, 'ok': bool(ok), 'detail': detail})
        print(f"{'PASS' if ok else 'FAIL'} {name}" + (f': {detail}' if detail is not None else ''),
              flush=True)
        self.write()

    def write(self):
        (self.root / 'result.json').write_text(json.dumps(self.results, indent=2) + '\n')


def state(session):
    """Agent state (player position, sprinting) plus the control status'
    canonical key set, offhand and screen name."""
    merged = session.call('state')
    status = session.call('status')
    merged.update(keys=status.get('keys', []), offhand=status.get('offhand'),
                  screen=status.get('screen'))
    return merged


def keys(session):
    return set(state(session).get('keys', []))


def toggle(session, module):
    session.call('key', BINDS[module][3:].lower(), 'tap')
    session.call('wait', '2')


def enabled(session, module):
    return session.saved()['modules'].get(module, {}).get('enabled', False)


def poll(session, predicate, ticks=60, step=2):
    """Wait up to `ticks` game ticks for predicate(state) to hold."""
    last = None
    for _ in range(0, ticks, step):
        last = state(session)
        if predicate(last):
            return True, last
        session.call('wait', str(step))
    last = state(session)
    return predicate(last), last


def inventory(session):
    return session.call('inventory')


def run(resources, allow_stale):
    with NativeSession(resources, 'smoke-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({
            'schema': 1,
            'modules': {},
            'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()},
                         'utility.panic.re-enable': True,
                         'inventory.auto-armor.inventory-only': False},
        })
        session.launch(creative=False)
        session.call('time', '6000')
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        # A stone runway at y=99 (z -3..20) above a floor at y=96..98 that
        # ends with a 1-block drop; everything stays inside loaded chunks.
        for _ in range(10):
            session.call('fill', '-3', '96', '-3', '3', '98', '28', 'minecraft:stone')
            session.call('fill', '-3', '99', '-3', '3', '99', '20', 'minecraft:stone')
            session.call('fill', '-3', '100', '-3', '3', '104', '28', 'minecraft:air')
            counts = session.call('blocks', '-3', '96', '-3', '3', '104', '28').get('counts', {})
            if counts.get('minecraft:stone') == 7 * 32 * 3 + 7 * 24:
                break
            session.call('wait', '20')
        checks.check('scenario terrain is in place', counts.get('minecraft:stone') == 7 * 32 * 3 + 7 * 24,
                     counts)
        session.call('tp', '0.5', '100', '0.5')
        session.call('look', '0', '0')
        session.call('wait', '10')

        # --- Sprint: synthetic sprint while walking forward.
        toggle(session, 'combat.sprint')
        checks.check('sprint enabled through its bind', enabled(session, 'combat.sprint'))
        session.call('key', 'w', 'down')
        ok, s = poll(session, lambda s: 'ControlLeft' in s.get('keys', []), 20)
        checks.check('sprint module holds the canonical sprint key', ok, s.get('keys'))
        ok, s = poll(session, lambda s: s['player'].get('sprinting'), 20)
        checks.check('player sprints', ok, s['player'].get('sprinting'))

        # --- Controller sprint held while the module is disabled: the host's
        # toggle mapping owns it, so disabling must not clear it.
        session.call('pad', 'ls', 'down')
        session.call('wait', '2')
        toggle(session, 'combat.sprint')
        ok, s = poll(session, lambda s: 'ControlLeft' in s.get('keys', []), 6)
        checks.check('disabling Sprint keeps a controller-held sprint', ok and not enabled(session, 'combat.sprint'),
                     s.get('keys'))
        session.call('pad', 'ls', 'up')
        ok, s = poll(session, lambda s: 'ControlLeft' not in s.get('keys', []), 10)
        checks.check('controller release ends sprint key', ok, s.get('keys'))
        session.call('key', 'w', 'up')

        # --- Menus: with Sprint on, a pause menu hands the key back at once.
        toggle(session, 'combat.sprint')
        session.call('key', 'w', 'down')
        poll(session, lambda s: 'ControlLeft' in s.get('keys', []), 20)
        session.call('pause')
        time.sleep(0.5)
        held = keys(session)
        checks.check('pause menu releases synthetic sprint', 'ControlLeft' not in held, sorted(held))
        session.call('resume')
        session.call('key', 'w', 'down')
        ok, s = poll(session, lambda s: 'ControlLeft' in s.get('keys', []), 20)
        checks.check('sprint resumes after the menu', ok, s.get('keys'))
        session.call('key', 'w', 'up')
        toggle(session, 'combat.sprint')
        ok, s = poll(session, lambda s: 'ControlLeft' not in s.get('keys', []), 10)
        checks.check('disabling Sprint releases its key', ok, s.get('keys'))

        # --- Parkour: walking off the runway end jumps at the edge.
        session.call('tp', '0.5', '100', '17.5')
        session.call('look', '0', '0')
        session.call('wait', '10')
        toggle(session, 'world.parkour')
        checks.check('parkour enabled', enabled(session, 'world.parkour'))
        start = state(session)['player']['position'][1]
        peak = start
        session.call('key', 'w', 'down')
        for _ in range(30):
            session.call('wait', '1')
            pos = state(session)['player']['position']
            peak = max(peak, pos[1])
            if pos[2] > 23:
                break
        session.call('key', 'w', 'up')
        checks.check('parkour jumps at the edge', peak > start + 0.5, {'start': start, 'peak': peak})
        session.call('wait', '20')
        held = keys(session)
        checks.check('no jump key left held after parkour', 'Space' not in held, sorted(held))
        toggle(session, 'world.parkour')

        # --- AutoArmor equips, then a no-op click in a full inventory recovers.
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait', '5')
        session.call('give', 'totem_of_undying', '1')
        session.call('give', 'leather_helmet', '1')
        toggle(session, 'inventory.auto-armor')
        ok, _ = poll(session, lambda s: 'leather_helmet' in json.dumps(inventory(session).get('armor')), 80)
        checks.check('AutoArmor equips a helmet', ok, inventory(session).get('armor'))
        poll(session, lambda s: s['screen'] == 'playing', 60)
        toggle(session, 'inventory.auto-armor')
        # 36 slots minus the totem leave 35: fill 34 with cobblestone and the
        # last with a better helmet, so removing the equipped one has no room.
        for count in (256,) * 8 + (128,):
            session.call('give', 'cobblestone', str(count))
        session.call('give', 'iron_helmet', '1')
        # The integrated server applies give; wait for the client copy.
        poll(session, lambda s: inventory(session)['counts'].get('minecraft:iron_helmet') == 1, 40)
        inv = inventory(session)
        free = [s for s in inv['main'] + inv['hotbar'] if not s.get('item')]
        checks.check('inventory is full with a better helmet inside',
                     not free and inv['counts'].get('minecraft:iron_helmet') == 1,
                     {'free': len(free), 'counts': inv['counts']})
        toggle(session, 'inventory.auto-armor')
        # It opens the inventory, tries to remove the leather helmet, cannot,
        # and must close again within the gate deadline instead of hanging.
        opened, _ = poll(session, lambda s: s['screen'] != 'playing', 40, 1)
        closed, s = poll(session, lambda s: s['screen'] == 'playing', 80, 2)
        checks.check('AutoArmor no-op closes its inventory', opened and closed, s['screen'])
        reopened, _ = poll(session, lambda s: s['screen'] != 'playing', 40, 2)
        checks.check('AutoArmor does not reopen for the same no-op', not reopened)
        # The container claim is free: AutoTotem can act.
        toggle(session, 'inventory.auto-totem')
        ok, s = poll(session, lambda s: (s.get('offhand') or {}).get('item', '').endswith('totem_of_undying'), 100)
        checks.check('AutoTotem swaps after the AutoArmor no-op', ok, s.get('offhand'))
        ok, s = poll(session, lambda s: s['screen'] == 'playing', 40)
        checks.check('AutoTotem closes the inventory it opened', ok, s['screen'])
        # The swap freed a slot: content changed, so AutoArmor may retry and
        # now has room to replace the leather helmet with the iron one.
        ok, _ = poll(session, lambda s: 'iron_helmet' in json.dumps(inventory(session)['armor'])
                     and s['screen'] == 'playing', 120)
        checks.check('AutoArmor upgrades once the content changes', ok, inventory(session)['armor'])

        # --- Panic disables everything and restores on the next press.
        toggle(session, 'combat.sprint')
        session.call('key', 'f10', 'tap')
        session.call('wait', '2')
        mods = session.saved()['modules']
        on = sorted(m for m, v in mods.items() if v.get('enabled'))
        checks.check('panic disables every module', not on, on)
        session.call('key', 'w', 'down')
        session.call('wait', '6')
        held = keys(session)
        checks.check('no synthetic sprint after panic', 'ControlLeft' not in held, sorted(held))
        session.call('key', 'w', 'up')
        session.call('key', 'f10', 'tap')
        session.call('wait', '2')
        on = sorted(m for m, v in session.saved()['modules'].items() if v.get('enabled'))
        checks.check('second panic restores the modules', set(on) >= {'combat.sprint', 'inventory.auto-armor', 'inventory.auto-totem'}, on)

        # --- Death and respawn: nothing stays held, modules keep working.
        session.call('key', 'w', 'down')
        session.call('command', 'kill @s')
        ok, s = poll(session, lambda s: s['screen'] not in ('playing',), 40)
        checks.check('death screen shown', ok, s['screen'])
        held = keys(session)
        checks.check('no synthetic keys on the death screen', not ({'ControlLeft', 'Space'} & held), sorted(held))
        session.call('key', 'w', 'up')
        session.call('wait', '25')  # the Respawn button activates after a delay
        session.call('respawn')
        ok, s = poll(session, lambda s: s['screen'] == 'playing', 60)
        checks.check('respawned', ok, s['screen'])
        session.call('wait', '20')
        session.call('key', 'w', 'down')
        ok, s = poll(session, lambda s: s['player'].get('sprinting'), 30)
        checks.check('sprint works after respawn', ok, s['player'].get('sprinting'))
        session.call('key', 'w', 'up')
        session.capture('final')
        log = (session.root / 'game.log').read_text(errors='replace')
        checks.check('no panics or runtime poison in the log',
                     'panicked' not in log and 'poisoned' not in log)
        failed = [r['check'] for r in checks.results if not r['ok']]
        print(f'{len(checks.results) - len(failed)}/{len(checks.results)} checks passed', flush=True)
        return failed


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    raise SystemExit(1 if run(args.resources, args.allow_stale) else 0)
