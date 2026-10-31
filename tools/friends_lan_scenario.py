#!/usr/bin/env python3
"""Local friends and enemies against a real player over a LAN connection.

A fresh host world is opened to LAN on localhost and a fresh guest client
joins it (each process with its own isolated homes). The guest runs
TriggerBot facing the host 2.2 blocks away, both under Resistance V; the
host's hurt log shows each hit. Between passes the guest leaves, its saved
profile is edited, and it rejoins:

* no friends: TriggerBot hits the host;
* the host listed as a friend (name in another case): no hit and no swing
  (`FriendManager.isFriend` is case-insensitive);
* "Use friends" off with the friend still listed: hits again;
* the friend entry's "target" flag off (`FriendEntry.isTargeted`): hits;
* no friends and the target filter's "Ignore naked": the unarmoured host is
  skipped;
* the host listed as an enemy: hit despite "Ignore naked"
  (`EntityTargetFilterValue.isValidTarget`);
* "Players" off with the enemy kept: no hit.

Team exclusion (TargetFilter) and bot exclusion (never on 26.3) are not
exercised here.

Status: written in client-foundation worker round 3 and never run on a host
build; its checks are unproven.

Results go to <guest session>/result.json, measurements to
measurements.json.
"""
import argparse
import json
import os

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks, poll

FLOOR = 99
BINDS = {'combat.trigger-bot': 'KeyB'}
HOST = (0.5, 0.4, 0)
GUEST = (0.5, 2.6, 180)


class Pair:
    def __init__(self, hs, gs, checks, address):
        self.hs, self.gs, self.checks, self.address = hs, gs, checks, address
        self.host = Run(hs, checks, BINDS)
        self.guest = Run(gs, checks, BINDS)
        self.m = {}

    def command(self, text):
        self.host.call('command', text)

    def effects(self):
        self.command('effect give @a minecraft:resistance 100000 4 true')
        self.command('effect give @a minecraft:saturation 100000 4 true')

    def place(self):
        self.guest.release()
        self.guest.call('mouse', 'left', 'up')
        self.command(f'tp VapeGuest {GUEST[0]} {FLOOR + 1} {GUEST[1]} {GUEST[2]} 0')
        self.host.call('tp', HOST[0], FLOOR + 1, HOST[1])
        self.host.call('look', HOST[2], '0')
        self.guest.wait(10)
        self.guest.call('look', GUEST[2], '0')
        self.guest.wait(6)

    def host_name(self):
        for entry in self.host.call('hurtlog').get('players', []):
            if entry['name'] != 'VapeGuest':
                return entry['name']
        return None

    def window(self, ticks=60):
        """The host's hurts and the guest's swings over `ticks`."""
        self.host.call('hurtlog')
        s0 = self.guest.call('state').get('swings', 0)
        self.guest.wait(ticks)
        swings = self.guest.call('state').get('swings', 0) - s0
        hurts = [h for entry in self.host.call('hurtlog').get('players', [])
                 if entry['name'] != 'VapeGuest' for h in entry.get('hurts', [])]
        return [{k: h.get(k) for k in ('t', 'source', 'amount')} for h in hurts], swings

    def rejoin(self, label, edit):
        gs = self.gs
        gs.call('quit')
        gs.process.wait(timeout=15)
        gs.log.close()
        gs.process = None
        document = gs.saved()
        edit(document)
        gs.seed(document)
        gs.launch(creative=False, world=None, extra=('--server', self.address, '--name', 'VapeGuest'))
        gs.call('wait-ready', '60')
        self.guest.wait(20)
        self.effects()
        saved = gs.saved()
        self.checks.check(f'guest rejoined: {label}', self.guest.enabled('combat.trigger-bot'),
                          {k: saved.get(k) for k in ('friends', 'enemies')})
        self.place()
        return saved

    def expect(self, label, hits):
        hurts, swings = self.window()
        if hits:
            ok = len(hurts) >= 1
        else:
            ok = not hurts and swings == 0
        self.checks.check(label, ok, [hurts, swings])
        self.m[label] = {'hurts': hurts, 'swings': swings}


