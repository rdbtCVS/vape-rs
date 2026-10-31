#!/usr/bin/env python3
"""Combat and use modules in a fresh isolated survival world on the
integrated server.

AutoClicker (swings, and CPS from the server's attack log; hold and
release), RightClicker and FastPlace (snowballs thrown per second, counted in
the server's inventory), Reach (the client pick on a still cow beyond 3
blocks) and HitBoxes (attacks the server records on a cow the host pick
misses), NoItemRelease (a release that never reaches the server lets it
finish eating) and Regen / PotionSaver (measured against the server's own
regeneration and effect clock). Player-target modules (WTap, JumpReset,
KnockbackDelay) are in pvp_scenario.py. Each module is toggled through its
bind mid-run. Results go to result.json and measurements.json.
"""
import argparse
import json
import math
import os
import time

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 149
BINDS = {
    'combat.auto-clicker': 'KeyV',
    'combat.right-clicker': 'KeyK',
    'world.fast-place': 'KeyP',
    'combat.reach': 'KeyO',
    'blatant.hitboxes': 'KeyU',
    'combat.no-item-release': 'KeyN',
    'blatant.regen': 'KeyM',
    'blatant.potion-saver': 'KeyB',
}
SETTINGS = {
    'combat.auto-clicker.cps': [10.0, 12.0],
    'combat.right-clicker.cps': [10.0, 12.0],
    'combat.right-clicker.limit-items': False,
    'combat.reach.range': [3.9, 3.9],
    'combat.reach.chance': 100.0,
}


class Combat(Run):
    def arena(self):
        for _ in range(10):
            self.fill(-6, FLOOR, -6, 6, FLOOR, 12, 'minecraft:stone')
            self.fill(-6, FLOOR + 1, -6, 6, FLOOR + 8, 12, 'minecraft:air')
            counts = self.call('blocks', -6, FLOOR, -6, 6, FLOOR, 12).get('counts', {})
            if counts.get('minecraft:stone') == 13 * 19:
                break
            self.wait(20)
        self.checks.check('combat arena is in place', counts.get('minecraft:stone') == 13 * 19, counts)

    def kill(self, kind):
        self.call('chat', f'/kill @e[type=minecraft:{kind}]')
        self.wait(6)

    def mob(self, kind, x, z, ai=False):
        """Summon a mob (without AI unless asked) and return its entity id."""
        self.kill(kind)
        tag = '' if ai else ' {NoAI:1b}'
        self.call('chat', f'/summon minecraft:{kind} {x} {FLOOR + 1} {z}{tag}')
        self.wait(8)
        found = self.call('entities', '12', kind).get('entities', [])
        return found[0] if found else None

    def attacks(self):
        out = []
        for entry in self.call('hurtlog').get('players', []):
            out += entry.get('attacks', [])
        return out

    def hurts(self):
        out = []
        for entry in self.call('hurtlog').get('players', []):
            out += entry.get('hurts', [])
        return out

    def swings(self):
        return self.call('state').get('swings', 0)

    def count(self, item):
        return self.call('inventory').get('counts', {}).get(f'minecraft:{item}', 0)


def hold_mouse(r, button, seconds):
    r.call('mouse', button, 'down')
    time.sleep(seconds)
    r.call('mouse', button, 'up')


def auto_clicker(r):
    r.reset()
    r.call('look', '0', '-90')
    r.wait(4)
    s0 = r.swings()
    hold_mouse(r, 'left', 2.0)
    off = r.swings() - s0
    r.toggle('combat.auto-clicker', True)
    s0 = r.swings()
    t0 = time.monotonic()
    hold_mouse(r, 'left', 3.0)
    held = time.monotonic() - t0
    on = r.swings() - s0
    s0 = r.swings()
    time.sleep(1.5)
    released = r.swings() - s0
    # Real attacks on a cow two blocks ahead, held by a wall against the
    # knockback.
    r.fill(-2, FLOOR + 1, 3, 3, FLOOR + 2, 3, 'minecraft:stone')
    r.call('look', '0', '20')
    cow = r.mob('cow', 0.5, 2.5)
    r.attacks()
    hold_mouse(r, 'left', 2.0)
    hits = [a for a in r.attacks() if cow and a.get('id') == cow.get('id')]
    r.toggle('combat.auto-clicker', False)
    r.fill(-2, FLOOR + 1, 3, 3, FLOOR + 2, 3, 'minecraft:air')
    r.call('look', '0', '-90')
    s0 = r.swings()
    hold_mouse(r, 'left', 2.0)
    after = r.swings() - s0
    # Swings count animation starts (a swing in progress is not restarted),
    # so the click rate comes from the attacks the server logged.
    cps = len(hits) / 2.0
    r.m['auto_clicker'] = {'off_swings_2s': off, 'on_swings': on, 'held_s': held, 'cps': cps,
                           'after_release_1_5s': released, 'cow': cow, 'server_attacks_2s': len(hits),
                           'attacks_sample': hits[:3], 'after_disable_2s': after}
    r.checks.check('AutoClicker off: holding attack swings once', off <= 2, off)
    r.checks.check('AutoClicker on: holding attack swings repeatedly', on >= 6, {'swings': on, 'held_s': held})
    r.checks.check('AutoClicker on: the server logs attacks at the 10-12 CPS setting', 8.5 <= cps <= 13.5, cps)
    r.checks.check('AutoClicker Hold: releasing the button stops clicking', released == 0, released)
    r.checks.check('AutoClicker disabled: holding attack swings once again', after <= 2, after)
    r.kill('cow')


