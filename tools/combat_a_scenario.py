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

``hs-kb``/``hs-crit``: HitSelect Normal's Preference after a real knockback
(a ``/damage`` from a cow behind the player pushes it up and toward the cow
ahead, a velocity packet). Clicks while rising: KB reduction lets them all
through, Critical hits cancels them (falling clicks pass in both). Clicks on
the ground right after one another: only the first gets through in both.

``bh-sp``: BlockHit with a sword in the main hand and a shield in the off
hand (the modern block), against the stationary cow:

* Manual (Chance 100): every attack click raises the shield for about a tick
  (`using` seen), then lets it go; with BlockHit off clicks never raise it.
* Auto with AutoClicker: the attack and use buttons held on the cow; the use
  press is swallowed and every third click raises the shield with the
  attack, so the shield goes up and down while the attacks keep landing
  (with BlockHit off the held use keeps it raised).

``bh-lan``: BlockHit against a real player over LAN (Predict and Lag target
players only), the guest holding a sword, a shield and the use button:

* Predict: the host hits the guest every 12 ticks; the shield is lowered
  between the hits and raised ahead of them (with BlockHit off it stays up).
* Lag: the guest backs away from the host; each release-use packet holds the
  guest's packets for the delay, so the host sees the guest's movement in
  bursts (stalled ticks) and the shield cycles; with BlockHit off it moves
  every tick.

