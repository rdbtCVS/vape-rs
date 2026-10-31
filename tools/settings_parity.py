#!/usr/bin/env python3
"""Compare reference setting declarations (ledger text) with our definitions.

Reads docs/features.csv (settings_and_defaults), src/core/registry.rs and
target/parity/settings.json, written by

    VAPE_RS_SETTINGS_DUMP=$PWD/target/parity/settings.json \
        cargo test --locked --offline --test settings_dump

Text only; the reference sources (../Vape-v4) are read, never run, to tell
registered settings from values the reference creates but never shows
(not passed to addValue/U). Writes target/parity/report.json. Rows with no matching module
are submodes, helpers or not-started rows.
"""
import collections, csv, json, os, re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# A worktree sits away from the checkout; VAPE_RS_REFERENCE names the reference.
REFERENCE = Path(os.environ.get('VAPE_RS_REFERENCE') or ROOT.parent / 'Vape-v4')  # read as text only
norm = lambda s: re.sub(r'[^a-z0-9]', '', s.lower())
# Rows whose settings live outside a registry module.
ALIASES = {'module.clientsettings': 'client'}

def args(text, start):
    """Split the argument list starting at text[start] == '('."""
    depth, cur, out, i, q = 0, '', [], start, None
    while i < len(text):
        c = text[i]
        if q:
            cur += c
            if c == '\\':
                cur += text[i + 1]; i += 1
            elif c == q:
                q = None
        elif c in '"\'':
            q = c; cur += c
        elif c in '([{':
            depth += 1
            if depth > 1: cur += c
        elif c in ')]}':
            depth -= 1
            if depth == 0:
                out.append(cur.strip()); return out
            cur += c
        elif c == ',' and depth == 1:
            out.append(cur.strip()); cur = ''
        else:
            cur += c
        i += 1
    out.append(cur.strip())
    return out

def clean(a):
    a = re.sub(r'^(\((Object|String|boolean|double|int|float|long|ModeSelection|ModeSelection\[\])\)\s*)+', '', a)
    a = re.sub(r'^\(\(Object\)\s*', '', a)
    return a.strip()

def lit(a):
    a = clean(a)
    if a.startswith('"'):
        return a[1:-1]
    if a in ('true', 'false'):
        return a == 'true'
    try:
        return float(a.rstrip('dDfFL'))
    except ValueError:
        return a

def registered(paths):
    """Fields the reference shows: those passed to addValue(...) or U(...).

    None when the source has no such call (nothing to compare against).
    """
    names = set()
    for rel in re.split(r'[;|]\s*', paths):
        source = REFERENCE / rel.strip()
        if not rel.strip() or not source.is_file():
            continue
        text = source.read_text(errors='replace')
        for m in re.finditer(r'\b(?:addValue|U)\s*\(', text):
            names.update(re.findall(r'this\.(\w+)', ','.join(args(text, m.end() - 1))))
    return names or None

OPTION = re.compile(r'ModeOption\s+(\w+)\s*=\s*[^;]*?"([^"]*)"|this\.(\w+)\s*=\s*new ModeOption\s*\(\s*"([^"]*)"')

def options(paths):
    """Mode option fields and their labels, as in `threeDimensionalMode = new ESP3D(this, "3D")`."""
    out = {}
    for rel in re.split(r'[;|]\s*', paths):
        source = REFERENCE / rel.strip()
        if rel.strip() and source.is_file():
            for m in OPTION.finditer(source.read_text(errors='replace')):
                out.setdefault(m.group(1) or m.group(3), m.group(2) if m.group(1) else m.group(4))
    return out

DECL = re.compile(r'\b(\w*Value)\s*\)?\s*\.\s*create\w*\s*\(|\bnew (\w*Value)\s*\(')

def statements(r):
    """Setting declarations: from the reference sources when present,
    else from the ledger's settings_and_defaults text."""
    found = []
    for rel in re.split(r'[;|]\s*', r['reference_paths']):
        source = REFERENCE / rel.strip()
        if rel.strip() and source.is_file():
            text = source.read_text(errors='replace')
            for m in DECL.finditer(text):
                line = text[text.rfind('\n', 0, m.start()) + 1:m.start()]
                found.append((line, text, m))
    if found:
        return found
    for part in r['settings_and_defaults'].split(' | '):
        m = DECL.search(part)
        if m:
            found.append((part[:m.start()], part, m))
    return found

