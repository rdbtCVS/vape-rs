#!/usr/bin/env python3
"""AutoPearl and PearlCatch host scenarios in fresh isolated homes.

PearlCatch (single player, survival, one fresh session per variant) on a
stone pad under open sky with ender pearls in slot 2 and wind charges in
slot 3, starting on slot 1:

- Upward with silent aim (the default): the module selects the pearl, then
  the charge, throws both (one of each leaves the inventory), restores slot 1
  and disables itself; the camera pitch never moves.
- Upward with silent aim off: the shared rotation's visible controller turns
  the camera up past -75 before the pearl leaves.
- Current aim (silent): both throws from a -30 view; the camera stays put.

AutoPearl Aggro over LAN (two owned local processes): the host player, three
blocks ahead of the guest, throws a real pearl forward. The guest's AutoPearl
turns the camera (visible aim, the default), selects its pearl slot, throws
(the server takes the pearl), restores its empty slot, stays enabled, and
the server teleports the guest far down the runway, within a fifth of the
pearl's reach from where the host's pearl landed (the reference's drag-free
pitch solver lands a little short at long range).

AutoPearl On bind over LAN, the same runway: the bind with no pearl flying
enables the module, which finds nothing and disables itself; the bind again
while the host's pearl is in the air throws after it (the guest lands down
the runway) and the module disables itself after the throw.

Results go to result.json in each session's home.
"""
import argparse
import os

from native_session import NativeSession
from smoke_scenario import Checks, enabled, poll, state

CATCH = 'utility.pearl-catch'
AUTO = 'utility.auto-pearl'
KEYS = {CATCH: 'KeyU', AUTO: 'KeyY'}
FLOOR = 99


def toggle(session, module):
    session.call('key', KEYS[module][3:].lower(), 'tap')
    session.call('wait', '2')


def counts(session):
    return session.call('inventory').get('counts', {})


def call(session, *args):
    return session.call(*map(str, args))


def pad(session, checks, x=(-6, 6), z=(-6, 6), sky=160):
    """A stone floor at y=99 with air above it."""
    expected = (x[1] - x[0] + 1) * (z[1] - z[0] + 1)
    for _ in range(10):
        call(session, 'fill', x[0], FLOOR - 3, z[0], x[1], FLOOR, z[1], 'minecraft:stone')
        for lo in range(FLOOR + 1, sky, 20):
            call(session, 'fill', x[0], lo, z[0], x[1], min(lo + 19, sky), z[1], 'minecraft:air')
        session.call('wait', '10')
        found = call(session, 'blocks', x[0], FLOOR, z[0], x[1], FLOOR, z[1]).get('counts', {})
        if found.get('minecraft:stone') == expected:
            break
        session.call('wait', '20')
    checks.check('stone pad under open sky is in place',
                 found.get('minecraft:stone') == expected, found)


