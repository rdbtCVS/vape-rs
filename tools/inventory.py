#!/usr/bin/env python3
"""Static discovery only. Never imports or executes the reference checkout.

The discovery JSON is reproducible evidence, not a completed behavioral spec.
Existing ledger decisions/evidence survive regeneration.
"""
import csv
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT.parent / 'Vape-v4/src/main/java/gg/vape'
FIELDS = 'id display_name category reference_paths reference_methods settings_and_defaults visibility_conditions triggers inputs state_machine actions disable_cleanup dependencies conflicts old_version_gate modern_semantics horizon_hook_ids tests status evidence decision'.split()


def slug(value):
    return re.sub(r'[^a-z0-9]+', '-', value.lower()).strip('-')


def discover():
    manager = (REF / 'manager/ModManager.java').read_text()
    names = []
    for section in (manager.split('public void init()', 1)[1].split('public boolean getState', 1)[0],
                    manager.split('private void registerTextGuiSettings()', 1)[1].split('private void registerModuleWithCustomizer', 1)[0],
                    manager.split('private void registerHudModules()', 1)[1].split('public int countEnabledModules', 1)[0]):
        names.extend(re.findall(r'new (\w+)\(', section))
    files = {p.stem: p for p in sorted((REF / 'module').rglob('*.java'))}
    pending = list(dict.fromkeys(n for n in names if n in files))
    result = []
    visited = set()
    while pending:
        name = pending.pop(0)
        if name in visited:
            continue
        visited.add(name)
        path = files[name]
        source = path.read_text()
        # Include inherited settings and recursively constructed module helpers.
        parent = re.search(r'extends\s+(\w+)', source)
        children = [c for c in re.findall(r'new (\w+)\(', source) if c in files and c != name]
        for child in children:
            if child not in visited:
                pending.append(child)
        if parent and parent[1] in files and parent[1] not in visited:
            pending.append(parent[1])
        display = re.search(r'super\("([^"]+)"', source)
        category = re.search(r'Category\.(\w+)', source)
        settings, modes, events, visibility = [], [], [], []
        for line_no, line in enumerate(source.splitlines(), 1):
            entry = {'line': line_no, 'source': line.strip()}
            if re.search(r'\w+Value\.(create|of)\(|new \w+Value\(', line):
                settings.append(entry)
            if re.search(r'new (?:ModeOption|\w+(?:Speed|Fly|Mode))\(', line) or 'getSelectionValue()' in line:
                modes.append(entry)
            if re.search(r'void on\w+\(Event', line):
                events.append(entry)
            if re.search(r'setDefaultVisibility|setVisible|setVisibility|\.addParent|\.setParent', line):
                visibility.append(entry)
        result.append({'class': name, 'display': display[1] if display else name,
                       'category': category[1] if category else 'inherited/HUD; review',
                       'path': str(path.relative_to(REF)), 'sha256': hashlib.sha256(source.encode()).hexdigest(),
                       'parent': parent[1] if parent else None, 'children': children,
                       'settings': settings, 'modes': modes, 'events': events, 'visibility': visibility,
                       'registered': name in names})
    return result


def main():
    entries = discover()
    output = ROOT / 'docs/features.csv'
    old = {}
    if output.exists():
        old = {r['id']: r for r in csv.DictReader(output.open())}
    rows = []
    for entry in entries:
        base = 'module.' + slug(entry['class'])
        row = dict.fromkeys(FIELDS, '')
        row.update(id=base, display_name=entry['display'], category=entry['category'],
                   reference_paths='src/main/java/gg/vape/' + entry['path'], reference_methods='constructor; ' + '; '.join(e['source'] for e in entry['events']),
                   settings_and_defaults=' | '.join(s['source'] for s in entry['settings']),
                   visibility_conditions=' | '.join(s['source'] for s in entry['visibility']),
                   dependencies='parent=' + str(entry['parent']) + '; children=' + ','.join(entry['children']),
                   old_version_gate='Review ModManager registration constraints; retain all modern-useful behavior',
                   status='not-started', decision='Static discovery; behavioral specification incomplete')
        rows.append(old.pop(base, row))
        for index, mode in enumerate(entry['modes']):
            labels = re.findall(r'"([^"\n]+)"', mode['source'])
            mode_id = base + '.mode.' + slug(labels[0] if labels else str(index))
            mode_row = dict.fromkeys(FIELDS, '')
            mode_row.update(id=mode_id, display_name=labels[0] if labels else mode['source'],
                            category=entry['category'], reference_paths=row['reference_paths'] + ':' + str(mode['line']),
                            settings_and_defaults=mode['source'], dependencies=base, status='not-started',
                            decision='Mode candidate; resolve aliases/default/visibility and helper behavior before specification')
            if mode_id not in {r['id'] for r in rows}:
                rows.append(old.pop(mode_id, mode_row))
    for feature, label in {
        'ui': 'Click GUI windows/layout/search/favorites/navigation/animations',
        'settings': 'Typed setting editors, defaults and conditional visibility',
        'bindings': 'Toggle/hold/release/multiple keyboard and mouse binds',
        'macros': 'Macro triggers, actions, sequencing and cancellation',
        'profiles': 'Local profiles: CRUD/import/export/defaults/migrations',
        'friends': 'Local friends/enemies/aliases/team and bot target filters',
        'hud-editor': 'HUD drag/anchors/snapping/scaling/colors',
        'localization': 'Translations, input, fonts and resources',
        'target-info': 'Target/inventory/rotation/trajectory displays',
        'arbitration': 'Rotation/attack/use/hotbar/inventory/rescue claims',
        'timing': 'Deterministic randomized click and rotation timing',
        'ai': 'SilentAura local neural inference and model provenance',
        'notifications': 'Local notifications and profile/toggle feedback',
        'performance': 'Local performance and debug diagnostics',
    }.items():
        row = dict.fromkeys(FIELDS, '')
        row.update(id='client.' + feature, display_name=label, category='Cross-cutting', status='not-started', decision='Needs decomposition and behavioral specification')
        rows.append(old.pop(row['id'], row))
    for feature in ['accounts', 'licensing', 'cloud-profiles', 'public-profiles', 'reviews', 'parties', 'shared-activity', 'shared-inventories', 'remote-sync']:
        row = dict.fromkeys(FIELDS, '')
        row.update(id='online.' + feature, display_name=feature, category='Online product', status='excluded-approved', decision='plan.md sections 1 and 4: explicit user-approved online product removal')
        rows.append(old.pop(row['id'], row))
    rows.extend(old.values())
    with output.open('w', newline='') as stream:
        writer = csv.DictWriter(stream, FIELDS)
        writer.writeheader()
        writer.writerows(rows)
    (ROOT / 'docs/discovery.json').write_text(json.dumps(entries, indent=2) + '\n')
    print(f'{len(entries)} module classes/helpers; {len(rows)} ledger entries. Discovery is not an exhaustive specification.')


if __name__ == '__main__':
    main()
