#!/usr/bin/env python3
"""Throwpot, ThrowDebuff, Refill, AutoArmor and AutoTool weapon scoring in
survival on the integrated server. Every case runs in its own fresh isolated
homes and world and writes assertions.json beside its captures; the summary
names each case's directory."""
import argparse
import json
from pathlib import Path
import time
from native_session import NativeSession

HEALING = 'minecraft:splash_potion[minecraft:potion_contents={potion:"minecraft:healing"}]'
STRONG = 'minecraft:splash_potion[minecraft:potion_contents={potion:"minecraft:strong_healing"}]'
POISON = 'minecraft:splash_potion[minecraft:potion_contents={potion:"minecraft:poison"}]'
WEAKNESS = 'minecraft:splash_potion[minecraft:potion_contents={potion:"minecraft:weakness"}]'
STEW = 'minecraft:mushroom_stew'
# A stack-level instant-consume component, as soup servers configure stew.
INSTANT_STEW = 'minecraft:mushroom_stew[minecraft:consumable={consume_seconds:0.0f}]'
BINDS = {
    'inventory.throw-pot': 'KeyK',
    'inventory.throw-debuff': 'KeyL',
    'inventory.refill': 'KeyR',
    'inventory.auto-armor': 'KeyU',
}


class Case:
    def __init__(self, session):
        self.s = session
        self.checks = {}

    def command(self, text):
        self.s.call('command', text)
        self.s.call('wait', '2')

    def state(self):
        return self.s.call('state')

    def inventory(self):
        return self.s.call('inventory')

    def hotbar(self):
        return [slot.get('item') for slot in self.inventory()['hotbar']]

    def enabled(self, module):
        return self.s.saved()['modules'].get(module, {}).get('enabled') is True

    def tap(self, module):
        self.s.call('key', BINDS[module][3:].lower(), 'tap')

    def poll(self, predicate, ticks=80, step=2):
        for _ in range(0, ticks, step):
            if predicate():
                return True
            self.s.call('wait', str(step))
        return predicate()

    def prepare(self, settings=None, modules=None):
        self.s.seed({'schema': 1, 'modules': modules or {}, 'settings': {
            **{f'{m}.binds': [k] for m, k in BINDS.items()}, **(settings or {})}})
        self.s.launch()
        self.s.call('tp', '0.5', '100', '0.5')
        self.s.call('wait-ready', '40')
        self.s.call('fill', '-3', '99', '-3', '3', '99', '5', 'minecraft:stone')
        self.s.call('tp', '0.5', '100', '0.5')
        self.command('gamemode survival')
        # Health only changes through the scenario's own damage and heals.
        self.command('gamerule natural_health_regeneration false')
        self.command('clear @s')
        self.command('effect clear @s')
        self.s.call('wait', '20')  # the fixture's fall/teleport guard expires

    def put(self, where, item):
        self.command(f'item replace entity @s {where} with {item}')

    def passed(self):
        (self.s.root / 'assertions.json').write_text(
            json.dumps({'passed': True, 'checks': self.checks}, indent=2) + '\n')


def throwpot_pots(case, mode):
    """Dynamic stops before overhealing; Single takes the first pot."""
    case.prepare({'inventory.throw-pot.type': 'pots', 'inventory.throw-pot.mode': mode})
    for slot, item in enumerate((HEALING, HEALING, STRONG, HEALING)):
        case.put(f'hotbar.{slot}', item)
    case.s.call('hotbar', '6')
    case.s.call('look', '0', '90')
    case.command('damage @s 10 minecraft:generic')
    before = case.state()['player']['health']
    assert before == 10, before
    case.tap('inventory.throw-pot')
    case.s.call('wait', '40')
    hotbar = case.inventory()['hotbar']
    state = case.state()
    kept = [i for i in range(4) if hotbar[i].get('item')]
    # Dynamic: 10 + 4 + 4 = 18, the Instant Health II (+8) and the last
    # Instant Health I (+4) would pass 20 and are kept. Single: slot 1 only.
    expected = [2, 3] if mode == 'dynamic' else [1, 2, 3]
    assert kept == expected, (mode, hotbar)
    assert 'strong_healing' in hotbar[2].get('components', ''), hotbar[2]
    assert state['player']['health'] > before, state['player']
    assert state['selected'] == 6, state['selected']
    assert not case.enabled('inventory.throw-pot')
    case.checks.update(health_before=before, health_after=state['player']['health'],
                       kept_slots=kept, selected_after=state['selected'])
    case.s.capture('throwpot-' + mode)


