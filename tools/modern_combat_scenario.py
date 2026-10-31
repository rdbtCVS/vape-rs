#!/usr/bin/env python3
"""HitSwap and AutoMace in a fresh single-player world.

A stationary (NoAI) ravager (a monster with 100 health, armourless) stands
on a stone floor. The server gives the hotbar: a diamond sword (slot 1,
selected), a Density V mace (2), a Breach IV mace (3) and an iron axe (4).
Each case teleports the player 8 blocks above the ravager looking straight
down and lets it fall onto it. The server's attack log gives the damage (a
falling sword crit 10.5, a Density V smash about 52, a plain or Breach IV
smash about 34 on the armourless ravager) and the durability used tells
which slot made the hit:

* nothing on, one click in reach: the sword hits (baseline);
* AutoMace with Attack (Manual, Density), no click: it swaps to the Density
  mace, smashes and swaps back to the sword;
* HitSwap, one click in reach: the press is swallowed, the mace is
  selected, HitSwap's own attack smashes and the sword comes back;
* HitSwap under TriggerBot (Mobs), no click: TriggerBot's synthetic attack
  goes through HitSwap the same way;
* AutoMace Manual Breach, and Auto (which estimates Density higher on it),
  after relaunches with those settings (the hotbar is given again);
* AutoMace off: a click in reach hits with the sword again.

Results go to result.json, measurements to measurements.json.
"""
import argparse
import json
import os
import time

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 99
GOLEM = (0.5, FLOOR + 1, 0.5)
BINDS = {'combat.auto-mace': 'KeyM', 'combat.hit-swap': 'KeyH', 'combat.trigger-bot': 'KeyB',
         'combat.shield-breaker': 'KeyN'}
SETTINGS = {f'{m}.binds': [k] for m, k in BINDS.items()}
SETTINGS.update({'combat.auto-mace.targets.mobs': True, 'combat.auto-mace.attack': True,
                 'combat.trigger-bot.targets.mobs': True})
SWORD = 10.5
DENSITY = 42.0


def golem(session):
    near = session.call('mobs', '24').get('near', [])
    return next((m for m in near if m.get('kind') == 'minecraft:ravager'), None)


def health(session):
    g = golem(session)
    return g['health'] if g else None


def attacks(run):
    out = []
    for p in run.call('hurtlog').get('players', []):
        out += p.get('attacks', [])
    return out


def ensure_golem(run, checks):
    g = golem(run.s)
    if g and g['health'] >= 60:
        return True
    if g:
        run.call('command', 'kill @e[type=minecraft:ravager]')
        run.wait(20)
    run.call('command', f'summon minecraft:ravager {GOLEM[0]} {GOLEM[1]} {GOLEM[2]} '
             '{NoAI:1b,PersistenceRequired:1b}')
    for _ in range(10):
        run.wait(10)
        if golem(run.s):
            return True
    checks.check('a stationary ravager stands on the floor', False)
    return False


def fall(run, name, click=False):
    """Teleport above the golem, fall onto it (clicking once in reach when
    asked), step off after landing and return what happened."""
    run.release()
    run.call('command', f'tp @s 3.5 {FLOOR + 1} 0.5 0 0')
    run.wait(20)
    attacks(run)
    worn = wear(run)
    before = health(run.s)
    run.call('command', f'tp @s {GOLEM[0]} {FLOOR + 9} {GOLEM[2]} 0 90')
    run.wait(2)
    run.call('look', '0', '90')
    samples, clicked, shot = [], None, False
    t0 = time.monotonic()
    while time.monotonic() - t0 < 5:
        st = run.call('status')
        p = st.get('player') or {}
        target = run.call('state').get('looking_at')
        golem_seen = 'ravager' in json.dumps(target)
        samples.append({'ms': round((time.monotonic() - t0) * 1000), 'y': round(p['position'][1], 3),
                        'selected': st.get('selected'), 'held': st.get('held'), 'golem': golem_seen})
        if click and clicked is None and golem_seen:
            run.call('mouse', 'left', 'click')
            clicked = samples[-1]['ms']
        if not shot and st.get('selected') != 1:
            run.s.capture(f'{name}-swapped')
            shot = True
        if p.get('on_ground') and p['position'][1] < FLOOR + 1.01 and len(samples) > 3:
            break
    run.call('command', f'tp @s 3.5 {FLOOR + 1} 0.5 0 0')
    run.wait(20)
    st = run.call('status')
    after = health(run.s)
    every = attacks(run)
    hits = [a for a in every if a.get('landed', True)]
    used = [b - a for a, b in zip(worn, wear(run))]
    return {'before': before, 'after': after, 'clicked_ms': clicked, 'attacks': hits,
            'all_attacks': every, 'wear': used, 'worn_slots': [i + 1 for i, d in enumerate(used) if d],
            'first': max((a.get('amount', 0) for a in hits[:1]), default=0),
            'selected_after': st.get('selected'), 'held_after': st.get('held'),
            'selected_seen': sorted({s['selected'] for s in samples}), 'samples': samples}


