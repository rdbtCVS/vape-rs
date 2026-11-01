#!/usr/bin/env python3
"""Inventory and utility round 6: AutoArmor with no screen ("Open inventory"
and "Inventory only" both off), in single player and over LAN. Every case
runs in its own fresh isolated homes and world and writes assertions.json
beside its captures; the summary names each case's directory."""
import argparse
import json
from pathlib import Path
import time

from native_session import NativeSession
import utility_debt_scenario as ud
from utility_debt_scenario import IRON, armor, armor_trace

AA = 'inventory.auto-armor'
SCREENLESS = {f'{AA}.inventory-only': False, f'{AA}.open-inventory': False}


def screens_until(case, done, seconds=8, step=1):
    """Screens seen (in order, deduplicated) until done()."""
    screens = []
    start = time.monotonic()
    while time.monotonic() - start < seconds:
        screen = case.state()['screen']
        if not screens or screens[-1] != screen:
            screens.append(screen)
        if done():
            break
        case.s.call('wait', str(step))
    return screens


def main_items(case):
    return [s.get('item') for s in case.inventory()['main']]


def armor_screenless(case):
    """Both screen settings off: the clicks go to the player's menu with no
    screen; head, chest, then feet; the old helmet goes to the main rows."""
    case.prepare(SCREENLESS)
    case.put('armor.head', 'minecraft:leather_helmet')
    case.put('inventory.0', 'minecraft:iron_boots')
    case.put('inventory.5', 'minecraft:iron_chestplate')
    case.put('inventory.20', 'minecraft:iron_helmet')
    case.tap(AA)
    screens = []
    start = time.monotonic()
    trace = []
    while time.monotonic() - start < 8:
        screen = case.state()['screen']
        if not screens or screens[-1] != screen:
            screens.append(screen)
        now = armor(case)
        if not trace or trace[-1][1] != now:
            trace.append((round(time.monotonic() - start, 3), now))
        if now == IRON:
            break
        case.s.call('wait', '1')
    assert trace[-1][1] == IRON, trace
    assert screens == ['playing'], screens
    assert 'minecraft:leather_helmet' in main_items(case), main_items(case)
    carried = case.inventory().get('carried')
    assert not carried or not carried.get('item'), carried
    assert case.enabled(AA)
    case.checks.update(screens=screens, trace=trace)
    case.s.capture('armor-screenless')


def armor_screenless_drop(case):
    """With "Drop equipped" the worn helmet is picked up and dropped
    outside the menu (slot -999), still with no screen."""
    case.prepare({**SCREENLESS, f'{AA}.drop-equipped': True})
    case.put('armor.head', 'minecraft:leather_helmet')
    case.put('inventory.0', 'minecraft:iron_helmet')
    case.tap(AA)
    screens = screens_until(case, lambda: armor(case)['head'] == 'minecraft:iron_helmet')
    inventory = case.inventory()
    assert armor(case)['head'] == 'minecraft:iron_helmet', armor(case)
    assert screens == ['playing'], screens
    assert 'minecraft:leather_helmet' not in inventory['counts'], inventory['counts']
    assert not inventory.get('carried') or not inventory['carried'].get('item')
    case.s.call('wait', '10')
    entities = [e for e in case.s.call('entities', '8')['entities'] if e.get('kind') == 'item']
    assert any(e.get('item', {}).get('item') == 'minecraft:leather_helmet' for e in entities), entities
    case.checks.update(screens=screens, dropped=entities)
    case.s.capture('armor-screenless-drop')


def armor_screenless_delay(case, delay):
    """The Delay still spaces the screenless clicks."""
    case.prepare({**SCREENLESS, f'{AA}.delay': [delay, delay]})
    case.put('inventory.0', 'minecraft:iron_boots')
    case.put('inventory.1', 'minecraft:iron_chestplate')
    case.put('inventory.2', 'minecraft:iron_helmet')
    case.tap(AA)
    trace = armor_trace(case, lambda a: a == IRON)
    assert trace[-1][1] == IRON, trace
    assert case.state()['screen'] == 'playing'
    case.checks.update(delay=delay, trace=trace, total=trace[-1][0])
    case.s.capture(f'armor-screenless-delay-{delay}')


def armor_screenless_noop(case):
    """Nothing better to wear: no click, no screen, the module stays on."""
    case.prepare(SCREENLESS)
    case.put('armor.head', 'minecraft:iron_helmet')
    case.put('inventory.0', 'minecraft:leather_helmet')
    before = case.inventory()['counts']
    case.tap(AA)
    screens = screens_until(case, lambda: False, seconds=3, step=2)
    assert screens == ['playing'], screens
    assert armor(case)['head'] == 'minecraft:iron_helmet'
    assert case.inventory()['counts'] == before
    assert case.enabled(AA)
    case.checks.update(screens=screens, counts=before)


CASES = {
    'armor-screenless': armor_screenless,
    'armor-screenless-drop': armor_screenless_drop,
    'armor-screenless-delay-300': lambda c: armor_screenless_delay(c, 300),
    'armor-screenless-delay-1': lambda c: armor_screenless_delay(c, 1),
    'armor-screenless-noop': armor_screenless_noop,
}


