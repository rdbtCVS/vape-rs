#!/usr/bin/env python3
"""Controlled LAN scenario: a fresh host world opened to LAN on localhost and
a fresh guest client running vape-rs over the real protocol.

Both processes use their own isolated homes. The guest exercises the remote
paths the single-player smoke cannot: remote movement ticks (Sprint), remote
container clicks whose state id the server may never bump (AutoTotem,
AutoArmor no-op recovery), and a disconnect/reconnect that reloads the
inventory from the server, so the offhand totem proves server acceptance.
"""
import argparse
import json
import time
from pathlib import Path

from native_session import NativeSession
from smoke_scenario import BINDS, Checks, enabled, inventory, poll, state, toggle


def run(resources, allow_stale):
    with NativeSession(resources, 'lan-host-', allow_stale=allow_stale) as host, \
            NativeSession(resources, 'lan-guest-', allow_stale=allow_stale) as guest:
        checks = Checks(guest.root)
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=False, world='LanHost')
        host.call('time', '6000')
        host.call('tp', '0.5', '100', '0.5')
        host.call('wait-ready', '40')
        host.call('fill', '-6', '96', '-6', '6', '99', '20', 'minecraft:stone')
        host.call('fill', '-6', '100', '-6', '6', '104', '20', 'minecraft:air')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        address = f'localhost:{port}'

        guest.seed({'schema': 1, 'modules': {},
                    'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()},
                                 'inventory.auto-armor.inventory-only': False}})
        guest.launch(creative=False, world=None,
                     extra=('--server', address, '--name', 'VapeGuest'))
        guest.call('wait-ready', '60')
        host.call('command', 'tp VapeGuest 2.5 100 0.5')
        guest.call('wait', '20')
        s = state(guest)
        checks.check('guest joined the LAN world', s['player']['health'] > 0, s['player']['position'])

        # Remote movement path.
        guest.call('look', '0', '0')
        toggle(guest, 'combat.sprint')
        guest.call('key', 'w', 'down')
        ok, s = poll(guest, lambda s: s['player'].get('sprinting'), 30)
        checks.check('guest sprints through the remote movement tick', ok, s.get('keys'))
        guest.call('key', 'w', 'up')
        toggle(guest, 'combat.sprint')
        ok, s = poll(guest, lambda s: 'ControlLeft' not in s['keys'], 10)
        checks.check('guest Sprint releases its key', ok, s['keys'])

        # Remote container clicks: equip, then a no-op in a full inventory.
        host.call('command', 'give VapeGuest minecraft:totem_of_undying 1')
        host.call('command', 'give VapeGuest minecraft:leather_helmet 1')
        poll(guest, lambda s: inventory(guest)['counts'].get('minecraft:leather_helmet'), 40)
        toggle(guest, 'inventory.auto-armor')
        ok, _ = poll(guest, lambda s: 'leather_helmet' in json.dumps(inventory(guest)['armor'])
                     and s['screen'] == 'playing', 100)
        checks.check('guest AutoArmor equips over the network', ok, inventory(guest)['armor'])
        toggle(guest, 'inventory.auto-armor')
        for count in (256,) * 8 + (128,):
            host.call('command', f'give VapeGuest minecraft:cobblestone {count}')
        host.call('command', 'give VapeGuest minecraft:iron_helmet 1')
        ok, _ = poll(guest, lambda s: inventory(guest)['counts'].get('minecraft:iron_helmet') == 1, 60)
        checks.check('guest inventory is full with a better helmet', ok, inventory(guest)['counts'])
        toggle(guest, 'inventory.auto-armor')
        opened, _ = poll(guest, lambda s: s['screen'] != 'playing', 40, 1)
        closed, s = poll(guest, lambda s: s['screen'] == 'playing', 80)
        checks.check('guest AutoArmor no-op closes its inventory', opened and closed, s['screen'])
        reopened, _ = poll(guest, lambda s: s['screen'] != 'playing', 40)
        checks.check('guest AutoArmor does not reopen for the same no-op', not reopened)
        toggle(guest, 'inventory.auto-totem')
        ok, s = poll(guest, lambda s: (s.get('offhand') or {}).get('item', '').endswith('totem_of_undying'), 100)
        checks.check('guest AutoTotem swaps after the no-op', ok, s.get('offhand'))
        ok, _ = poll(guest, lambda s: 'iron_helmet' in json.dumps(inventory(guest)['armor'])
                     and s['screen'] == 'playing', 120)
        checks.check('guest AutoArmor upgrades once room appears', ok, inventory(guest)['armor'])

        # Server acceptance: reconnect and read the server's copy.
        guest.call('wait', '20')
        guest.call('disconnect')
        time.sleep(1)  # ticks stop on the title screen
        held = set(guest.call('status').get('keys', []))
        checks.check('no synthetic keys after disconnect', not ({'ControlLeft', 'Space'} & held), sorted(held))
        guest.call('connect', address)
        guest.call('wait-ready', '60')
        guest.call('wait', '20')
        s = state(guest)
        inv = inventory(guest)
        checks.check('server kept the offhand totem after reconnect',
                     (s.get('offhand') or {}).get('item', '').endswith('totem_of_undying'), s.get('offhand'))
        checks.check('server kept the iron helmet after reconnect',
                     'iron_helmet' in json.dumps(inv['armor']), inv['armor'])
        checks.check('modules persist across the reconnect',
                     enabled(guest, 'inventory.auto-totem') and enabled(guest, 'inventory.auto-armor'))
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