HOTBAR = ('minecraft:diamond_sword',
          'minecraft:mace[minecraft:enchantments={"minecraft:density":5}]',
          'minecraft:mace[minecraft:enchantments={"minecraft:breach":4}]',
          'minecraft:iron_axe')


def stock(run, checks):
    """Clear the inventory and fill the hotbar through the server (the ctl
    `enchant` only changes this client's copy): sword, Density V mace,
    Breach IV mace, iron axe; the sword ends up selected."""
    run.call('command', 'clear @s')
    run.wait(10)
    for item in HOTBAR:
        run.call('command', f'give @s {item} 1')
        run.wait(6)
    for _ in range(20):
        bar = run.call('inventory').get('hotbar', [])[:4]
        text = [json.dumps(h) for h in bar]
        ok = (len(bar) == 4 and 'diamond_sword' in text[0] and 'density' in text[1]
              and 'breach' in text[2] and 'iron_axe' in text[3])
        if ok:
            break
        run.wait(10)
    checks.check('hotbar: sword, Density V mace, Breach IV mace, iron axe (server-side)', ok, bar)
    run.call('select', '1')
    return bar


def wear(run):
    """The durability used on hotbar slots 1 to 4."""
    return [h.get('damage', 0) for h in run.call('inventory').get('hotbar', [])[:4]]


def restart(run, changes, checks, enable=()):
    s = run.s
    s.call('quit')
    s.process.wait(timeout=15)
    s.log.close()
    s.process = None
    document = s.saved()
    document['settings'].update(changes)
    s.seed(document)
    s.launch(creative=False, world='ModernCombat')
    s.call('wait-ready', '40')
    run.wait(20)
    # The player's inventory does not survive the relaunch.
    run.call('effect', 'resistance', '100000', '4')
    run.call('effect', 'saturation', '100000', '0')
    stock(run, checks)
    for module in enable:
        run.toggle(module, True)