def lan(resources):
    """A LAN guest with both screen settings off equips over the real
    protocol with no screen; a reconnect proves the server kept it."""
    results = []

    def check(name, ok, detail=None):
        results.append({'check': name, 'ok': bool(ok), 'detail': detail})
        print('PASS' if ok else 'FAIL', name, flush=True)

    with NativeSession(resources, 'r6-lan-host-') as host, \
            NativeSession(resources, 'r6-lan-guest-') as guest:
        host.seed({'schema': 1, 'modules': {}, 'settings': {}})
        host.launch(creative=False, world='LanHost')
        host.call('time', '6000')
        host.call('tp', '0.5', '100', '0.5')
        host.call('wait-ready', '40')
        host.call('fill', '-6', '96', '-6', '6', '99', '20', 'minecraft:stone')
        host.call('fill', '-6', '100', '-6', '6', '104', '20', 'minecraft:air')
        opened = host.call('lan', 'open', '0', 'commands')
        port = opened.get('port') or opened.get('lan', {}).get('port')
        check('host opened to LAN', port, opened)
        address = f'localhost:{port}'
        guest.seed({'schema': 1, 'modules': {}, 'settings': {
            f'{AA}.binds': ['KeyU'], **SCREENLESS, f'{AA}.drop-equipped': True}})
        guest.launch(creative=False, world=None,
                     extra=('--server', address, '--name', 'VapeGuest'))
        guest.call('wait-ready', '60')
        host.call('command', 'tp VapeGuest 2.5 100 0.5')
        host.call('command', 'gamemode survival VapeGuest')
        guest.call('wait', '20')
        for where, item in (('armor.head', 'leather_helmet'), ('inventory.3', 'iron_helmet'),
                            ('inventory.4', 'iron_chestplate'), ('inventory.22', 'iron_boots')):
            host.call('command', f'item replace entity VapeGuest {where} with minecraft:{item}')
        guest.call('wait', '10')
        inv = guest.call('inventory')
        check('guest received the armor', inv['counts'].get('minecraft:iron_helmet') == 1, inv['counts'])
        guest.call('key', 'u', 'tap')
        screens = []
        start = time.monotonic()
        while time.monotonic() - start < 10:
            screen = guest.call('state')['screen']
            if not screens or screens[-1] != screen:
                screens.append(screen)
            parts = {p['part']: p.get('item') for p in guest.call('inventory')['armor']}
            if parts == IRON:
                break
            guest.call('wait', '1')
        check('guest equips iron with no screen', parts == IRON, parts)
        check('guest screen stayed playing', screens == ['playing'], screens)
        guest.call('wait', '10')
        entities = [e for e in host.call('entities', '12')['entities'] if e.get('kind') == 'item']
        check('the server dropped the old helmet',
              any(e.get('item', {}).get('item') == 'minecraft:leather_helmet' for e in entities),
              entities)
        guest.call('wait', '20')
        guest.call('disconnect')
        time.sleep(1)
        guest.call('connect', address)
        guest.call('wait-ready', '60')
        guest.call('wait', '20')
        parts = {p['part']: p.get('item') for p in guest.call('inventory')['armor']}
        check('server kept the armor after reconnect', parts == IRON, parts)
        counts = guest.call('inventory')['counts']
        check('no leather helmet left in the inventory', 'minecraft:leather_helmet' not in counts, counts)
        guest.capture('armor-screenless-lan')
        for name, session in (('host', host), ('guest', guest)):
            log = (session.root / 'game.log').read_text(errors='replace')
            check(f'no panics in the {name} log', 'panicked' not in log and 'poisoned' not in log)
        guest.call('disconnect')
        guest.close()
        host.close()
        failed = [r['check'] for r in results if not r['ok']]
        (guest.root / 'assertions.json').write_text(json.dumps(
            {'passed': not failed, 'checks': results}, indent=2) + '\n')
        return {'root': str(guest.root), 'host': str(host.root),
                'passed': not failed, 'failed': failed}


def run(resources, names):
    summary = {}
    for name in names:
        if name == 'armor-screenless-lan':
            summary[name] = lan(resources)
            print(name, 'PASS' if summary[name]['passed'] else 'FAIL', flush=True)
            continue
        with NativeSession(resources, f'r6-{name[:16]}-') as session:
            case = ud.Case(session)
            CASES[name](case)
            session.close()
            case.passed()
            summary[name] = {'root': str(session.root), **case.checks}
            print(name, 'PASS', json.dumps(case.checks), flush=True)
    if {'armor-screenless-delay-300', 'armor-screenless-delay-1'} <= summary.keys():
        slow, fast = (summary[n]['total'] for n in
                      ('armor-screenless-delay-300', 'armor-screenless-delay-1'))
        # Three clicks each wait the Delay sample.
        assert slow - fast > 0.5, (slow, fast)
        summary['armor-screenless-delay-gap'] = {'delay_300_s': slow, 'delay_1_s': fast}
    out = Path(__file__).resolve().parents[1] / 'target/scenarios' / (
        'invutil-r6-summary-' + time.strftime('%Y%m%dT%H%M%S') + '.json')
    out.write_text(json.dumps(summary, indent=2) + '\n')
    print(out)
    return summary


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('cases', nargs='*', help='case names (default: all)')
    args = parser.parse_args()
    known = [*CASES, 'armor-screenless-lan']
    unknown = set(args.cases) - set(known)
    if unknown:
        parser.error(f'unknown cases {sorted(unknown)}; choose from {known}')
    summary = run(args.resources, args.cases or known)
    raise SystemExit(0 if all(v.get('passed', True) for v in summary.values()) else 1)