def throwpot_budget(case, kind):
    """Both counts the stew's 8 HP in the Dynamic budget; Pots does not."""
    case.prepare({'inventory.throw-pot.type': kind, 'inventory.throw-pot.delay': [200, 200]})
    for slot, item in enumerate((STEW, HEALING, HEALING, HEALING)):
        case.put(f'hotbar.{slot}', item)
    case.s.call('hotbar', '6')
    case.s.call('look', '0', '90')
    case.command('damage @s 14 minecraft:generic')
    before = case.state()['player']['health']
    assert before == 6, before
    case.tap('inventory.throw-pot')
    seen = []
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        selected = case.state()['selected']
        if not seen or seen[-1] != selected:
            seen.append(selected)
    case.s.call('wait', '10')
    hotbar = case.hotbar()
    state = case.state()
    pots = hotbar[:4].count('minecraft:splash_potion')
    assert hotbar[0] == 'minecraft:mushroom_stew', hotbar
    if kind == 'both':
        # 6 + 8 (stew) + 4 = 18; the other two pots would pass 20. The pot
        # in slot 2 is nearer the saved slot 6 than the stew in slot 1.
        assert pots == 2 and hotbar[1] is None, hotbar
        assert 1 in seen and 2 in seen, seen
    else:
        assert pots == 0, hotbar  # 6 + 4 + 4 + 4 = 18
        assert 1 not in seen, seen
    assert state['selected'] == 6
    assert not case.enabled('inventory.throw-pot')
    case.checks.update(health_before=before, health_after=state['player']['health'],
                       hotbar_after=hotbar[:4], selection_trace=seen)


def throwpot_soup(case):
    """Soup Single selects the stew and starts eating it through the host's
    use path, then restores the slot; a vanilla stew is kept."""
    case.prepare({'inventory.throw-pot.type': 'soup', 'inventory.throw-pot.mode': 'single',
                  'inventory.throw-pot.delay': [200, 200]})
    case.command('effect give @s minecraft:hunger 2 255 true')
    case.s.call('wait', '50')
    food = case.state()['player']['food']
    assert food < 20, food
    case.put('hotbar.0', STEW)
    case.s.call('hotbar', '6')

    def run():
        case.tap('inventory.throw-pot')
        trace = []
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            state = case.state()
            sample = (state['selected'], state['using'] is not None)
            if not trace or trace[-1] != sample:
                trace.append(sample)
        case.s.call('wait', '10')
        return trace

    trace = run()
    hotbar = case.inventory()['hotbar']
    assert (1, True) in trace, trace  # eating the selected stew
    assert trace[-1] == (6, False), trace
    assert hotbar[0].get('item') == 'minecraft:mushroom_stew', hotbar[0]
    assert 'minecraft:bowl' not in [s.get('item') for s in hotbar]
    dropped = [e for e in case.s.call('entities', '8')['entities'] if e.get('kind') == 'item']
    assert not dropped, dropped  # "Throw bowls" drops only a held bowl
    assert not case.enabled('inventory.throw-pot')
    case.checks.update(food=food, trace=trace, slot1=hotbar[0])
    # A soup server's instant stew: the server's stack holds the component,
    # but the client's slot sync skips it and the use path reads the item
    # type's consume time, so one press does not eat it and the bowl drop
    # cannot be reached (blocked, docs/decisions.md).
    case.put('hotbar.0', INSTANT_STEW)
    server = case.s.call('command', 'data get entity @s Inventory')['chat']
    assert 'minecraft:consumable' in server[0] and 'consume_seconds: 0.0f' in server[0], server
    given = case.inventory()['hotbar'][0]
    assert not given.get('components'), given  # the client copy lost it
    trace = run()
    hotbar = case.inventory()['hotbar']
    assert hotbar[0].get('item') == 'minecraft:mushroom_stew', hotbar[0]
    case.checks.update(instant_server_stack=server, instant_client_stack=given,
                       instant_trace=trace, instant_after=hotbar[0])
    case.s.capture('throwpot-soup')