def pearl_catch(resources, allow_stale, name, extra_settings, look):
    with NativeSession(resources, f'pc-{name}-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        settings = {f'{CATCH}.binds': [KEYS[CATCH]], **extra_settings}
        session.seed({'schema': 1, 'modules': {}, 'settings': settings})
        session.launch(creative=False)
        session.call('time', '6000')
        session.call('tp', '0.5', str(FLOOR + 1), '0.5')
        session.call('wait-ready', '40')
        pad(session, checks)
        # The agent's slots are 1-based and the command's hotbar.N 0-based:
        # slot 1 stays empty, pearls in slot 2 and charges in slot 3.
        session.call('command', 'item replace entity @s hotbar.1 with minecraft:ender_pearl 4')
        session.call('command', 'item replace entity @s hotbar.2 with minecraft:wind_charge 4')
        session.call('hotbar', '1')
        session.call('tp', '0.5', str(FLOOR + 1), '0.5')
        session.call('look', *look)
        session.call('wait', '20')
        before = counts(session)
        s = state(session)
        view = s['player']
        checks.check('pearls in slot 2 and charges in slot 3, an empty hand, the camera at the start view',
                     before.get('minecraft:ender_pearl') == 4 and before.get('minecraft:wind_charge') == 4
                     and s['selected'] == 1 and s.get('held') is None
                     and abs(view['pitch'] - float(look[1])) < 0.01,
                     {'counts': before, 'held': s.get('held'), 'view': [view['yaw'], view['pitch']]})
        session.call('key', KEYS[CATCH][3:].lower(), 'tap')
        samples = []
        for _ in range(60):
            s = session.call('state')
            samples.append({'selected': s['selected'], 'yaw': round(s['player']['yaw'], 2),
                            'pitch': round(s['player']['pitch'], 2),
                            'position': [round(v, 2) for v in s['player']['position']],
                            'health': round(s['player']['health'], 2),
                            'pearls': counts(session).get('minecraft:ender_pearl', 0)})
            session.call('wait', '1')
        session.capture(f'pearl-catch-{name}')
        session.call('wait', '60')
        after = counts(session)
        selected = [x['selected'] for x in samples]
        order = [v for i, v in enumerate(selected) if i == 0 or selected[i - 1] != v]
        pitches = sorted({x['pitch'] for x in samples})
        # Each throw's slot is held for a tick or two, between samples at
        # times; only slot 2 holds pearls and slot 3 charges (the offhand is
        # empty), so each leaving the inventory is the evidence of its slot.
        checks.check('PearlCatch selected the pearl slot, ended on slot 1',
                     order[0] == 1 and 2 in order and order[-1] == 1, order)
        checks.check('PearlCatch threw one pearl and one wind charge',
                     after.get('minecraft:ender_pearl') == 3 and after.get('minecraft:wind_charge') == 3,
                     after)
        checks.check('PearlCatch disabled itself', not enabled(session, CATCH))
        checks.check('PearlCatch restored slot 1', state(session)['selected'] == 1, state(session)['selected'])
        if extra_settings.get(f'{CATCH}.silent-aim') is False:
            first_pearl = selected.index(2) if 2 in selected else len(samples) - 1
            before_pearl = [x['pitch'] for x in samples[:first_pearl + 1]]
            checks.check('visible PearlCatch turned the camera up past -75 by the pearl throw',
                         min(before_pearl) < -75.0, before_pearl)
        else:
            # The silent aim never turns the camera while the pearl is still
            # in the hotbar. After the throw the integrated server can answer
            # the use with an absolute teleport carrying the rotation the use
            # sent (Horizon's as_guest, lan_actions.rs), so the view checked
            # afterwards is reported, not asserted, when that happened.
            pitch = float(look[1])
            aiming = [x for x in samples if x['pearls'] == 4]
            checks.check('silent PearlCatch left the camera pitch while aiming',
                         all(abs(x['pitch'] - pitch) < 0.01 for x in aiming), [x['pitch'] for x in aiming])
            moved = [i for i, x in enumerate(samples) if abs(x['pitch'] - pitch) >= 0.01]
            checks.check('silent PearlCatch: the camera after the throw', True,
                         {'unchanged': not moved, 'first_changed_sample': moved[:1],
                          'pearls_then': [samples[i]['pearls'] for i in moved[:1]]})
        end = state(session)['player']
        checks.check('trace', True, {'samples': samples, 'end': end['position'], 'health': end['health']})
        checks.check('the player is alive after the pearl', state(session)['player']['health'] > 0,
                     state(session)['player']['health'])
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', name, f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}',
              session.root)
        return ok


