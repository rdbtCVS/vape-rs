#!/usr/bin/env python3
"""Target mode and Target area host scenario for KillAura, SilentAura and AimAssist.

Three stationary zombies (NoAI, full knockback resistance) stand around the
player at different distance, angle and health:

  NEAR   1.8 blocks, 80 degrees off the view, 800 health
  AHEAD  2.8 blocks, 10 degrees off the view, 1000 health
  WEAK   2.6 blocks, 40 degrees off the view, 300 health

so Distance chooses NEAR, Yaw AHEAD and Health WEAK. Mobs carry no player
equipment scores, so Armor and Threat fall back to distance (NEAR), as the
reference comparators do for non-players. Five launches rotate the modes so
each module runs every mode once; each module is toggled alone and the
server's mob health shows which zombie it hit (KillAura, SilentAura) or the
view shows which one it turned to (AimAssist). Show target is captured with
SilentAura's Ring and Box. SilentAura's Kiss rotation and AimAssist's
Adaptive mode run in the later launches.

Target area (launches 0 and 1): one spider (1.4 wide, NoAI) stands with the
player's x inside its box, so Closest aims at its near face (straight ahead)
and Center at its middle (about 10.5 degrees right). AimAssist's settled
view shows its aim point. SilentAura (Extra swing distance 0, so its range
is 3.0) has its validity test on the aim coordinates: the spider's middle is
3.3 away and its near face 2.55, so with Center there is no target and no
hit, and with Closest the server's spider health drops while the view stays
put. (Single player applies the attack's knockback along the view yaw, not
the motion packet's, so knockback cannot show the managed rotation here.)
Zombies are tested at night: undead burn in daylight even with NoAI.
"""
import argparse
import json
import math
import os

from native_session import NativeSession
from smoke_scenario import Checks

KILL = ('combat.kill-aura', 'KeyK')
SILENT = ('combat.silent-aura', 'KeyJ')
AIM = ('combat.aim-assist', 'KeyH')
MODES = ['Distance', 'Yaw', 'Armor', 'Threat', 'Health']
EYE = (0.5, 100.0, 0.5)
VIEW = -10.0


def place(yaw, distance):
    """The point at `distance` along the Minecraft yaw from the player."""
    r = math.radians(yaw)
    return (EYE[0] - distance * math.sin(r), EYE[2] + distance * math.cos(r))


ZOMBIES = {
    'NEAR': (place(70.0, 1.8), 800),
    'AHEAD': (place(0.0, 2.8), 1000),
    'WEAK': (place(-50.0, 2.6), 300),
}
EXPECTED = {'Distance': 'NEAR', 'Yaw': 'AHEAD', 'Armor': 'NEAR', 'Threat': 'NEAR', 'Health': 'WEAK'}
SPIDER = (1.1, 3.75)


def yaw_to(x, z):
    return -math.degrees(math.atan2(x - EYE[0], z - EYE[2]))


def wrap(angle):
    return (angle + 180.0) % 360.0 - 180.0


CENTER_YAW = yaw_to(*SPIDER)
CLOSEST_YAW = yaw_to(EYE[0], SPIDER[1] - 0.7)


def mobs(session, kind):
    return [m for m in session.call('mobs', '16').get('near', [])
            if m.get('kind') == kind and (m.get('health') or 0) > 0]


def zombies(session):
    """Each labelled zombie's (id, health, pos), by nearest spawn point."""
    out = {}
    for m in mobs(session, 'minecraft:zombie'):
        x, _, z = m['pos']
        label = min(ZOMBIES, key=lambda k: math.dist((x, z), ZOMBIES[k][0]))
        if math.dist((x, z), ZOMBIES[label][0]) < 0.5:
            out[label] = {'id': m.get('id'), 'health': m.get('health'), 'pos': m['pos']}
    return out


def enabled(session, module):
    return session.saved()['modules'].get(module[0], {}).get('enabled', False)


def toggle(session, module):
    session.call('key', module[1][3:].lower(), 'tap')
    session.call('wait', '2')


def view(session, yaw, pitch=0.0):
    session.call('tp', str(EYE[0]), str(EYE[1]), str(EYE[2]))
    session.call('look', str(yaw), str(pitch))
    session.call('wait', '4')


def state_yaw(session):
    return session.call('state')['player']['yaw']


def summon(session, kind, x, z, health):
    """A stationary mob: NoAI, full knockback resistance, 1000 max health."""
    attributes = ('{id:"minecraft:max_health",base:1000},'
                  '{id:"minecraft:knockback_resistance",base:1}')
    nbt = f'{{PersistenceRequired:1b,NoAI:1b,Health:{health}f,attributes:[{attributes}]}}'
    session.call('chat', f'/summon {kind} {x:.3f} {EYE[1]} {z:.3f} {nbt}')


def clear_mobs(session):
    for kind in ('zombie', 'spider'):
        session.call('chat', f'/kill @e[type=minecraft:{kind}]')
    session.call('wait', '30')