def throwdebuff(case, mode):
    # Hidden by default as in the reference; hidden modules refuse binds.
    case.prepare({'inventory.throw-debuff.mode': mode},
                 {'inventory.throw-debuff': {'visible': True}})
    items = (POISON, POISON, WEAKNESS, HEALING) if mode == 'all' else (WEAKNESS, POISON, POISON)
    for slot, item in enumerate(items):
        case.put(f'hotbar.{slot}', item)
    case.s.call('hotbar', '6')
    case.s.call('look', '0', '-60')
    case.tap('inventory.throw-debuff')
    case.s.call('wait', '40')
    hotbar = case.inventory()['hotbar']
    kept = [i for i in range(len(items)) if hotbar[i].get('item')]
    expected = [3] if mode == 'all' else [1, 2]
    assert kept == expected, (mode, hotbar)
    if mode == 'all':
        assert 'healing' in hotbar[3].get('components', ''), hotbar[3]
    assert case.state()['selected'] == 6
    assert not case.enabled('inventory.throw-debuff')
    case.checks.update(kept_slots=kept)
    case.s.capture('throwdebuff-' + mode)


def refill(case, kind, main):
    case.prepare({'inventory.refill.type': kind})
    for slot in range(6):
        case.put(f'hotbar.{slot}', 'minecraft:stone')
    for slot, item in enumerate(main):
        case.put(f'inventory.{slot}', item)
    case.tap('inventory.refill')
    screens = []
    for _ in range(60):
        screen = case.state()['screen']
        if not screens or screens[-1] != screen:
            screens.append(screen)
        if not case.enabled('inventory.refill') and screen == 'playing':
            break
        case.s.call('wait', '2')
    inventory = case.inventory()
    hotbar = [s.get('item') for s in inventory['hotbar']]
    rest = [s.get('item') for s in inventory['main']]
    assert not case.enabled('inventory.refill')
    case.checks.update(screens=screens, hotbar=hotbar, main=rest)
    return screens, hotbar, rest


def refill_cases(case, which):
    main = [STEW, STEW, HEALING, 'minecraft:stone']
    if which == 'both':
        screens, hotbar, rest = refill(case, 'both', main)
        assert 'inventory' in screens and screens[-1] == 'playing', screens
        assert sorted(filter(None, hotbar[6:])) == sorted(
            ['minecraft:mushroom_stew'] * 2 + ['minecraft:splash_potion']), hotbar
        assert rest.count('minecraft:stone') == 1 and rest.count('minecraft:mushroom_stew') == 0
    elif which == 'pots':
        screens, hotbar, rest = refill(case, 'pots', main)
        assert 'inventory' in screens and screens[-1] == 'playing', screens
        assert hotbar[6:].count('minecraft:splash_potion') == 1, hotbar
        assert 'minecraft:mushroom_stew' not in hotbar
        assert rest.count('minecraft:mushroom_stew') == 2, rest
    else:
        # Pots with only stews in the main inventory: onEnable turns it off
        # without opening the inventory.
        screens, hotbar, rest = refill(case, 'pots', [STEW, STEW])
        assert screens == ['playing'], screens
        assert rest.count('minecraft:mushroom_stew') == 2
    case.s.capture('refill-' + which)


def armor(case):
    parts = case.inventory()['armor']
    return {p['part']: p.get('item') for p in parts}


def armor_trace(case, done, seconds=8):
    """(seconds since start, armor) at each change until done(armor)."""
    start = time.monotonic()
    trace = []
    while time.monotonic() - start < seconds:
        now = armor(case)
        if not trace or trace[-1][1] != now:
            trace.append((round(time.monotonic() - start, 3), now))
        if done(now):
            break
        case.s.call('wait', '1')
    return trace


IRON = {'head': 'minecraft:iron_helmet', 'chest': 'minecraft:iron_chestplate',
        'legs': None, 'feet': 'minecraft:iron_boots'}


def armor_inventory_only(case, delay):
    """Defaults ("Inventory only"): nothing happens while playing; opening
    the inventory equips head, chest, then feet, one click per Delay."""
    case.prepare({'inventory.auto-armor.delay': [delay, delay]})
    case.put('armor.head', 'minecraft:leather_helmet')
    case.put('inventory.0', 'minecraft:iron_boots')
    case.put('inventory.1', 'minecraft:iron_chestplate')
    case.put('inventory.2', 'minecraft:iron_helmet')
    case.tap('inventory.auto-armor')
    case.s.call('wait', '20')
    assert armor(case)['head'] == 'minecraft:leather_helmet'
    assert case.state()['screen'] == 'playing'
    case.s.call('open-inventory')
    trace = armor_trace(case, lambda a: a == IRON)
    assert trace[-1][1] == IRON, trace
    order = []
    for _, now in trace:
        for part, item in now.items():
            if item == IRON[part] and item and part not in order:
                order.append(part)
    assert order == ['head', 'chest', 'feet'], trace
    main = [s.get('item') for s in case.inventory()['main']]
    assert 'minecraft:leather_helmet' in main, main
    case.checks.update(delay=delay, trace=trace, order=order, total=trace[-1][0])
    case.s.capture(f'armor-inventory-only-{delay}')
    case.s.call('key', 'escape', 'tap')
    return trace[-1][0]


