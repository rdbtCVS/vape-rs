#!/usr/bin/env python3
"""Combat part A host scenarios: HitSelect in single player and TargetFilter
over a real LAN connection, each process with its own isolated homes.

``sp``: a stationary (NoAI) cow two blocks ahead; the player clicks it once
per tick for a burst. The server's attack log counts the attacks that reached
it and the client's swing counter the swings:

* HitSelect off: every click attacks.
* "Right-click use cancel" (Chance 100): clicks while the cow is still
  invulnerable (hurt time > 2, the player on the ground) are dropped, swing
  and all, so far fewer attacks reach the server and they keep landing.
* "Normal" (Chance 100): only the sprint-reset-timed clicks pass; the others
  are replaced by the local swing (``applyAttackEffects``), so the swings
  stay with the clicks while the attacks drop.

``lan``: the guest runs KillAura with TargetFilter against the host player.

* "Teams by server": no attacks while both share a scoreboard team (same
  prefix), attacks with different prefixes, attacks again with TargetFilter
  disabled.
* "Teams by color" with "Recolor visuals": a host team of the guest's colour
  (another team name and prefix) is a teammate, the detected colour shows in
  the setting, and with a blue host team the host is attacked and its ESP box
  is painted blue (screenshots for inspection).

Results go to <session>/result.json and measurements.json.
"""
import argparse
import json
import time
from pathlib import Path

from movement_scenario import Run
from native_session import NativeSession
from smoke_scenario import Checks

FLOOR = 99
HS = 'combat.hit-select'
TF = 'utility.target-filter'
KA = 'combat.kill-aura'
ESP = 'render.esp'
BINDS = {HS: 'KeyH', TF: 'KeyG', KA: 'KeyK', ESP: 'KeyJ'}


def binds(*modules):
    return {f'{m}.binds': [BINDS[m]] for m in modules}


def arena(r):
    for _ in range(10):
        r.call('fill', -4, FLOOR, -4, 4, FLOOR, 6, 'minecraft:stone')
        r.call('fill', -4, FLOOR + 1, -4, 4, FLOOR + 5, 6, 'minecraft:air')
        counts = r.call('blocks', -4, FLOOR, -4, 4, FLOOR, 6).get('counts', {})
        if counts.get('minecraft:stone') == 9 * 11:
            return True
        r.wait(20)
    return False


def cow(r):
    near = r.call('mobs', '16').get('near', [])
    return next((m for m in near if m.get('kind') == 'minecraft:cow'), None)


def server_attacks(r, mob_id):
    out = []
    for entry in r.call('hurtlog').get('players', []):
        out += [a for a in entry.get('attacks', []) if a.get('id') == mob_id]
    return out


def burst(r, mob_id, clicks=40):
    """One click per tick; the attacks the server logged and the swings."""
    r.call('tp', 0.5, FLOOR + 1, 0.5)
    r.call('look-at-entity', mob_id)
    r.wait(12)
    server_attacks(r, mob_id)
    swings = r.call('state').get('swings', 0)
    for _ in range(clicks):
        r.call('mouse', 'left', 'click')
        r.wait(1)
    r.wait(6)
    attacks = server_attacks(r, mob_id)
    return {'clicks': clicks, 'attacks': len(attacks),
            'landed': sum(1 for a in attacks if a.get('landed')),
            'swings': r.call('state').get('swings', 0) - swings}


