#!/usr/bin/env python3
"""Compare reference setting declarations (ledger text) with our definitions.

Reads docs/features.csv (settings_and_defaults), src/core/registry.rs and
target/parity/settings.json, written by

    VAPE_RS_SETTINGS_DUMP=$PWD/target/parity/settings.json \
        cargo test --locked --offline --test settings_dump

Text only; writes target/parity/report.json. Rows with no matching module
are submodes, helpers or not-started rows.
"""
import collections, csv, json, re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
norm = lambda s: re.sub(r'[^a-z0-9]', '', s.lower())

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

def reference():
    out = {}
    for r in csv.DictReader(open(ROOT / 'docs/features.csv')):
        if r['kind'] not in ('module', 'client', 'submodule'):
            continue
        decls = []
        for part in r['settings_and_defaults'].split(' | '):
            m = re.search(r'(\w*Value)\.create\s*\(', part) or re.search(r'new (\w*Value)\s*\(', part)
            if not m:
                continue
            a = [lit(x) for x in args(part, m.end() - 1)]
            kind = m.group(1)
            a = a[1:] if a and (a[0] in ('this', 'Object)this') or str(a[0]).endswith('this')) else a
            if kind == 'LimitValue' or kind == 'OptionalLimitValue':
                a = a[1:]  # storage id first
            if not a or not isinstance(a[0], str) or not a[0]:
                continue  # label built at runtime; not comparable as text
            d = {'kind': kind, 'label': a[0]}
            if kind in ('BooleanValue', 'ClientSettingsBooleanValue'):
                d['default'] = a[1] if len(a) > 1 else None
            elif kind == 'NumberValue' and len(a) >= 7:
                d.update(min=a[3], default=a[4], max=a[5], step=a[6])
            elif kind in ('RandomValue', 'RandomClickDelayValue') and len(a) >= 7:
                d.update(min=a[3], default=[a[4], a[5]], max=a[6])
            elif kind == 'ModeValue' and len(a) >= 2:
                d['default'] = a[1]
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
        mid = reg.get(norm(name)) or reg.get(norm(rid.split('.')[-1]))
        if not mid:
            report.append({'row': rid, 'status': status, 'problem': 'no module', 'name': name})
            continue
        mine = [d for d in defs if d['id'].startswith(mid + '.')]
        bylabel = {norm(d['label']): d for d in mine}
        for d in decls:
            o = bylabel.get(norm(d['label']))
            if not o:
                report.append({'row': rid, 'status': status, 'problem': 'missing setting', 'label': d['label'], 'kind': d['kind'], 'ref_default': d.get('default')})
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
            if d['kind'] == 'NumberValue' and 'Number' in o['kind']:
                m = re.search(r'min: ([-\d.e]+), max: ([-\d.e]+), step: ([-\d.e]+)', o['kind'])
                if m and (abs(float(m.group(1)) - d['min']) > 1e-9 or abs(float(m.group(2)) - d['max']) > 1e-9):
                    report.append({'row': rid, 'status': status, 'problem': 'range differs', 'label': d['label'], 'ref': [d['min'], d['max'], d['step']], 'ours': [float(x) for x in m.groups()], 'id': o['id']})
    Path(ROOT / 'target/parity/report.json').write_text(json.dumps(report, indent=1))
    print(collections.Counter((r['problem'], r['status']) for r in report).most_common())

main()