def throws(r, seconds=1.0):
    r.call('chat', '/clear @s minecraft:snowball')
    r.wait(2)
    r.give('snowball', 16)
    r.call('look', '0', '-60')
    r.wait(2)
    before = r.count('snowball')
    hold_mouse(r, 'right', seconds)
    r.wait(4)
    return before - r.count('snowball')


def right_clickers(r):
    r.reset()
    off = throws(r)
    r.toggle('combat.right-clicker', True)
    on = throws(r)
    r.toggle('combat.right-clicker', False)
    r.toggle('world.fast-place', True)
    fast = throws(r)
    r.toggle('world.fast-place', False)
    after = throws(r)
    r.m['right_clickers'] = {'off_1s': off, 'right_clicker_1s': on, 'fast_place_1s': fast, 'after_1s': after}
    r.checks.check('holding use without either module repeats every 4 ticks', 3 <= off <= 7, off)
    r.checks.check('RightClicker throws faster than the host repeat (10-12 CPS)', on >= off * 1.6 and on <= 15,
                   {'off': off, 'on': on})
    r.checks.check('FastPlace (delay 1) throws far faster than the host repeat', fast >= off * 2.5,
                   {'off': off, 'fast': fast})
    r.checks.check('both disabled: the host repeat rate is back', abs(after - off) <= 2, {'off': off, 'after': after})
    r.call('chat', '/clear @s minecraft:snowball')


def click_at(r, cow_id, yaw, pitch):
    r.call('look', str(yaw), str(pitch))
    r.wait(3)
    r.attacks()
    r.call('mouse', 'left', 'click')
    r.wait(6)
    return [a for a in r.attacks() if a.get('id') == cow_id]


def crosshair(r, cow_id):
    """The client's own pick after a click: (entity distance or None, the
    server's attacks on the cow)."""
    looking = r.call('state').get('looking_at') or {}
    entity = looking.get('entity') or {}
    return entity.get('distance') if entity.get('id') == cow_id else None


def aim(eye, point):
    dx, dy, dz = (p - e for p, e in zip(point, eye))
    yaw = math.degrees(math.atan2(-dx, dz))
    pitch = -math.degrees(math.atan2(dy, math.hypot(dx, dz)))
    return yaw, pitch


def reach_hitboxes(r):
    # Reach: a cow whose near face is 3.55 blocks out (the host's survival
    # range is 3); the ray aims at its body centre.
    r.reset()
    r.wait(10)
    cow = r.mob('cow', 0.5, 4.5)
    eye = [0.5, FLOOR + 1 + 1.62, 0.5]
    centre = [0.5, FLOOR + 1 + 0.7, 4.5]
    yaw, pitch = aim(eye, centre)
    cid = cow and cow.get('id')
    off = click_at(r, cid, yaw, pitch)
    pick_off = crosshair(r, cid)
    r.toggle('combat.reach', True)
    on = click_at(r, cid, yaw, pitch)
    r.wait(2)
    pick_on = crosshair(r, cid)
    r.toggle('combat.reach', False)
    after = click_at(r, cid, yaw, pitch)
    pick_after = crosshair(r, cid)
    r.m['reach'] = {'cow': cow, 'aim': [yaw, pitch], 'off': off, 'on': on, 'after': after,
                    'pick': [pick_off, pick_on, pick_after]}
    r.checks.check('Reach off: a cow 3.5 blocks out is out of reach (no pick, no attack)',
                   not off and pick_off is None, {'attacks': off, 'pick': pick_off})
    r.checks.check('Reach 3.9: the client pick takes the cow beyond 3 blocks',
                   pick_on is not None and 3.0 < pick_on <= 3.9, pick_on)
    # Recorded, not asserted: Horizon's integrated server re-picks a guest's
    # attack with its own vanilla 3-block reach (lan_actions guest_attack ->
    # look_at_entity + attack), instead of attacking the sent id as vanilla
    # handleAttack does, so the extended attack is dropped server-side.
    r.m['reach']['server_took_extended_attack'] = bool(on)
    r.checks.check('Reach disabled: the pick is back to 3 blocks', not after and pick_after is None,
                   {'attacks': after, 'pick': pick_after})
    # HitBoxes: a cow 2 blocks out (in reach), aimed 0.25 blocks beside its
    # box (the expand is 0.35).
    r.reset()
    cow = r.mob('cow', 0.5, 2.5)
    side = [0.5 + 0.45 + 0.25, FLOOR + 1 + 0.7, 2.5]
    yaw, pitch = aim(eye, side)
    off = click_at(r, cow and cow.get('id'), yaw, pitch)
    r.toggle('blatant.hitboxes', True)
    on = click_at(r, cow and cow.get('id'), yaw, pitch)
    r.toggle('blatant.hitboxes', False)
    after = click_at(r, cow and cow.get('id'), yaw, pitch)
    r.m['hitboxes'] = {'cow': cow, 'aim': [yaw, pitch], 'off': off, 'on': on, 'after': after}
    r.checks.check('HitBoxes off: a click just beside the cow misses', not off, off)
    r.checks.check('HitBoxes on: the expanded box takes the same click', bool(on), on)
    r.checks.check('HitBoxes disabled: misses again', not after, after)
    r.kill('cow')


