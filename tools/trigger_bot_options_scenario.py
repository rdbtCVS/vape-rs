#!/usr/bin/env python3
"""TriggerBot options in a fresh isolated survival world on the integrated
server: Target miss chance, Early hit chance, Limit to items and Select first
hit.

A NoAI, Invulnerable zombie (held by a wall) stands two blocks ahead; the
player holds a stone sword (a 12.5-tick cooldown, longer than a swing). The
server's attack log gives every attack's tick and attack-strength scale, and
the client's swing counter gives attacks on nothing. Each option set runs in
its own relaunch of the same home:

- defaults: hits come at full strength, and a physical click on the zombie
  followed by a look at the sky leaves no air attack;
- miss 100 / early 100: the same click leaves one air attack once the
  cooldown is ready, and the hits that follow come early (scale under 1);
- Limit to items (swords) with Select first hit: no hit until the player is
  hurt, then hits; none with dirt in hand; again none after the view leaves
  the zombie for over a second.

Results go to result.json and measurements.json.
"""
import argparse
import json
import os

from clicker_modes_scenario import target
from combat_scenario import FLOOR, Combat
from movement_scenario import relaunch
from native_session import NativeSession
from smoke_scenario import Checks

BINDS = {'combat.trigger-bot': 'KeyB'}
BASE = {'combat.trigger-bot.targets.mobs': True}


class Trigger(Combat):
    def setup(self):
        self.reset()
        self.fill(-2, FLOOR + 1, 3, 3, FLOOR + 3, 3, 'minecraft:stone')
        self.zombie = target(self, 'zombie', 0.5, 2.5)
        self.give('stone_sword')
        self.hover()
        self.toggle('combat.trigger-bot', True)

    def hover(self):
        self.call('look', '0', '10')
        self.wait(4)

    def sky(self):
        self.call('look', '180', '-60')

    def hits(self, ticks):
        """Attacks on the zombie the server logs over `ticks`."""
        self.attacks()
        self.wait(ticks)
        zid = self.zombie and self.zombie.get('id')
        return [a for a in self.attacks() if a.get('id') == zid]

    def click_then_sky(self):
        """A physical click on the zombie, then the view on the sky: the
        swings that start over the next 30 ticks (attacks on nothing)."""
        self.hover()
        self.wait(14)
        self.call('mouse', 'left', 'click')
        # The click's own attack lands on the zombie before the view moves.
        self.wait(2)
        self.sky()
        s0 = self.swings()
        self.wait(30)
        return self.swings() - s0


def scales(hits):
    return [round(a.get('scale', 0), 3) for a in hits]


def run(resources, allow_stale):
    with NativeSession(resources, 'trigger-options-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()}, **BASE}})
        session.launch(creative=True)
        session.call('chat', '/gamemode survival')
        session.call('chat', '/gamerule spawn_mobs false')
        session.call('time', '18000')
        r = Trigger(session, checks, BINDS)
        r.wait(20)
        r.arena()

        # Defaults.
        r.setup()
        looking = (r.call('state').get('looking_at') or {}).get('entity') or {}
        checks.check('the crosshair is on the zombie', r.zombie and looking.get('id') == r.zombie.get('id'), looking)
        base = r.hits(60)
        air = r.click_then_sky()
        r.m['defaults'] = {'zombie': r.zombie, 'scales': scales(base), 'ticks': [a['t'] for a in base],
                           'air_swings': air}
        checks.check('defaults: TriggerBot hits the zombie as the cooldown recharges', len(base) >= 3, len(base))
        checks.check('defaults: every hit is at full strength (no early hit)',
                     base and min(scales(base)) >= 0.95, scales(base))
        checks.check('defaults (miss 0): no air attack after the click', air == 0, air)

        # Target miss chance 100, Early hit chance 100.
        relaunch(session, {'combat.trigger-bot.miss-chance': 100.0, 'combat.trigger-bot.early-chance': 100.0})
        r.setup()
        air = r.click_then_sky()
        r.hover()
        early = r.hits(60)
        again = r.click_then_sky()
        r.m['miss_early'] = {'air_swings': air, 'scales': scales(early), 'ticks': [a['t'] for a in early],
                             'air_swings_again': again}
        checks.check('miss 100: the click on the zombie leaves one air attack', air == 1, air)
        checks.check('miss 100: a second click leaves another', again == 1, again)
        checks.check('early 100: TriggerBot hits before the cooldown is full',
                     len(early) >= 3 and max(scales(early)) < 0.95, scales(early))

        # Limit to items (swords) and Select first hit.
        relaunch(session, {'combat.trigger-bot.miss-chance': 0.0, 'combat.trigger-bot.early-chance': 0.0,
                           'combat.trigger-bot.limit-items': True,
                           'combat.trigger-bot.allowed-items': ['swords'],
                           'combat.trigger-bot.select-first-hit': True})
        r.setup()
        waiting = r.hits(40)
        r.attacks()
        r.call('chat', '/damage @s 1 minecraft:generic')
        hurt = r.hits(40)
        r.give('dirt', 1)
        r.hover()
        dirt = r.hits(40)
        r.call('select', 'stone_sword')
        r.hover()
        resumed = r.hits(40)
        r.sky()
        r.wait(30)
        r.hover()
        reset = r.hits(40)
        r.heal()
        r.m['limit_select'] = {'waiting': len(waiting), 'after_hurt': len(hurt), 'dirt': len(dirt),
                               'sword_again': len(resumed), 'after_look_away': len(reset)}
        checks.check('Select first hit: no hit before the player is hurt', not waiting, len(waiting))
        checks.check('Select first hit: hits once the player was hurt', len(hurt) >= 1, len(hurt))
        checks.check('Limit to items: no hit with dirt in hand', not dirt, len(dirt))
        checks.check('Limit to items: hits again with the sword', len(resumed) >= 1, len(resumed))
        checks.check('Select first hit: a second away resets the wait', not reset, len(reset))

        r.toggle('combat.trigger-bot', False)
        session.capture('final')
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