def floor(session, checks):
    counts = {}
    for _ in range(10):
        session.call('fill', '-5', '99', '-4', '5', '99', '7', 'minecraft:stone')
        session.call('fill', '-5', '100', '-4', '5', '104', '7', 'minecraft:air')
        counts = session.call('blocks', '-5', '99', '-4', '5', '99', '7').get('counts', {})
        if counts.get('minecraft:stone') == 11 * 12:
            break
        session.call('wait', '20')
    checks.check('floor is in place', counts.get('minecraft:stone') == 11 * 12, counts)


def settings_for(launch):
    kill, silent, aim = MODES[launch], MODES[(launch + 1) % 5], MODES[(launch + 2) % 5]
    return (kill, silent, aim), {
        f'{KILL[0]}.target-mode': kill,
        f'{SILENT[0]}.target-mode': silent,
        f'{SILENT[0]}.render-type': 'Ring' if launch % 2 == 0 else 'Box',
        f'{SILENT[0]}.rotation-mode': 'Kiss' if launch == 3 else 'PID',
        f'{SILENT[0]}.target-area': 'Closest' if launch == 1 else 'Center',
        f'{AIM[0]}.target-mode': aim,
        f'{AIM[0]}.mode': 'Adaptive' if launch >= 3 else 'Simple',
        f'{AIM[0]}.target-area': 'Closest' if launch == 1 else 'Center',
    }


def launch_world(session, launch, first):
    modes, changes = settings_for(launch)
    if first:
        settings = {}
        for module, bind in (KILL, SILENT, AIM):
            settings[f'{module}.binds'] = [bind]
            settings[f'{module}.targets.mobs'] = True
            settings[f'{module}.max-angle'] = 360.0
        settings.update({
            f'{KILL[0]}.max-targets': 1.0,
            f'{KILL[0]}.show-target': True,
            f'{SILENT[0]}.show-target': True,
            f'{SILENT[0]}.extra-swing': 0.0,
            f'{AIM[0]}.require-mouse': False,
            f'{AIM[0]}.distance': 8.0,
            f'{AIM[0]}.horizontal-speed': 10.0,
        })
        settings.update(changes)
        session.seed({'schema': 1, 'modules': {}, 'settings': settings})
    else:
        session.call('quit')
        session.process.wait(timeout=15)
        session.log.close()
        session.process = None
        document = session.saved()
        document['settings'].update(changes)
        for module in (KILL, SILENT, AIM):
            document['modules'].setdefault(module[0], {})['enabled'] = False
        session.seed(document)
    session.launch(creative=True)
    session.call('chat', '/gamerule spawn_mobs false')
    session.call('time', '18000')
    session.call('tp', str(EYE[0]), str(EYE[1]), str(EYE[2]))
    session.call('wait-ready', '40')
    return modes


def zombie_round(session, checks, launch, modes, record):
    clear_mobs(session)
    for label, ((x, z), health) in ZOMBIES.items():
        summon(session, 'minecraft:zombie', x, z, health)
    found = {}
    for _ in range(20):
        session.call('wait', '10')
        found = zombies(session)
        if len(found) == 3:
            break
    checks.check(f'launch {launch}: three labelled zombies stand around the player', len(found) == 3,
                 found)
    kill, silent, aim = modes
    for module, mode, name in ((KILL, kill, 'KillAura'), (SILENT, silent, 'SilentAura')):
        view(session, VIEW)
        before = zombies(session)
        toggle(session, module)
        on = enabled(session, module)
        session.call('wait', '60')
        after = zombies(session)
        lost = {k: round(before[k]['health'] - after[k]['health'], 2)
                for k in ZOMBIES if k in before and k in after}
        want = EXPECTED[mode]
        hit = [k for k, v in lost.items() if v > 0]
        extra = {}
        if module is SILENT:
            extra['rotation'] = 'Kiss' if launch == 3 else 'PID'
            extra['render'] = 'Ring' if launch % 2 == 0 else 'Box'
        checks.check(f'launch {launch}: {name} Target mode {mode} hits only {want}',
                     on and hit == [want] and lost[want] >= 1.0,
                     {'enabled': on, 'health_lost': lost, **extra})
        record.append({'launch': launch, 'module': name, 'mode': mode, 'expected': want,
                       'health_lost': lost, **extra})
        # Show target on the chosen zombie, the view turned toward it.
        (x, z), _ = ZOMBIES[want]
        view(session, wrap(yaw_to(x, z) + 12.0), 25.0)
        session.call('wait', '6')
        session.capture(f'launch{launch}-{name.lower()}-{mode.lower()}')
        toggle(session, module)
        checks.check(f'launch {launch}: {name} disabled', not enabled(session, module))
    # AimAssist turns the view onto its target.
    adaptive = launch >= 3
    view(session, VIEW)
    toggle(session, AIM)
    on = enabled(session, AIM)
    yaws = []
    for _ in range(25):
        session.call('wait', '10')
        yaws.append(state_yaw(session))
    settled = yaws[-1]
    errors = {k: round(abs(wrap(settled - yaw_to(*ZOMBIES[k][0]))), 2) for k in ZOMBIES}
    looking = session.call('state').get('looking_at')
    want = EXPECTED[aim]
    best = min(errors, key=errors.get)
    checks.check(f'launch {launch}: AimAssist ({"Adaptive" if adaptive else "Simple"}) Target mode '
                 f'{aim} turns onto {want}',
                 on and best == want and errors[want] < 8.0,
                 {'enabled': on, 'settled_yaw': settled, 'yaw_error': errors, 'looking_at': looking,
                  'trace': yaws[::4]})
    record.append({'launch': launch, 'module': 'AimAssist', 'mode': aim, 'expected': want,
                   'adaptive': adaptive, 'settled_yaw': settled, 'yaw_error': errors})
    session.capture(f'launch{launch}-aimassist-{aim.lower()}')
    toggle(session, AIM)
    checks.check(f'launch {launch}: AimAssist disabled', not enabled(session, AIM))