def sp_session(resources, allow_stale, mode, checks_out):
    with NativeSession(resources, f'combat-a-sp-{mode[:6].lower()}-', allow_stale=allow_stale) as s:
        checks = Checks(s.root)
        settings = {**binds(HS), f'{HS}.mode': mode, f'{HS}.chance': 100.0}
        s.seed({'schema': 1, 'modules': {}, 'settings': settings})
        s.launch(creative=False, world='HitSelect')
        r = Run(s, checks, BINDS)
        r.call('command', 'gamerule spawn_mobs false')
        r.call('command', 'difficulty peaceful')
        r.call('time', '6000')
        r.call('tp', 0.5, FLOOR + 1, 0.5)
        r.call('wait-ready', '40')
        checks.check('floor is in place', arena(r))
        r.call('command', f'summon minecraft:cow 0.5 {FLOOR + 1} 2.5 '
               '{NoAI:1b,PersistenceRequired:1b,Health:1000f,'
               'attributes:[{id:"minecraft:max_health",base:1000}]}')
        found = None
        for _ in range(20):
            r.wait(10)
            found = cow(r)
            if found:
                break
        checks.check('a stationary cow stands two blocks ahead', found is not None, found)
        if not found:
            return checks
        m = {'mode': mode}
        m['off'] = burst(r, found['id'])
        r.toggle(HS, True)
        m['on'] = burst(r, found['id'])
        r.toggle(HS, False)
        m['after'] = burst(r, found['id'])
        off, on, after = m['off'], m['on'], m['after']
        checks.check('HitSelect off: every click attacks the cow',
                     off['attacks'] >= 0.85 * off['clicks'], off)
        if mode == 'Normal':
            checks.check('Normal: most clicks inside the invulnerability are cancelled',
                         on['attacks'] <= 0.6 * off['attacks'] and on['attacks'] >= 2, m)
            # A swing in progress is not restarted: the counter counts starts.
            checks.check('Normal: a cancelled click still swings (applyAttackEffects)',
                         on['swings'] >= off['swings'] - 1, m)
        else:
            checks.check('Right-click use cancel: clicks on the invulnerable cow are dropped',
                         on['attacks'] <= 0.5 * off['attacks'] and on['landed'] >= 2, m)
            checks.check('Right-click use cancel: a dropped click does not swing',
                         on['swings'] < off['swings'] and on['swings'] <= on['attacks'] + 2, m)
        checks.check('HitSelect disabled: every click attacks again',
                     after['attacks'] >= 0.85 * after['clicks'], after)
        log = (s.root / 'game.log').read_text(errors='replace')
        checks.check('no panics in the log', 'panicked' not in log and 'poisoned' not in log)
        (s.root / 'measurements.json').write_text(json.dumps(m, indent=2))
        checks_out.append((s.root, checks))
        return checks


def guest_attacks(host):
    """The guest's attacks on the host player: its hurt records (player
    attacks are not in the attack log, which holds attacks on mobs)."""
    out = 0
    for entry in host.call('hurtlog').get('players', []):
        if entry.get('name') != 'VapeGuest':
            out += sum(1 for h in entry.get('hurts', []) if h.get('source') == 'minecraft:player_attack')
    return out


def attacks_over(host, guest, ticks=60):
    place(host, guest)
    guest_attacks(host)
    guest.wait(ticks)
    return guest_attacks(host)


def place(host, guest):
    host.call('tp', 0.5, FLOOR + 1, 2.5)
    host.call('look', 180, 0)
    host.call('command', f'tp VapeGuest 0.5 {FLOOR + 1} 0.5 0 0')
    guest.wait(20)
    guest.call('look', 0, 0)
    guest.wait(4)


def host_name(host):
    names = [e.get('name') for e in host.call('hurtlog').get('players', [])]
    return next((n for n in names if n and n != 'VapeGuest'), None)


def join_host(host, team):
    out = host.call('command', f'team join {team} {host.name}')
    host.wait(10)
    return out


def join_guest(gs, address, settings):
    gs.seed({'schema': 1, 'modules': {}, 'settings': settings})
    gs.launch(creative=False, world=None, extra=('--server', address, '--name', 'VapeGuest'))
    gs.call('wait-ready', '60')


