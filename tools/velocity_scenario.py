#!/usr/bin/env python3
"""Velocity host scenario in a fresh isolated survival world.

The player (Resistance V, so no damage) stands on an obsidian floor inside a
walled arena. Knockback comes from two real server paths:

* a zombie's melee hit: the server's ClientboundSetEntityMotion for the
  local player (Velocity's entity-motion path);
* primed TNT: this server sends the player's explosion push as an own
  ClientboundSetEntityMotion too, so it also exercises the motion path (the
  separate Explode-packet push hook is not reached here).

Each is measured with Velocity off and on (Chance 100, Horizontal 0,
Vertical 0, Ticks 0) as the player's largest horizontal displacement, and the
delayed Ticks mode is checked to still reduce the zombie knockback. Results
go to result.json.
"""
import argparse
import os
import json
import math

from native_session import NativeSession
from smoke_scenario import Checks

BIND = 'KeyV'
FULL = {'combat.velocity.chance': 100.0, 'combat.velocity.horizontal': 0.0,
        'combat.velocity.vertical': 0.0, 'combat.velocity.ticks': 0.0}


def enabled(session):
    return session.saved()['modules'].get('combat.velocity', {}).get('enabled', False)


def toggle(session):
    session.call('key', BIND[3:].lower(), 'tap')
    session.call('wait', '2')


def position(session):
    return session.call('state')['player']['position']


def arena(session, checks):
    for _ in range(10):
        session.call('fill', '-8', '99', '-8', '8', '99', '8', 'minecraft:obsidian')
        session.call('fill', '-8', '100', '-8', '8', '106', '8', 'minecraft:air')
        counts = session.call('blocks', '-8', '99', '-8', '8', '99', '8').get('counts', {})
        if counts.get('minecraft:obsidian') == 17 * 17:
            break
        session.call('wait', '20')
    checks.check('obsidian arena is in place', counts.get('minecraft:obsidian') == 17 * 17, counts)


def kill_zombies(session):
    session.call('chat', '/kill @e[type=minecraft:zombie]')
    session.call('wait', '10')


def displacement(session, ticks=40, step=2):
    start = position(session)
    best = 0.0
    for _ in range(0, ticks, step):
        session.call('wait', str(step))
        p = position(session)
        best = max(best, math.hypot(p[0] - start[0], p[2] - start[2]))
    return best


def zombie_hit(session):
    """Stand still facing a zombie until it hits once; return the largest
    horizontal displacement over the following ticks."""
    kill_zombies(session)
    session.call('tp', '0.5', '100', '0.5')
    session.call('look', '0', '0')
    session.call('wait', '6')
    session.call('chat', '/summon minecraft:zombie 0.5 100 2.0')
    before = session.call('state')['player']['health']
    for _ in range(60):
        session.call('wait', '2')
        s = session.call('state')['player']
        if s.get('hurt_time', 0) > 0:
            break
    hurt = s.get('hurt_time', 0) > 0
    moved = displacement(session, 20)
    kill_zombies(session)
    return hurt, moved, before


def tnt_push(session):
    session.call('tp', '0.5', '100', '0.5')
    session.call('look', '0', '0')
    session.call('wait', '6')
    # The default 80-tick fuse (this server ignores a summoned fuse tag).
    session.call('chat', '/summon minecraft:tnt 0.5 100 2.0')
    session.call('wait', '70')
    return displacement(session, 40)


def run(resources, allow_stale):
    with NativeSession(resources, 'velocity-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {'combat.velocity.binds': [BIND], **FULL}})
        # A creative world has commands; the test itself runs in survival.
        session.launch(creative=True)
        session.call('chat', '/gamemode survival')
        session.call('chat', '/difficulty normal')
        session.call('time', '6000')
        session.call('tp', '0.5', '100', '0.5')
        session.call('wait-ready', '40')
        arena(session, checks)
        session.call('chat', '/effect give @s minecraft:resistance 100000 4 true')
        session.call('chat', '/effect give @s minecraft:regeneration 100000 4 true')
        session.call('wait', '10')

        hurt, off_hit, _ = zombie_hit(session)
        checks.check('a zombie hit knocks the player back without Velocity', hurt and off_hit > 0.6,
                     {'hurt': hurt, 'moved': off_hit})
        off_tnt = tnt_push(session)
        checks.check('TNT pushes the player without Velocity', off_tnt > 1.0, off_tnt)

        toggle(session)
        checks.check('Velocity enabled through its bind', enabled(session))
        hurt, on_hit, _ = zombie_hit(session)
        checks.check('Velocity cancels the zombie knockback', hurt and on_hit < off_hit * 0.5,
                     {'off': off_hit, 'on': on_hit, 'hurt': hurt})
        on_tnt = tnt_push(session)
        checks.check('Velocity cancels the TNT push', on_tnt < off_tnt * 0.3,
                     {'off': off_tnt, 'on': on_tnt})

        toggle(session)
        checks.check('Velocity disabled through its bind', not enabled(session))
        again = tnt_push(session)
        checks.check('disabled Velocity restores the TNT push', again > 1.0, again)

        # The reference default path: keep the packet, scale the motion one
        # tick later.
        session.call('quit')
        session.process.wait(timeout=15)
        session.log.close()
        session.process = None
        document = session.saved()
        document['settings']['combat.velocity.ticks'] = 1.0
        document['modules']['combat.velocity'] = {**document['modules'].get('combat.velocity', {}),
                                                  'enabled': True}
        session.seed(document)
        session.launch(creative=True)
        session.call('wait-ready', '40')
        session.call('chat', '/gamemode survival')
        session.call('chat', '/effect give @s minecraft:resistance 100000 4 true')
        checks.check('Velocity restored enabled from the profile', enabled(session))
        hurt, delayed, _ = zombie_hit(session)
        checks.check('Ticks 1: delayed scaling still cuts the zombie knockback',
                     hurt and delayed < off_hit * 0.7, {'off': off_hit, 'delayed': delayed, 'hurt': hurt})
        session.capture('final')
        (session.root / 'measurements.json').write_text(json.dumps(
            {'zombie_off': off_hit, 'zombie_on': on_hit, 'zombie_delayed': delayed,
             'tnt_off': off_tnt, 'tnt_on': on_tnt, 'tnt_after_disable': again}, indent=2))
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?',
                        default=os.environ.get('HORIZON_RESOURCES'),
                        help='prepared Minecraft 26.3 resources (or set HORIZON_RESOURCES)')
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run(args.resources, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