def armor_drop(case):
    case.prepare({'inventory.auto-armor.drop-equipped': True})
    case.put('armor.head', 'minecraft:leather_helmet')
    case.put('inventory.0', 'minecraft:iron_helmet')
    case.tap('inventory.auto-armor')
    case.s.call('open-inventory')
    trace = armor_trace(case, lambda a: a['head'] == 'minecraft:iron_helmet')
    inventory = case.inventory()
    assert trace[-1][1]['head'] == 'minecraft:iron_helmet', trace
    assert 'minecraft:leather_helmet' not in inventory['counts'], inventory['counts']
    assert inventory.get('carried') in (None, {}) or not inventory['carried'].get('item')
    entities = [e for e in case.s.call('entities', '8')['entities'] if e.get('kind') == 'item']
    assert any(e.get('item', {}).get('item') == 'minecraft:leather_helmet' for e in entities), entities
    case.checks.update(trace=trace, dropped=entities)
    case.s.capture('armor-drop-equipped')


def armor_open(case):
    """"Inventory only" off: opens the inventory itself, equips, closes."""
    case.prepare({'inventory.auto-armor.inventory-only': False})
    case.put('inventory.0', 'minecraft:iron_boots')
    case.put('inventory.5', 'minecraft:iron_chestplate')
    case.put('inventory.20', 'minecraft:iron_helmet')
    case.tap('inventory.auto-armor')
    screens = []
    start = time.monotonic()
    while time.monotonic() - start < 8:
        screen = case.state()['screen']
        if not screens or screens[-1] != screen:
            screens.append(screen)
        if armor(case) == IRON and screen == 'playing':
            break
        case.s.call('wait', '1')
    assert armor(case) == IRON, armor(case)
    # The first sample can already see the inventory it opened.
    assert screens[-2:] == ['inventory', 'playing'] and len(screens) <= 3, screens
    assert case.enabled('inventory.auto-armor')
    case.checks.update(screens=screens)
    case.s.capture('armor-open-inventory')


def armor_durability(case, check):
    case.prepare({'inventory.auto-armor.inventory-only': False,
                  'inventory.auto-armor.check-durability': check})
    case.put('armor.head', 'minecraft:iron_helmet[minecraft:damage=100]')
    case.put('inventory.0', 'minecraft:iron_helmet')
    case.tap('inventory.auto-armor')
    screens = []
    start = time.monotonic()
    while time.monotonic() - start < 4:
        screen = case.state()['screen']
        if not screens or screens[-1] != screen:
            screens.append(screen)
        case.s.call('wait', '2')
    head = next(p for p in case.inventory()['armor'] if p['part'] == 'head')
    if check:
        assert head.get('item') == 'minecraft:iron_helmet' and not head.get('damage'), head
        assert screens[-2:] == ['inventory', 'playing'] and len(screens) <= 3, screens
        main = case.inventory()['main']
        assert any(s.get('damage') == 100 for s in main), main
    else:
        assert head.get('damage') == 100, head
        assert screens == ['playing'], screens
    case.checks.update(check_durability=check, head=head, screens=screens)
    case.s.capture(f'armor-durability-{check}')


def armor_combat(case):
    """Combat check: a hit holds equipping while the guard lasts; the Text
    GUI's Extended suffix shows "[In Combat]" meanwhile."""
    case.prepare({'inventory.auto-armor.inventory-only': False,
                  'inventory.auto-armor.combat-check': True,
                  'hud.text-gui.suffix': 'Extended'},
                 {'hud.text-gui': {'enabled': True}})
    case.tap('inventory.auto-armor')
    case.s.call('wait', '10')
    case.s.call('command', 'damage @s 1 minecraft:generic')
    hit = time.monotonic()
    case.s.call('command', 'item replace entity @s inventory.0 with minecraft:iron_helmet')
    case.s.call('wait', '8')
    held = armor(case)['head']
    screen = case.state()['screen']
    assert held is None and screen == 'playing', (held, screen)
    case.s.capture('armor-in-combat')
    while time.monotonic() - hit < 8 and armor(case)['head'] is None:
        case.s.call('wait', '1')
    equipped = round(time.monotonic() - hit, 3)
    assert armor(case)['head'] == 'minecraft:iron_helmet'
    # The guard holds 20 ticks, then "Open inventory" waits 200 + Delay ms.
    assert equipped >= 1.2, equipped
    case.checks.update(blocked_after_8_ticks=True, equipped_after_s=equipped)
    case.s.call('wait', '10')
    case.s.capture('armor-combat-released')