def reference():
    out = {}
    for r in csv.DictReader(open(ROOT / 'docs/features.csv')):
        if r['kind'] not in ('module', 'client', 'submodule'):
            continue
        decls = []
        shown = registered(r['reference_paths'])
        labels = options(r['reference_paths'])
        for line, text, m in statements(r):
            kind = m.group(1) or m.group(2)
            if kind in ('Value', 'ModeValue') and 'create' not in m.group(0) and kind == 'Value':
                continue
            a = [lit(x) for x in args(text, m.end() - 1)]
            a = a[1:] if a and str(a[0]).strip('()').endswith('this') else a  # the owner
            if kind == 'LimitValue' or kind == 'OptionalLimitValue':
                a = a[1:]  # storage id first
            if not a or not isinstance(a[0], str) or not a[0]:
                continue  # label built at runtime; not comparable as text
            d = {'kind': kind, 'label': a[0]}
            field = re.search(r'(?:this\.|\b\w+(?:<[^>]*>)? )(\w+)\s*=\s*(?:\(\w+\)\s*)*$', line)
            d['hidden'] = bool(shown is not None and field and field.group(1) not in shown)
            if kind in ('BooleanValue', 'ClientSettingsBooleanValue'):
                d['default'] = a[1] if len(a) > 1 else None
            elif kind == 'NumberValue' and len(a) >= 7:
                d.update(min=a[3], default=a[4], max=a[5], step=a[6])
            elif kind in ('RandomValue', 'RandomClickDelayValue') and len(a) >= 7:
                d.update(min=a[3], default=[a[4], a[5]], max=a[6], step=a[7] if len(a) > 7 and isinstance(a[7], float) else None)
            elif kind == 'ModeValue' and len(a) >= 2:
                # create(owner, name[, description], default, options...)
                d['default'] = next((x for x in a[1:3] if isinstance(x, str) and x.startswith('this.')), a[1])
                if isinstance(a[1], str) and a[1].startswith('this.'):
                    d['default'] = labels.get(a[1][5:], a[1])
            decls.append(d)
        if decls:
            out[r['id']] = (r['display_name'], r['status'], decls)
    return out

def ours():
    reg = {}
    for line in (ROOT / 'src/core/registry.rs').read_text().splitlines():
        m = re.match(r'\s*(\w+) => "([^"]+)", "([^"]+)"', line)
        if m:
            reg[norm(m.group(3))] = m.group(2)
            reg.setdefault(norm(m.group(1)), m.group(2))
    defs = json.load(open(ROOT / 'target/parity/settings.json'))
    return reg, defs

def main():
    ref = reference()
    reg, defs = ours()
    report = []
    for rid, (name, status, decls) in sorted(ref.items()):
        mid = ALIASES.get(rid) or reg.get(norm(name)) or reg.get(norm(rid.split('.')[-1]))
        if not mid:
            report.append({'row': rid, 'status': status, 'problem': 'no module', 'name': name})
            continue
        mine = [d for d in defs if d['id'].startswith(mid + '.')]
        bylabel = {norm(d['label']): d for d in mine}
        for d in decls:
            o = bylabel.get(norm(d['label']))
            if not o:
                problem = 'hidden in reference' if d['hidden'] else 'missing setting'
                report.append({'row': rid, 'status': status, 'problem': problem, 'label': d['label'], 'kind': d['kind'], 'ref_default': d.get('default')})
                continue
            rd, od = d.get('default'), o['default']
            if rd is None or isinstance(rd, str) and not re.match(r'^[\w .+-]+$', rd):
                continue
            same = rd == od
            if isinstance(rd, float) and isinstance(od, (int, float)):
                same = abs(rd - od) < 1e-9
            if isinstance(rd, list) and isinstance(od, list):
                same = all(abs(float(x) - float(y)) < 1e-9 for x, y in zip(rd, od))
            if isinstance(rd, str) and isinstance(od, str):
                # A field reference such as this.analogMode names the option.
                name = rd[5:] if rd.startswith('this.') else rd
                field = norm(name) if re.fullmatch(r'[a-z]+[A-Z]\w*', name) else None
                same = norm(rd) == norm(od) or bool(field and norm(od) and norm(od) in field)
            if not same:
                report.append({'row': rid, 'status': status, 'problem': 'default differs', 'label': d['label'], 'ref': rd, 'ours': od, 'id': o['id']})
            if (d['kind'] == 'NumberValue' and 'Number' in o['kind']) or (d['kind'].startswith('Random') and 'Range' in o['kind'] and 'min' in d):
                m = re.search(r'min: ([-\d.e]+), max: ([-\d.e]+), step: ([-\d.e]+)', o['kind'])
                if m and (abs(float(m.group(1)) - d['min']) > 1e-9 or abs(float(m.group(2)) - d['max']) > 1e-9):
                    report.append({'row': rid, 'status': status, 'problem': 'range differs', 'label': d['label'], 'ref': [d['min'], d['max'], d['step']], 'ours': [float(x) for x in m.groups()], 'id': o['id']})
    Path(ROOT / 'target/parity/report.json').write_text(json.dumps(report, indent=1))
    print(collections.Counter((r['problem'], r['status']) for r in report).most_common())

main()