def lan(resources, allow_stale, checks_out):
    with NativeSession(resources, 'combat-a-lan-host-', allow_stale=allow_stale) as hs, \
            NativeSession(resources, 'combat-a-lan-guest-', allow_stale=allow_stale) as gs:
        checks = Checks(gs.root)
        m = {}
        hs.seed({'schema': 1, 'modules': {}, 'settings': {}})
        hs.launch(creative=False, world='TeamsHost')
        host = Run(hs, checks, {})
        host.call('time', '6000')
        host.call('tp', 0.5, FLOOR + 1, 0.5)
        host.call('wait-ready', '40')
        checks.check('floor is in place', arena(host))
        host.call('command', 'gamerule spawn_mobs false')
        host.call('command', 'difficulty peaceful')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)
        address = f'localhost:{port}'

        base = {**binds(TF, KA, ESP), f'{TF}.teams-by-server': True}
        join_guest(gs, address, base)
        guest = Run(gs, checks, BINDS)
        host.call('command', 'effect give @a minecraft:resistance 100000 4 true')
        host.call('command', 'effect give @a minecraft:saturation 100000 4 true')
        host.name = host_name(host)
        # KillAura's knockback must not push the host out of its range.
        kb = host.call('command', f'attribute {host.name} minecraft:knockback_resistance base set 1')
        checks.check('the host resists knockback', 'knockback' in json.dumps(kb).lower(), kb)
        place(host, guest)
        others = guest.call('entities', '12', 'player').get('entities', [])
        checks.check('guest joined and sees the host player', bool(others), others)
        for cmd in ('team add alpha', 'team modify alpha prefix "[A] "', 'team add beta',
                    'team modify beta prefix "[B] "', 'team join alpha VapeGuest'):
            host.call('command', cmd)
        join_host(host, 'alpha')

        # --- Teams by server ----------------------------------------------
        guest.toggle(TF, True)
        guest.toggle(KA, True)
        guest.wait(10)
        same = attacks_over(host, guest)
        join_host(host, 'beta')
        guest.wait(10)
        other = attacks_over(host, guest)
        join_host(host, 'alpha')
        guest.wait(10)
        guest.toggle(TF, False)
        unfiltered = attacks_over(host, guest)
        guest.toggle(KA, False)
        m['teams_by_server'] = {'same_team': same, 'other_prefix': other, 'filter_off': unfiltered}
        checks.check('Teams by server: no attack on a host in the same team', same == 0, m['teams_by_server'])
        checks.check('Teams by server: a host with another team prefix is attacked', other >= 2,
                     m['teams_by_server'])
        checks.check('TargetFilter disabled: the teammate is attacked again', unfiltered >= 2,
                     m['teams_by_server'])

        # --- Teams by color ---------------------------------------------------
        gs.call('disconnect')
        gs.close()
        for cmd in ('team add gamma', 'team modify gamma prefix "[G] "', 'team modify alpha color red',
                    'team modify gamma color red', 'team add delta', 'team modify delta prefix "[D] "',
                    'team modify delta color blue'):
            host.call('command', cmd)
        join_host(host, 'gamma')
        join_guest(gs, address, {**binds(TF, KA, ESP), f'{TF}.teams-by-color': True,
                                 f'{TF}.recolor-visuals': True})
        place(host, guest)
        guest.toggle(TF, True)
        guest.toggle(ESP, True)
        guest.wait(10)
        detected = gs.saved()['settings'].get(f'{TF}.detected-color')
        gs.capture('teams-by-color-red-teammate-esp')
        guest.toggle(KA, True)
        guest.wait(10)
        red = attacks_over(host, guest)
        join_host(host, 'delta')
        guest.wait(10)
        blue = attacks_over(host, guest)
        guest.toggle(KA, False)
        place(host, guest)
        gs.capture('teams-by-color-blue-enemy-esp')
        m['teams_by_color'] = {'detected': detected, 'same_color': red, 'other_color': blue}
        checks.check('Teams by color: the guest shows its detected red team colour',
                     detected == '#FF5555', detected)
        checks.check('Teams by color: a red host in another team is a teammate (no attack)', red == 0,
                     m['teams_by_color'])
        checks.check('Teams by color: a blue host is attacked', blue >= 2, m['teams_by_color'])

        for name, session in (('host', hs), ('guest', gs)):
            log = (session.root / 'game.log').read_text(errors='replace')
            checks.check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
        (gs.root / 'measurements.json').write_text(json.dumps(m, indent=2, default=str))
        gs.call('disconnect')
        checks_out.append((gs.root, checks))
        return checks


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('part', choices=('sp', 'lan', 'all'))
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    results = []
    if args.part in ('sp', 'all'):
        sp_session(args.resources, args.allow_stale, 'Right-click use cancel', results)
        sp_session(args.resources, args.allow_stale, 'Normal', results)
    if args.part in ('lan', 'all'):
        lan(args.resources, args.allow_stale, results)
    failed = []
    for root, checks in results:
        bad = [r['check'] for r in checks.results if not r['ok']]
        print(f'{len(checks.results) - len(bad)}/{len(checks.results)} checks passed', root, flush=True)
        failed += bad
    for f in failed:
        print('FAILED:', f, flush=True)
    raise SystemExit(1 if failed or not results else 0)


if __name__ == '__main__':
    main()