def run_cases(resources, allow_stale):
    with NativeSession(resources, 'modern-combat-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        run = Run(session, checks, BINDS)
        m = {}
        session.seed({'schema': 1, 'modules': {}, 'settings': dict(SETTINGS)})
        session.launch(creative=False, world='ModernCombat')
        session.call('command', 'gamerule spawn_mobs false')
        session.call('time', '1000')
        session.call('wait-ready', '40')
        run.call('effect', 'resistance', '100000', '4')
        run.call('effect', 'saturation', '100000', '0')
        for _ in range(10):
            run.fill(-4, FLOOR, -4, 6, FLOOR, 4, 'minecraft:stone')
            run.fill(-4, FLOOR + 1, -4, 6, FLOOR + 12, 4, 'minecraft:air')
            counts = run.call('blocks', -4, FLOOR, -4, 6, FLOOR, 4).get('counts', {})
            if counts.get('minecraft:stone') == 11 * 9:
                break
            run.wait(20)
        checks.check('floor is in place', counts.get('minecraft:stone') == 11 * 9, counts)
        run.call('command', f'tp @s 3.5 {FLOOR + 1} 0.5 0 0')
        run.wait(20)
        checks.check('a stationary ravager stands on the floor', ensure_golem(run, checks),
                     golem(session))
        m['inventory'] = stock(run, checks)
        r = m['baseline'] = fall(run, 'baseline', click=True)
        checks.check('nothing on: the click lands a sword hit',
                     0 < r['first'] < SWORD + 5 and r['worn_slots'] == [1],
                     {k: r[k] for k in ('first', 'before', 'after', 'clicked_ms', 'worn_slots')})

        run.toggle('combat.auto-mace', True)
        r = m['auto_mace_density'] = fall(run, 'auto-mace-density')
        checks.check('AutoMace (Attack, Manual Density) smashes with the Density mace unclicked',
                     r['first'] >= DENSITY and r['worn_slots'] == [2],
                     {k: r[k] for k in ('first', 'before', 'after', 'selected_seen', 'worn_slots')})
        checks.check('AutoMace swaps back to the sword after its smash',
                     r['selected_after'] == 1 and 2 in r['selected_seen'],
                     {k: r[k] for k in ('selected_after', 'selected_seen')})
        run.toggle('combat.auto-mace', False)
        ensure_golem(run, checks)

        run.toggle('combat.hit-swap', True)
        r = m['hit_swap_click'] = fall(run, 'hit-swap-click', click=True)
        checks.check('HitSwap turns a smashing sword click into a Density mace smash',
                     r['first'] >= DENSITY and r['worn_slots'] == [2],
                     {k: r[k] for k in ('first', 'clicked_ms', 'before', 'after', 'selected_seen',
                                        'worn_slots')})
        checks.check('HitSwap swaps back to the sword', r['selected_after'] == 1, r['selected_seen'])
        ensure_golem(run, checks)

        run.toggle('combat.trigger-bot', True)
        r = m['hit_swap_trigger_bot'] = fall(run, 'hit-swap-trigger-bot')
        run.toggle('combat.trigger-bot', False)
        checks.check("TriggerBot's synthetic attack goes through HitSwap's mace swap",
                     r['first'] >= DENSITY and r['worn_slots'] == [2],
                     {k: r[k] for k in ('first', 'before', 'after', 'selected_seen', 'worn_slots')})
        checks.check('HitSwap swaps back after the TriggerBot smash', r['selected_after'] == 1,
                     r['selected_seen'])
        run.toggle('combat.hit-swap', False)
        ensure_golem(run, checks)

        restart(run, {'combat.auto-mace.mace-type': 'Breach'}, checks, enable=['combat.auto-mace'])
        r = m['auto_mace_breach'] = fall(run, 'auto-mace-breach')
        density = m['auto_mace_density']['first']
        checks.check('AutoMace Manual Breach smashes with the Breach mace',
                     SWORD + 5 < r['first'] < density - 8 and r['worn_slots'] == [3],
                     {k: r[k] for k in ('first', 'selected_seen', 'worn_slots')}
                     | {'density_first': density})
        ensure_golem(run, checks)

        restart(run, {'combat.auto-mace.mace-selection': 'Auto'}, checks, enable=['combat.auto-mace'])
        r = m['auto_mace_auto'] = fall(run, 'auto-mace-auto')
        checks.check('AutoMace Auto picks the higher estimate (Density on an armourless ravager)',
                     r['first'] >= DENSITY and r['worn_slots'] == [2],
                     {k: r[k] for k in ('first', 'selected_seen', 'worn_slots')})
        run.toggle('combat.auto-mace', False)
        ensure_golem(run, checks)
        r = m['disabled'] = fall(run, 'disabled', click=True)
        checks.check('AutoMace off: the click hits with the sword again',
                     0 < r['first'] < SWORD + 5 and r['selected_seen'] == [1]
                     and r['worn_slots'] == [1],
                     {k: r[k] for k in ('first', 'selected_seen', 'worn_slots')})
        session.capture('final')
        (session.root / 'measurements.json').write_text(json.dumps(m, indent=2) + '\n')
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?', default=os.environ.get('HORIZON_RESOURCES'),
                        help='prepared Minecraft 26.3 resources (or set HORIZON_RESOURCES)')
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run_cases(args.resources, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
