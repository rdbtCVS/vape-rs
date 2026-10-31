"""Click GUI row positions, computed as the GUI lays them out rather than
fixed, so a new module in a category does not shift the clicks.

A category frame lists that category's registered modules sorted by
lower-cased name (src/ui/gui/render.rs, module_list). In the first column
the body starts at y=26 with 20-unit rows; the row centre is x=160 and its
settings dots are at x=221. The navigation frame lists CATEGORIES in order
from y=28, rows centred 10 below their top. Modules hidden by default
(src/core/visibility.rs, DEFAULT_HIDDEN) leave their frame, so they take no
row until a scenario unhides them.
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENTRY = re.compile(r'\s*(\w+) => "([^"]+)", "([^"]+)", "([^"]+)"')


def hidden():
    """ModuleId variants hidden by default."""
    path = ROOT / 'src/core/visibility.rs'
    if not path.exists():
        return set()
    body = re.search(r'const DEFAULT_HIDDEN: \[ModuleId; \d+\] = \[(.*?)\];',
                     path.read_text(), re.S).group(1)
    return set(re.findall(r'ModuleId::(\w+)', body))


def modules(category, shown=()):
    """Module ids of a category, in GUI order; `shown` lists default-hidden
    ids the scenario has unhidden."""
    out, skip = [], hidden()
    for line in (ROOT / 'src/core/registry.rs').read_text().splitlines():
        m = ENTRY.match(line)
        if m and m.group(4) == category and (m.group(1) not in skip or m.group(2) in shown):
            out.append((m.group(3).lower(), m.group(2)))
    return [module for _, module in sorted(out)]


def categories():
    text = (ROOT / 'src/ui/gui/render.rs').read_text()
    body = re.search(r'const CATEGORIES: \[&str; \d+\] = \[(.*?)\];', text, re.S).group(1)
    return re.findall(r'"([^"]+)"', body)


def category(name):
    """Centre of a category's navigation row."""
    return 30, 38 + 20 * categories().index(name)


def row(category_name, module, x=160, shown=()):
    """Centre of a module's row in a first-column category frame."""
    return x, 36 + 20 * modules(category_name, shown).index(module)


def dots(category_name, module, shown=()):
    return row(category_name, module, 221, shown)