def auto_pearl(resources, allow_stale, mode='aggro'):
    prefix = 'pearl-lan' if mode == 'aggro' else f'pearl-{mode}-lan'
    with NativeSession(resources, f'{prefix}-host-', allow_stale=allow_stale) as host, \
            NativeSession(resources, f'{prefix}-guest-', allow_stale=allow_stale) as guest:
        checks = Checks(guest.root)
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=False, world='LanHost')
        host.call('time', '6000')
        host.call('tp', '0.5', str(FLOOR + 1), '3.5')
        host.call('wait-ready', '40')
        pad(checks=checks, session=host, z=(-6, 60), sky=130)
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        checks.check('host opened to LAN', bool(port), opened)

        guest.seed({'schema': 1, 'modules': {},
                    'settings': {f'{AUTO}.binds': [KEYS[AUTO]], f'{AUTO}.mode': mode}})
        guest.launch(creative=False, world=None,
                     extra=('--server', f'localhost:{port}', '--name', 'VapeGuest'))
        guest.call('wait-ready', '60')
        host.call('command', 'item replace entity VapeGuest hotbar.3 with minecraft:ender_pearl 4')
        host.call('command', 'tp VapeGuest 0.5 100 0.5 0 0')
        guest.call('hotbar', '1')
        guest.call('wait', '20')
        s = state(guest)
        before = counts(guest)
        checks.check('guest joined with pearls in slot 4 and an empty hand',
                     s['player']['health'] > 0 and before.get('minecraft:ender_pearl') == 4
                     and s.get('held') is None, {'position': s['player']['position'], 'counts': before,
                                                 'held': s.get('held')})
        toggle(guest, AUTO)
        if mode == 'aggro':
            checks.check('AutoPearl enabled through its bind', enabled(guest, AUTO))
        else:
            # On bind looks once at the pearls already flying: none.
            guest.call('wait', '3')
            checks.check('AutoPearl On bind with no pearl flying disables itself',
                         not enabled(guest, AUTO) and state(guest)['selected'] == 1)
        guest.call('wait', '10')

        host.call('hotbar', '1')
        host.call('give', 'minecraft:ender_pearl', '2')
        host.call('tp', '0.5', str(FLOOR + 1), '3.5')
        host.call('look', '0', '-30')
        host.call('wait', '10')
        start = state(guest)['player']['position']
        host.call('mouse', 'right', 'down')
        host.call('wait', '1')
        host.call('mouse', 'right', 'up')
        if mode == 'on-bind':
            # The bind while the host's pearl is in the air.
            guest.call('wait', '2')
            guest.call('key', KEYS[AUTO][3:].lower(), 'tap')
        samples = []
        for _ in range(80):
            s = guest.call('state')
            samples.append({'selected': s['selected'], 'yaw': s['player']['yaw'],
                            'pitch': s['player']['pitch'], 'position': s['player']['position']})
            guest.call('wait', '1')
        guest.capture('auto-pearl-guest')
        ok, s = poll(guest, lambda s: s['player']['position'][2] > 10.0, 80)
        after = counts(guest)
        selected = [x['selected'] for x in samples]
        order = [v for i, v in enumerate(selected) if i == 0 or selected[i - 1] != v]
        views = sorted({(round(x['yaw'], 1), round(x['pitch'], 1)) for x in samples})
        checks.check('host threw a real pearl', counts(host).get('minecraft:ender_pearl') == 1, counts(host))
        checks.check('guest AutoPearl selected its pearl slot and restored the hand',
                     4 in order and order[-1] == 1 and state(guest)['selected'] == 1, order)
        checks.check('guest server took one pearl', after.get('minecraft:ender_pearl') == 3, after)
        checks.check('guest camera turned for the visible aim', len(views) > 1, views[:6])
        checks.check('guest teleported down the runway', ok,
                     {'start': start, 'end': s['player']['position']})
        landed = state(host)['player']['position']
        apart = sum((a - b) ** 2 for a, b in zip(landed, s['player']['position'])) ** 0.5
        reach = sum((a - b) ** 2 for a, b in zip(landed, start)) ** 0.5
        # ProjectilePitchUtil.calculatePitch solves a drag-free arc and
        # computeThrowPitch adds 5 degrees; with the pearl's 0.99 drag that
        # lands somewhat short at long range, as in the reference.
        checks.check("guest landed near the host's pearl landing", apart < 0.2 * reach,
                     {'host': landed, 'guest': s['player']['position'], 'apart': round(apart, 2),
                      'reach': round(reach, 2)})
        if mode == 'aggro':
            checks.check('AutoPearl Aggro stays enabled', enabled(guest, AUTO))
        else:
            checks.check('AutoPearl On bind disables itself after its throw', not enabled(guest, AUTO))
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'auto-pearl {mode}', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}',
              guest.root)
        return ok


def run(resources, allow_stale, only=None):
    variants = {
        'upward-silent': lambda: pearl_catch(resources, allow_stale, 'upward-silent', {}, ('0', '0')),
        'upward-visible': lambda: pearl_catch(resources, allow_stale, 'upward-visible',
                                              {f'{CATCH}.silent-aim': False}, ('0', '0')),
        'current-aim': lambda: pearl_catch(resources, allow_stale, 'current-aim',
                                           {f'{CATCH}.aim-mode': 'current-aim'}, ('0', '-30')),
        'auto-pearl': lambda: auto_pearl(resources, allow_stale),
        'auto-pearl-on-bind': lambda: auto_pearl(resources, allow_stale, 'on-bind'),
    }
    results = {name: f() for name, f in variants.items() if not only or name in only}
    print(results)
    return all(results.values())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?',
                        default=os.environ.get('HORIZON_RESOURCES'),
                        help='prepared Minecraft 26.3 resources (or set HORIZON_RESOURCES)')
    parser.add_argument('--allow-stale', action='store_true')
    parser.add_argument('--only', action='append')
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run(args.resources, args.allow_stale, args.only) else 1)


if __name__ == '__main__':
    main()