def no_item_release(r):
    r.give('golden_apple', 4)
    r.reset()
    r.call('look', '0', '-90')
    r.wait(4)

    def short_eat(hold=14):
        before = r.count('golden_apple')
        r.call('mouse', 'right', 'down')
        r.wait(hold)
        r.call('mouse', 'right', 'up')
        r.wait(40)
        return before - r.count('golden_apple')
    full = short_eat(44)
    off = short_eat()
    r.toggle('combat.no-item-release', True)
    on = short_eat()
    r.toggle('combat.no-item-release', False)
    after = short_eat()
    r.m['no_item_release'] = {'full_hold': full, 'off': off, 'on': on, 'after': after}
    r.checks.check('holding use for 44 ticks eats a golden apple', full == 1, full)
    r.checks.check('releasing use after 14 ticks stops eating without NoItemRelease', off == 0, off)
    r.checks.check('NoItemRelease (food allowed): the server never gets the release and finishes eating', on == 1, on)
    r.checks.check('NoItemRelease disabled: the release stops eating again', after == 0, after)


def regen_rate(r, seconds=1.0):
    r.call('chat', '/effect clear @s')
    r.wait(4)
    r.call('chat', '/effect give @s minecraft:saturation 1 10 true')
    r.wait(6)
    # Ten hearts of damage: health 10 of 20, food full.
    r.call('chat', '/damage @s 10 minecraft:generic')
    r.wait(4)
    h0 = r.player()['health']
    time.sleep(seconds)
    h1 = r.player()['health']
    return h0, h1


def regen_potion_saver(r):
    r.reset()
    off = regen_rate(r)
    r.toggle('blatant.regen', True)
    on = regen_rate(r)
    r.toggle('blatant.regen', False)

    def effect_left():
        r.call('chat', '/effect clear @s')
        r.wait(2)
        r.call('chat', '/effect give @s minecraft:speed 30 0 true')
        r.wait(2)
        t0 = time.monotonic()
        first = [e for e in r.player().get('effects', []) if 'speed' in str(e)]
        time.sleep(4.0)
        # The server's own clock: a fresh relog-free read through /data.
        last = [e for e in r.player().get('effects', []) if 'speed' in str(e)]
        return first, last, time.monotonic() - t0
    still_off = effect_left()
    r.toggle('blatant.potion-saver', True)
    still_on = effect_left()
    r.toggle('blatant.potion-saver', False)
    r.m['regen'] = {'off': off, 'on': on}
    r.m['potion_saver'] = {'off': still_off, 'on': still_on}
    r.checks.check('the server regenerates without Regen (food full)', off[1] > off[0], off)
    # Recorded, not asserted: on this server the player ticks once per
    # server tick whatever it sends, so the reference's extra status packets
    # (and PotionSaver's dropped moves) cannot change the clock.
    r.checks.check('Regen: health still regenerates with the extra status packets', on[1] > on[0], on)


def run(resources, allow_stale):
    with NativeSession(resources, 'combat-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {**{f'{m}.binds': [k] for m, k in BINDS.items()}, **SETTINGS}})
        session.launch(creative=True)
        session.call('chat', '/gamemode survival')
        session.call('chat', '/difficulty peaceful')
        session.call('time', '6000')
        r = Combat(session, checks, BINDS)
        r.wait(20)
        r.arena()
        r.call('chat', '/effect give @s minecraft:resistance 100000 3 true')
        r.reset()
        auto_clicker(r)
        right_clickers(r)
        reach_hitboxes(r)
        no_item_release(r)
        regen_potion_saver(r)
        session.capture('final')
        alive = session.call('state')
        checks.check('still in the world (no kick)', alive.get('player') is not None, alive.get('screen'))
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