def spider(session):
    found = mobs(session, 'minecraft:spider')
    return found[0] if found else None


def area_round(session, checks, launch, record):
    area = 'Closest' if launch == 1 else 'Center'
    want = CLOSEST_YAW if area == 'Closest' else CENTER_YAW
    clear_mobs(session)
    summon(session, 'minecraft:spider', SPIDER[0], SPIDER[1], 1000)
    s = None
    for _ in range(20):
        session.call('wait', '10')
        s = spider(session)
        if s:
            break
    checks.check(f'launch {launch}: a spider stands ahead', s is not None, s)
    if s is None:
        return
    # AimAssist from 30 degrees left: the settled view is its aim point.
    view(session, 30.0)
    toggle(session, AIM)
    yaws = []
    for _ in range(25):
        session.call('wait', '10')
        yaws.append(state_yaw(session))
    toggle(session, AIM)
    settled = sum(yaws[-5:]) / 5
    record.append({'launch': launch, 'module': 'AimAssist', 'area': area, 'aim_yaw': round(want, 2),
                   'settled_yaw': round(settled, 2), 'trace': yaws[::4]})
    checks.check(f'launch {launch}: AimAssist Target area {area} settles near {want:.1f}',
                 abs(wrap(settled - want)) < 6.0, {'settled_yaw': settled, 'aim_yaw': want})

    # SilentAura: Closest finds the near face in range, Center does not.
    view(session, 30.0)
    before = spider(session)
    toggle(session, SILENT)
    on = enabled(session, SILENT)
    session.call('wait', '60')
    after = spider(session)
    toggle(session, SILENT)
    lost = round(before['health'] - after['health'], 2) if before and after else None
    moved = round(math.dist((before['pos'][0], before['pos'][2]),
                            (after['pos'][0], after['pos'][2])), 3) if before and after else None
    centre = math.dist((EYE[0], EYE[2]), SPIDER)
    face = SPIDER[1] - 0.7 - EYE[2]
    record.append({'launch': launch, 'module': 'SilentAura', 'area': area, 'range': 3.0,
                   'center_distance': round(centre, 3), 'face_distance': round(face, 3),
                   'health_lost': lost, 'moved': moved, 'view_after': state_yaw(session)})
    if area == 'Closest':
        checks.check('launch 1: SilentAura Target area Closest hits the spider by its near face '
                     f'({face:.2f} in range, its middle {centre:.2f} is not)',
                     on and lost is not None and lost >= 1.0, {'enabled': on, 'health_lost': lost})
    else:
        checks.check(f'launch 0: SilentAura Target area Center has no target (the middle is '
                     f'{centre:.2f}, past the 3.0 range)',
                     on and lost == 0, {'enabled': on, 'health_lost': lost})
    checks.check(f'launch {launch}: the NoAI spider stays put', moved is not None and moved < 0.05,
                 moved)
    checks.check(f'launch {launch}: SilentAura leaves the view where it was',
                 abs(wrap(state_yaw(session) - 30.0)) < 1e-3, state_yaw(session))
    return lost


def run(resources, allow_stale):
    with NativeSession(resources, 'targeting-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        record = []
        silent_area = {}
        for launch in range(5):
            modes = launch_world(session, launch, launch == 0)
            if launch == 0:
                floor(session, checks)
            zombie_round(session, checks, launch, modes, record)
            if launch in (0, 1):
                silent_area[launch] = area_round(session, checks, launch, record)
        center, closest = silent_area.get(0), silent_area.get(1)
        checks.check('SilentAura: Closest reaches a target Center cannot',
                     center == 0 and (closest or 0) >= 1.0, {'center': center, 'closest': closest})
        aim_area = {r['area']: r['settled_yaw'] for r in record
                    if r['module'] == 'AimAssist' and 'area' in r}
        checks.check('AimAssist: Closest settles left of Center',
                     len(aim_area) == 2 and wrap(aim_area['Closest'] - aim_area['Center']) > 4.0,
                     aim_area)
        (session.root / 'targeting.json').write_text(json.dumps(record, indent=2) + '\n')
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        for r in checks.results:
            if not r['ok']:
                print('FAILED', r['check'], json.dumps(r.get('detail'))[:400])
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