Results go to <session>/result.json and measurements.json.
"""
import argparse
import json
import math
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
BH = 'combat.block-hit'
AC = 'combat.auto-clicker'
BINDS = {HS: 'KeyH', TF: 'KeyG', KA: 'KeyK', ESP: 'KeyJ', BH: 'KeyB', AC: 'KeyV'}


def binds(*modules):
    return {f'{m}.binds': [BINDS[m]] for m in modules}


def arena(r):
    for _ in range(10):
        r.call('fill', -4, FLOOR, -4, 4, FLOOR, 6, 'minecraft:stone')
        r.call('fill', -4, FLOOR + 1, -4, 4, FLOOR + 5, 6, 'minecraft:air')
        counts = r.call('blocks', -4, FLOOR, -4, 4, FLOOR, 6).get('counts', {})
        if counts.get('minecraft:stone') == 9 * 11:
            # Under load the player can fall past the floor before it is
            # placed: put it back on top.
            pos = (r.call('state').get('player') or {}).get('position')
            if pos and pos[1] < FLOOR + 1 - 1e-3:
                r.call('tp', pos[0], FLOOR + 1, pos[2])
                r.wait(4)
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


def player(r):
    return r.call('state').get('player') or {}


def knocked(r, front_id, phase, limit=5):
    """One knockback, then a click per tick while `phase` holds (rising or
    falling): the clicks made and the attacks the server logged."""
    r.call('tp', 0.5, FLOOR + 1, 0.3)
    r.call('look-at-entity', front_id)
    r.wait(16)
    before = len(server_attacks(r, front_id))
    r.call('command', 'damage @s 1 minecraft:mob_attack by @e[tag=back,limit=1]')
    clicks, airborne, peak = 0, False, 0.0
    for _ in range(30):
        r.wait(1)
        p = player(r)
        vy, ground = p.get('velocity', [0, 0, 0])[1], p.get('on_ground', True)
        if not ground:
            airborne = True
            peak = max(peak, p.get('position', [0, 0, 0])[1] - (FLOOR + 1))
        if airborne and ground:
            break
        if not ground and clicks < limit and ((phase == 'rising') == (vy > 0)):
            r.call('look-at-entity', front_id)
            r.call('mouse', 'left', 'click')
            clicks += 1
    r.wait(8)
    return {'clicks': clicks, 'attacks': len(server_attacks(r, front_id)) - before,
            'airborne': airborne, 'peak': round(peak, 3)}


def grounded(r, front_id, clicks=5):
    """Clicks on consecutive ticks standing still, no knockback."""
    r.call('tp', 0.5, FLOOR + 1, 0.3)
    r.call('look-at-entity', front_id)
    r.wait(16)
    before = len(server_attacks(r, front_id))
    for _ in range(clicks):
        r.call('mouse', 'left', 'click')
        r.wait(1)
    r.wait(8)
    return {'clicks': clicks, 'attacks': len(server_attacks(r, front_id)) - before}


def total(rows):
    return {k: sum(row[k] for row in rows) for k in ('clicks', 'attacks')}


def hs_velocity(resources, allow_stale, preference, checks_out):
    tag = 'kb' if preference == 'KB reduction' else 'crit'
    with NativeSession(resources, f'combat-a-hs-{tag}-', allow_stale=allow_stale) as s:
        checks = Checks(s.root)
        settings = {**binds(HS), f'{HS}.mode': 'Normal', f'{HS}.chance': 100.0,
                    f'{HS}.preference': preference}
        s.seed({'schema': 1, 'modules': {}, 'settings': settings})
        s.launch(creative=False, world='HitSelectVelocity')
        r = Run(s, checks, BINDS)
        r.call('command', 'gamerule spawn_mobs false')
        r.call('command', 'difficulty easy')
        r.call('time', '6000')
        r.call('tp', 0.5, FLOOR + 1, 0.3)
        r.call('wait-ready', '40')
        checks.check('floor is in place', arena(r))
        r.call('command', 'effect give @s minecraft:regeneration 100000 4 true')
        r.call('command', 'effect give @s minecraft:saturation 100000 4 true')
        r.call('command', f'summon minecraft:cow 0.5 {FLOOR + 1} -1.0 '
               '{NoAI:1b,PersistenceRequired:1b,Tags:["back"]}')
        r.call('command', f'summon minecraft:cow 0.5 {FLOOR + 1} 1.6 '
               '{NoAI:1b,PersistenceRequired:1b,Health:1000f,'
               'attributes:[{id:"minecraft:max_health",base:1000}]}')
        front = None
        for _ in range(20):
            r.wait(10)
            near = r.call('mobs', '16').get('near', [])
            front = next((m for m in near if m.get('kind') == 'minecraft:cow'
                          and m.get('pos', [0, 0, 0])[2] > 1.0), None)
            if front and len([m for m in near if m.get('kind') == 'minecraft:cow']) == 2:
                break
        checks.check('a cow behind the player and one ahead', front is not None, front)
        if not front:
            return checks
        r.toggle(HS, True)
        m = {'preference': preference}
        m['grounded'] = [grounded(r, front['id']) for _ in range(4)]
        m['rising'] = [knocked(r, front['id'], 'rising') for _ in range(6)]
        m['falling'] = [knocked(r, front['id'], 'falling') for _ in range(6)]
        g, up, down = total(m['grounded']), total(m['rising']), total(m['falling'])
        m['totals'] = {'grounded': g, 'rising': up, 'falling': down}
        checks.check('the knockback lifts the player every time',
                     all(t['airborne'] for t in m['rising'] + m['falling']), m['totals'])
        checks.check('on the ground, clicks right after a hit are cancelled',
                     g['attacks'] <= 0.5 * g['clicks'] and g['attacks'] >= 1, m['totals'])
        checks.check('enough clicks while rising to compare', up['clicks'] >= 12, m['totals'])
        if tag == 'kb':
            checks.check('KB reduction: every click after the knockback goes through',
                         up['attacks'] >= up['clicks'] - 1, m['totals'])
        else:
            checks.check('Critical hits: clicks while rising after the knockback are cancelled',
                         up['attacks'] <= 1, m['totals'])
        checks.check('falling clicks after the knockback go through',
                     down['clicks'] >= 6 and down['attacks'] >= down['clicks'] - 1, m['totals'])
        log = (s.root / 'game.log').read_text(errors='replace')
        checks.check('no panics in the log', 'panicked' not in log and 'poisoned' not in log)
        (s.root / 'measurements.json').write_text(json.dumps(m, indent=2))
        checks_out.append((s.root, checks))
        return checks


def arm(r, target='@s'):
    """A sword in the main hand and a shield in the off hand."""
    r.call('command', f'item replace entity {target} weapon.mainhand with minecraft:diamond_sword')
    r.call('command', f'item replace entity {target} weapon.offhand with minecraft:shield')
    r.wait(10)


def using(r):
    return r.call('state').get('using') is not None


def samples(r, ticks):
    out = []
    for _ in range(ticks):
        r.wait(1)
        out.append(using(r))
    return out


def raises(seq):
    return sum(1 for a, b in zip(seq, seq[1:]) if b and not a)


def manual_clicks(r, mob_id, clicks=10):
    """Clicks 12 ticks apart: after how many the shield went up and down."""
    r.call('tp', 0.5, FLOOR + 1, 0.5)
    r.call('look-at-entity', mob_id)
    r.wait(12)
    raised = lowered = 0
    for _ in range(clicks):
        r.call('mouse', 'left', 'click')
        seen = samples(r, 6)
        raised += any(seen)
        lowered += any(seen) and not seen[-1]
        r.wait(6)
    return {'clicks': clicks, 'raised': raised, 'lowered': lowered}


def auto_hold(r, mob_id, ticks=60):
    """Attack and use held on the cow (attack first): the shield samples and
    the attacks the server logged."""
    r.call('tp', 0.5, FLOOR + 1, 0.5)
    r.call('look-at-entity', mob_id)
    r.wait(12)
    server_attacks(r, mob_id)
    r.call('mouse', 'left', 'down')
    r.wait(2)
    r.call('mouse', 'right', 'down')
    seen = samples(r, ticks)
    r.call('mouse', 'right', 'up')
    r.call('mouse', 'left', 'up')
    r.wait(6)
    return {'ticks': ticks, 'using': sum(seen), 'raises': raises(seen),
            'attacks': len(server_attacks(r, mob_id))}


def bh_sp(resources, allow_stale, mode, checks_out):
    with NativeSession(resources, f'combat-a-bh-{mode.lower()}-', allow_stale=allow_stale) as s:
        checks = Checks(s.root)
        settings = {**binds(BH, AC), f'{BH}.mode': mode, f'{BH}.chance': [100.0, 100.0]}
        if mode == 'Manual':
            # "Require mouse down" (default on) blocks only while the use
            # button is held, and a held use already raises the shield; the
            # gate itself is covered by the unit tests.
            settings[f'{BH}.require-mouse-down'] = False
        s.seed({'schema': 1, 'modules': {}, 'settings': settings})
        s.launch(creative=False, world='BlockHit')
        r = Run(s, checks, BINDS)
        r.call('command', 'gamerule spawn_mobs false')
        r.call('command', 'difficulty peaceful')
        r.call('time', '6000')
        r.call('tp', 0.5, FLOOR + 1, 0.5)
        r.call('wait-ready', '40')
        checks.check('floor is in place', arena(r))
        arm(r)
        st = r.call('state')
        checks.check('a shield in the off hand', 'shield' in json.dumps(st.get('offhand'))
                     or 'shield' in json.dumps(r.call('status').get('offhand')), st.get('offhand'))
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
        if mode == 'Manual':
            m['off'] = manual_clicks(r, found['id'])
            r.toggle(BH, True)
            m['on'] = manual_clicks(r, found['id'])
            r.toggle(BH, False)
            m['after'] = manual_clicks(r, found['id'])
            off, on, after = m['off'], m['on'], m['after']
            checks.check('BlockHit off: attack clicks never raise the shield', off['raised'] == 0, m)
            checks.check('Manual: an attack click raises the shield', on['raised'] >= 0.7 * on['clicks'], m)
            checks.check('Manual: the shield goes down again after the click', on['lowered'] >= on['raised'] - 1, m)
            checks.check('BlockHit disabled: clicks no longer raise it', after['raised'] == 0, m)
        else:
            r.toggle(AC, True)
            m['off'] = auto_hold(r, found['id'])
            r.toggle(BH, True)
            m['on'] = auto_hold(r, found['id'])
            r.toggle(BH, False)
            r.toggle(AC, False)
            off, on = m['off'], m['on']
            checks.check('BlockHit off: the held use keeps the shield raised',
                         off['using'] >= 0.8 * off['ticks'], m)
            checks.check('Auto: the swallowed use lowers the shield between block hits',
                         on['using'] <= 0.6 * on['ticks'], m)
            checks.check('Auto: the clicks raise the shield again and again', on['raises'] >= 3, m)
            checks.check('Auto: the attacks keep landing', on['attacks'] >= 3, m)
        log = (s.root / 'game.log').read_text(errors='replace')
        checks.check('no panics in the log', 'panicked' not in log and 'poisoned' not in log)
        (s.root / 'measurements.json').write_text(json.dumps(m, indent=2))
        checks_out.append((s.root, checks))
        return checks


def guest_position(host):
    for e in host.call('entities', '16', 'minecraft:player').get('entities', []):
        return e.get('pos')
    return None


def aim_at_guest(host):
    """The host looks at the guest's chest: an unblocked hit knocks the guest
    sideways off a fixed crosshair."""
    me = (host.call('state').get('player') or {}).get('position')
    them = guest_position(host)
    if not me or not them:
        return
    dx, dy, dz = them[0] - me[0], (them[1] + 1.0) - (me[1] + 1.62), them[2] - me[2]
    yaw = math.degrees(math.atan2(-dx, dz))
    pitch = -math.degrees(math.atan2(dy, math.hypot(dx, dz)))
    host.call('look', round(yaw, 3), round(pitch, 3))


def face_off(host, guest, distance=2.0):
    # A little off the z axis: BlockHit's `findTarget` uses the reference's
    # quadrant atan, which gives yaw 0 for an axis-aligned delta.
    host.call('tp', 0.7, FLOOR + 1, 0.5 + distance)
    host.call('look', 180, 0)
    host.call('command', f'tp VapeGuest 0.5 {FLOOR + 1} 0.5 0 0')
    guest.wait(20)
    guest.call('look', 0, 0)
    host.call('look', 180, 0)
    guest.wait(4)


def predict_hits(host, guest, hits=10, gap=12):
    """The host hits the guest every `gap` ticks while the guest holds attack,
    then use (the press the reference swallows while attacking with a sword).
    A wall behind the guest keeps the unblocked hits from knocking it out of
    reach (Horizon's /attribute does not take a LAN guest)."""
    host.call('fill', -1, FLOOR + 1, -1, 1, FLOOR + 2, -1, 'minecraft:stone')
    face_off(host, guest)
    guest.call('mouse', 'left', 'down')
    guest.wait(2)
    guest.call('mouse', 'right', 'down')
    guest.wait(10)
    seen = []
    for _ in range(hits):
        aim_at_guest(host)
        host.call('mouse', 'left', 'click')
        seen += samples(guest, gap // 2)
        # Unblocked hits push the guest along the wall and out of reach:
        # back to its spot, keeping its rotation.
        host.call('command', f'tp VapeGuest 0.5 {FLOOR + 1} 0.5')
        seen += samples(guest, gap - gap // 2)
    guest.call('mouse', 'right', 'up')
    guest.call('mouse', 'left', 'up')
    host.call('fill', -1, FLOOR + 1, -1, 1, FLOOR + 2, -1, 'minecraft:air')
    blocked = 0
    for entry in host.call('hurtlog').get('players', []):
        if entry.get('name') == 'VapeGuest':
            hurts = entry.get('hurts', [])[-hits:]
            blocked = sum(1 for h in hurts if (h.get('blocked') or 0) > 0)
    return {'samples': len(seen), 'using': sum(seen), 'raises': raises(seen), 'blocked': blocked}


def lag_backoff(host, guest, ticks=40):
    """The guest backs away holding use; the host's view of it per tick."""
    face_off(host, guest)
    guest.call('mouse', 'right', 'down')
    guest.wait(8)
    guest.call('key', 's', 'down')
    positions, seen = [], []
    for _ in range(ticks):
        host.call('wait', 1)
        positions.append(guest_position(host))
        seen.append(using(guest))
    guest.call('key', 's', 'up')
    guest.call('mouse', 'right', 'up')
    moved = [p for p in positions if p]
    stalls = sum(1 for a, b in zip(moved, moved[1:]) if abs(a[2] - b[2]) < 1e-6)
    return {'ticks': ticks, 'stalled': stalls, 'travel': (moved[0][2] - moved[-1][2]) if len(moved) > 1 else 0,
            'using': sum(seen), 'raises': raises(seen)}


def bh_lan(resources, allow_stale, mode, checks_out):
    with NativeSession(resources, f'combat-a-bhh-{mode.lower()}-', allow_stale=allow_stale) as hs, \
            NativeSession(resources, f'combat-a-bhg-{mode.lower()}-', allow_stale=allow_stale) as gs:
        checks = Checks(gs.root)
        hs.seed({'schema': 1, 'modules': {}, 'settings': {}})
        hs.launch(creative=False, world='BlockHitHost')
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
        join_guest(gs, f'localhost:{port}', {**binds(BH), f'{BH}.mode': mode})
        guest = Run(gs, checks, BINDS)
        host.call('command', 'effect give @a minecraft:resistance 100000 4 true')
        host.call('command', 'effect give @a minecraft:saturation 100000 4 true')
        host.call('command', 'effect give @a minecraft:regeneration 100000 4 true')
        # The guest's attack press must not knock the host out of reach.
        host.name = host_name(host)
        kb = host.call('command', f'attribute {host.name} minecraft:knockback_resistance base set 1')
        checks.check('the host resists knockback', 'knockback' in json.dumps(kb).lower(), kb)
        arm(host, 'VapeGuest')
        face_off(host, guest)
        others = guest.call('entities', '12', 'player').get('entities', [])
        checks.check('guest joined and sees the host player', bool(others), others)
        m = {'mode': mode}
        if mode == 'Predict':
            m['off'] = predict_hits(host, guest)
            guest.toggle(BH, True)
            m['on'] = predict_hits(host, guest)
            guest.toggle(BH, False)
            off, on = m['off'], m['on']
            checks.check('BlockHit off: the held use keeps the shield raised',
                         off['using'] >= 0.9 * off['samples'], m)
            checks.check('Predict: the shield is lowered between the hits',
                         on['using'] <= 0.7 * on['samples'], m)
            checks.check('Predict: the shield is raised ahead of the hits', on['raises'] >= 4, m)
        else:
            m['off'] = lag_backoff(host, guest)
            guest.toggle(BH, True)
            m['on'] = lag_backoff(host, guest)
            guest.toggle(BH, False)
            m['after'] = lag_backoff(host, guest)
            off, on, after = m['off'], m['on'], m['after']
            checks.check('BlockHit off: the host sees the guest move nearly every tick',
                         off['stalled'] <= 0.25 * off['ticks'] and off['travel'] > 0.5, m)
            checks.check('Lag: the shield cycles while the target is in front', on['raises'] >= 3, m)
            checks.check('Lag: the held release-use stalls the guest on the host',
                         on['stalled'] >= off['stalled'] + 6 and on['travel'] > 0.3, m)
            checks.check('BlockHit disabled: the packets flow again',
                         after['stalled'] <= off['stalled'] + 3, m)
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
    parser.add_argument('part', choices=('sp', 'lan', 'all', 'bh-sp', 'bh-lan', 'bh-manual', 'bh-auto',
                                         'bh-predict', 'bh-lag', 'hs-kb', 'hs-crit'))
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    results = []
    if args.part in ('sp', 'all'):
        sp_session(args.resources, args.allow_stale, 'Right-click use cancel', results)
        sp_session(args.resources, args.allow_stale, 'Normal', results)
    if args.part in ('lan', 'all'):
        lan(args.resources, args.allow_stale, results)
    for tag, preference in (('kb', 'KB reduction'), ('crit', 'Critical hits')):
        if args.part in ('all', f'hs-{tag}'):
            hs_velocity(args.resources, args.allow_stale, preference, results)
    for mode in ('Manual', 'Auto'):
        if args.part in ('bh-sp', 'all', f'bh-{mode.lower()}'):
            bh_sp(args.resources, args.allow_stale, mode, results)
    for mode in ('Predict', 'Lag'):
        if args.part in ('bh-lan', 'all', f'bh-{mode.lower()}'):
            bh_lan(args.resources, args.allow_stale, mode, results)
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
