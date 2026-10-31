#!/usr/bin/env python3
"""AutoTool "Not during combat" against a real player over LAN: two owned
local processes, the host (VapeHost) serving with commands and a guest
mining obsidian with AutoTool. A hit holds the swap while the host stays
within 10 blocks of a moving guest, or watches a stationary guest within 6;
it lifts once the host leaves, and at once for a friend."""
import argparse
import json
from pathlib import Path
from native_session import NativeSession

WALL = ('-3', '100', '3', '3', '102', '3')


def selected(guest):
    return guest.call('state')['selected']


def strafe(guest, ticks):
    """Alternate A/D so the guest keeps moving; returns the selections seen."""
    seen = []
    for n in range(0, ticks, 5):
        key = 'a' if n // 5 % 2 == 0 else 'd'
        guest.call('key', key, 'down')
        guest.call('wait', '5')
        guest.call('key', key, 'up')
        seen.append(selected(guest))
    return seen


def join(host, guest, address, name, friends=()):
    guest.seed({'schema': 1, 'modules': {'world.auto-tool': {'enabled': True}},
                'friends': [{'name': f, 'target': True} for f in friends],
                'settings': {'world.auto-tool.binds': ['KeyJ'],
                             'world.auto-tool.swap-to-delay': 0,
                             'world.auto-tool.not-combat': True}})
    guest.launch(creative=False, world=None, extra=('--server', address, '--name', name))
    guest.call('wait-ready', '60')
    host.call('command', f'tp {name} 0.5 100 0.5')
    host.call('command', f'clear {name}')
    host.call('command', f'give {name} minecraft:stick 1')
    host.call('command', f'give {name} minecraft:diamond_pickaxe 1')
    guest.call('wait', '40')  # join, teleport and fall guards expire
    guest.call('hotbar', '1')
    guest.call('look-at', 'block', '0', '101', '3')
    names = [e for e in guest.call('entities', '16')['entities'] if e.get('type') == 'minecraft:player']
    assert names, 'guest sees the host player'
    return names


def hit(host, name):
    host.call('command', f'damage {name} 1 minecraft:generic')


def run(resources):
    checks = {}
    with NativeSession(resources, 'lan-guard-host-') as host:
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=False, world='LanGuard', extra=('--name', 'VapeHost'))
        host.call('time', '6000')
        host.call('tp', '3.5', '100', '0.5')
        host.call('wait-ready', '40')
        host.call('fill', '-6', '96', '-6', '6', '99', '20', 'minecraft:stone')
        host.call('fill', '-6', '100', '-6', '6', '104', '20', 'minecraft:air')
        host.call('fill', *WALL, 'minecraft:obsidian')
        host.call('tp', '3.5', '100', '0.5')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        assert port, opened
        address = f'localhost:{port}'

        with NativeSession(resources, 'lan-guard-guest-') as guest:
            checks['guest_root'] = str(guest.root)
            players = join(host, guest, address, 'VapeGuest')
            checks['players_seen'] = players
            # Stationary, host 1.8 blocks away watching the guest, and the
            # guest's view within 60 degrees of the host: every 20-tick
            # expiry re-arms until five stationary expiries have passed.
            host.call('tp', '1.5', '100', '2.0')
            host.call('wait', '5')
            host.call('look-at', '0.5', '101.6', '0.5')
            guest.call('wait', '10')
            hit(host, 'VapeGuest')
            guest.call('mouse', 'left', 'down')
            guest.call('wait', '45')
            assert selected(guest) == 1, 'mutual looking holds past the first expiry'
            guest.capture('stationary-watched')
            for waited in range(45, 200, 5):
                if selected(guest) == 2:
                    break
                guest.call('wait', '5')
            assert selected(guest) == 2
            checks['stationary_watched_release_ticks'] = waited
            guest.call('mouse', 'left', 'up')
            guest.call('hotbar', '1')
            guest.call('wait', '5')

            # Moving with the host 3 blocks away: held past the 20 ticks.
            host.call('tp', '3.5', '100', '0.5')
            guest.call('wait', '10')
            hit(host, 'VapeGuest')
            guest.call('mouse', 'left', 'down')
            held = strafe(guest, 60)
            assert set(held) == {1}, held
            guest.capture('moving-enemy-near')
            # The host leaves (16 blocks): released on the next guard tick.
            host.call('tp', '0.5', '100', '16.5')
            released = strafe(guest, 20)
            assert released[-1] == 2, released
            checks['moving'] = {'held': held, 'after_host_left': released}
            guest.call('mouse', 'left', 'up')
            guest.capture('moving-enemy-left')
            guest.call('disconnect')
            guest.close()

        with NativeSession(resources, 'lan-guard-friend-') as guest:
            checks['friend_root'] = str(guest.root)
            host.call('tp', '3.5', '100', '0.5')
            join(host, guest, address, 'VapeFriend', friends=['VapeHost'])
            hit(host, 'VapeFriend')
            guest.call('mouse', 'left', 'down')
            early = strafe(guest, 10)
            late = strafe(guest, 30)
            assert early[0] == 1, early
            assert late[-1] == 2, late
            checks['friend'] = {'first_ticks': early, 'later': late}
            guest.call('mouse', 'left', 'up')
            guest.capture('friend-not-a-threat')
            guest.call('disconnect')
            guest.close()
        host.close()
        out = {'passed': True, 'checks': checks}
        (host.root / 'assertions.json').write_text(json.dumps(out, indent=2) + '\n')
        print(json.dumps(checks))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    run(parser.parse_args().resources)