def autotool_weapon(case, items, expected):
    case.prepare({'world.auto-tool.binds': ['KeyJ']}, {'world.auto-tool': {'enabled': True}})
    for slot, item in enumerate(items):
        case.put(f'hotbar.{slot}', 'minecraft:' + item)
    case.s.call('hotbar', '1')
    case.command('summon minecraft:armor_stand 0.5 100 3.0')
    case.s.call('look-at', '0.5', '101.1', '3.0')
    case.s.call('mouse', 'left', 'down')
    case.s.call('wait', '12')
    selected = case.state()['selected']
    case.s.call('mouse', 'left', 'up')
    assert selected == expected, (items, selected)
    case.checks.update(items=items, selected=selected)
    case.s.capture('autotool-weapon-' + items[-1])


CASES = {
    'throwpot-dynamic': lambda c: throwpot_pots(c, 'dynamic'),
    'throwpot-single': lambda c: throwpot_pots(c, 'single'),
    'throwpot-both': lambda c: throwpot_budget(c, 'both'),
    'throwpot-pots': lambda c: throwpot_budget(c, 'pots'),
    'throwpot-soup': throwpot_soup,
    'throwdebuff-all': lambda c: throwdebuff(c, 'all'),
    'throwdebuff-first': lambda c: throwdebuff(c, 'first'),
    'refill-both': lambda c: refill_cases(c, 'both'),
    'refill-pots': lambda c: refill_cases(c, 'pots'),
    'refill-pots-precheck': lambda c: refill_cases(c, 'precheck'),
    'armor-delay-200': lambda c: armor_inventory_only(c, 200),
    'armor-delay-1': lambda c: armor_inventory_only(c, 1),
    'armor-drop-equipped': armor_drop,
    'armor-open-inventory': armor_open,
    'armor-durability-off': lambda c: armor_durability(c, False),
    'armor-durability-on': lambda c: armor_durability(c, True),
    'armor-combat-check': armor_combat,
    # Wooden sword: 3 - 2.4 + 0.01 = 0.61, under the 1.0 a bare hand scores.
    'autotool-weapon-wooden': lambda c: autotool_weapon(c, ['stick', 'wooden_sword'], 1),
    # Diamond shovel 4.5 - 3 = 1.5 loses to the stone sword's 1.61.
    'autotool-weapon-stone': lambda c: autotool_weapon(
        c, ['stick', 'wooden_sword', 'diamond_shovel', 'stone_sword'], 4),
}


def run(resources, names):
    summary = {}
    for name in names:
        with NativeSession(resources, f'utility-{name}-') as session:
            case = Case(session)
            CASES[name](case)
            session.close()
            case.passed()
            summary[name] = {'root': str(session.root), **case.checks}
            print(name, 'PASS', json.dumps(case.checks), flush=True)
    if {'armor-delay-200', 'armor-delay-1'} <= summary.keys():
        slow, fast = (summary[n]['total'] for n in ('armor-delay-200', 'armor-delay-1'))
        # Four clicks and the open timer each wait the Delay sample.
        assert slow - fast > 0.6, (slow, fast)
        summary['armor-delay-gap'] = {'delay_200_s': slow, 'delay_1_s': fast}
    out = Path(__file__).resolve().parents[1] / 'target/scenarios' / (
        'utility-debt-summary-' + time.strftime('%Y%m%dT%H%M%S') + '.json')
    out.write_text(json.dumps(summary, indent=2) + '\n')
    print(out)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('cases', nargs='*', help='case names (default: all)')
    args = parser.parse_args()
    unknown = set(args.cases) - CASES.keys()
    if unknown:
        parser.error(f'unknown cases {sorted(unknown)}; choose from {list(CASES)}')
    run(args.resources, args.cases or list(CASES))