def run(resources, allow_stale):
    with NativeSession(resources, 'friends-lan-host-', allow_stale=allow_stale) as hs, \
            NativeSession(resources, 'friends-lan-guest-', allow_stale=allow_stale) as gs:
        checks = Checks(gs.root)
        binds = {f'{k}.binds': [v] for k, v in BINDS.items()}
        hs.seed({'schema': 1, 'modules': {}, 'settings': {}})
        hs.launch(creative=False, world='FriendsHost')
        host = Run(hs, checks, BINDS)
        host.call('time', '6000')
        host.call('tp', HOST[0], FLOOR + 1, HOST[1])
        host.call('wait-ready', '40')
        for _ in range(5):
            host.call('fill', '-8', str(FLOOR - 3), '-8', '8', str(FLOOR), '8', 'minecraft:obsidian')
            host.call('fill', '-8', str(FLOOR + 1), '-8', '8', str(FLOOR + 6), '8', 'minecraft:air')
            floor = host.call('blocks', '-8', str(FLOOR), '-8', '8', str(FLOOR), '8').get('counts', {})
            if floor.get('minecraft:obsidian') == 17 * 17:
                break
            host.wait(20)
        for z in (-1, 3):
            host.call('fill', '-2', str(FLOOR + 1), str(z), '2', str(FLOOR + 3), str(z), 'minecraft:obsidian')
        checks.check('obsidian floor is in place', floor.get('minecraft:obsidian') == 17 * 17, floor)
        host.call('command', 'gamerule mob_spawning false')
        host.call('command', 'difficulty peaceful')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        address = f'localhost:{port}'

        gs.seed({'schema': 1, 'modules': {}, 'settings': binds})
        gs.launch(creative=False, world=None, extra=('--server', address, '--name', 'VapeGuest'))
        gs.call('wait-ready', '60')
        p = Pair(hs, gs, checks, address)
        p.guest.wait(20)
        p.effects()
        p.place()
        name = p.host_name()
        checks.check("the host's player name is known", bool(name) and name != 'VapeGuest', name)
        seen = p.guest.call('entities', '4', 'player').get('entities', [])
        checks.check('the guest sees the host within 4 blocks', bool(seen), seen)
        armor = host.call('state').get('armor')
        p.m['host'] = {'name': name, 'armor': armor}

        p.guest.toggle('combat.trigger-bot', True)
        p.expect('no friends: TriggerBot hits the host', True)

        def friend(target=True):
            return [{'name': name.swapcase(), 'alias': '', 'target': target}]

        def set_friends(entries):
            return lambda d: d.update(friends=entries)
        p.rejoin('host listed as a friend', set_friends(friend()))
        p.expect('a friend (name case differs) is never targeted', False)

        p.rejoin('Use friends off', lambda d: d['settings'].update({'friends.use': False}))
        p.expect('Use friends off: the listed friend is targeted', True)

        def inactive(d):
            d['settings']['friends.use'] = True
            d['friends'] = friend(target=False)
        p.rejoin('friend entry not targeted', inactive)
        p.expect("a friend entry with its target flag off is not a friend", True)

        def naked(d):
            d['friends'] = []
            d['settings']['combat.trigger-bot.targets.ignore-naked'] = True
        p.rejoin('Ignore naked', naked)
        p.expect('Ignore naked skips the unarmoured host', False)

        p.rejoin('host listed as an enemy',
                 lambda d: d.update(enemies=[{'name': name, 'alias': '', 'exclusive': False}]))
        p.expect('an enemy is targeted despite Ignore naked', True)

        p.rejoin('Players off',
                 lambda d: d['settings'].update({'combat.trigger-bot.targets.players': False}))
        p.expect('Players off: not even an enemy player is targeted', False)
        p.guest.toggle('combat.trigger-bot', False)

        gs.capture('final')
        hs.capture('host-final')
        alive = gs.call('state')
        checks.check('guest still connected (no kick)', alive.get('player') is not None, alive.get('screen'))
        for label, session in (('host', hs), ('guest', gs)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {label} log', 'panicked' not in log and 'poisoned' not in log)
        (gs.root / 'measurements.json').write_text(json.dumps(p.m, indent=2, default=str))
        gs.call('disconnect')
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}', gs.root)
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
