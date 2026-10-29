#!/usr/bin/env python3
"""Resolve ledger rows whose category static discovery left as
'inherited/HUD; review', and give every row a `kind`.

Reads the Java reference as text only (never builds or runs it):
- the `extends` chain of each class, up to an ancestor that passes a
  `Category.X` to its super constructor;
- the classes ModManager instantiates (independent registered modules);
- HudModule descendants (the HUD family registered by registerHudModules).

Kinds: module (registered, user-visible), submode (a ModeOption of a module),
submodule (a SubModule/Mod child owned by a module), helper (GUI component,
controller, packet, thread or rule class whose behavior belongs to its owning
feature row), client / online (cross-cutting rows, unchanged). Helper rows are
not separate deliverables: their behavior is tracked by the owner row.

Usage: python3 -I tools/ledger_triage.py [--write]
"""
import csv
import collections
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent / 'Vape-v4/src/main/java/gg/vape'
LEDGER = ROOT / 'docs/features.csv'
INHERITED = 'inherited/HUD; review'
MODULE_BASES = {'Mod', 'SubModule', 'HudModule', 'UtilityMod', 'ClickerMod', 'ModBendable',
                'ConfigSettingsModule'}


def index():
    classes = {}
    for path in JAVA.rglob('*.java'):
        text = path.read_text(errors='replace')
        for m in re.finditer(r'\bclass\s+(\w+)(?:<[^>{]*>)?\s+extends\s+(\w+)', text):
            classes.setdefault(m.group(1), {'parent': m.group(2), 'path': path, 'text': text})
        for m in re.finditer(r'\bclass\s+(\w+)\b(?![^{]*\bextends\b)', text):
            classes.setdefault(m.group(1), {'parent': None, 'path': path, 'text': text})
    return classes


def chain(classes, name):
    seen = []
    while name and name not in seen:
        seen.append(name)
        name = classes.get(name, {}).get('parent')
    return seen


def category(classes, name):
    for cls in chain(classes, name):
        text = classes.get(cls, {}).get('text', '')
        found = re.findall(r'super\([^;]*?Category\.([A-Z]+)', text)
        if found:
            return found[0], cls
    return None, None


def main(write):
    classes = index()
    manager = (JAVA / 'manager/ModManager.java').read_text(errors='replace')
    registered = set(re.findall(r'new\s+(\w+)\s*\(', manager))
    rows = list(csv.DictReader(LEDGER.open()))
    fields = list(rows[0].keys())
    if 'kind' not in fields:
        fields.insert(fields.index('category') + 1, 'kind')
    by_id = {r['id']: r for r in rows}
    changed = collections.Counter()

    def resolve(row):
        cls = Path(row['reference_paths'].split(';')[0].split(':')[0]).stem
        lineage = chain(classes, cls)
        if '.mode.' in row['id']:
            owner = by_id.get(row['dependencies'].split(';')[0].strip())
            owner_cat = owner and (owner['category'] if owner['category'] != INHERITED
                                   else resolve(owner)[1])
            return 'submode', owner_cat or 'OTHER', f'mode of {owner and owner["id"]}'
        if 'HudModule' in lineage[1:] or cls == 'HudModule':
            return ('module' if cls != 'HudModule' else 'helper'), 'HUD', \
                f'HudModule descendant ({" -> ".join(lineage)})'
        cat, source = category(classes, cls)
        if cls in registered:
            return 'module', cat or 'OTHER', \
                f'registered in ModManager; Category.{cat} from {source}' if cat else \
                'registered in ModManager; no Category found in its extends chain'
        if MODULE_BASES & set(lineage[1:]):
            owner = row['dependencies'].split(';')[0].strip() or 'owning module'
            return 'submodule', cat or 'OTHER', \
                f'unregistered {lineage[1]} child; behavior owned by {owner}'
        owner = row['dependencies'].split(';')[0].strip()
        base = lineage[1] if len(lineage) > 1 else 'no Mod base'
        return 'helper', 'helper', f'{base} helper class, not a separate feature' + \
            (f'; owned by {owner}' if owner and not owner.startswith('parent=') else '')

    for row in rows:
        if row['category'] == INHERITED:
            kind, cat, why = resolve(row)
            row['kind'], row['category'] = kind, cat
            note = f'Triage 2026-10-29: {kind}; {why}.'
            row['decision'] = f"{row['decision']} {note}".strip()
            changed[kind] += 1
        elif not row.get('kind'):
            row['kind'] = ('submode' if '.mode.' in row['id'] else
                           row['id'].split('.')[0] if row['id'].split('.')[0] in ('client', 'online')
                           else 'module')
    print('resolved:', dict(changed))
    remaining = collections.Counter((r['kind'], r['status']) for r in rows)
    for key, count in sorted(remaining.items()):
        print(f'  {key[0]:10} {key[1]:24} {count}')
    if write:
        with LEDGER.open('w', newline='') as out:
            writer = csv.DictWriter(out, fieldnames=fields, lineterminator='\n')
            writer.writeheader()
            writer.writerows(rows)


if __name__ == '__main__':
    main('--write' in sys.argv)
