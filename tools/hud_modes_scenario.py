#!/usr/bin/env python3
"""Keystrokes and Clock mode host scenario in a fresh isolated world.

Captures each Keystrokes key/mouse style (keyboard/button, arrow/icon,
no spacebar, CPS only) with W and the left mouse button held, and each Clock
type (analog, digital with date, digital 24 hour without date). Settings are
seeded per launch and checked through the saved document. Results go to
result.json.
"""
import argparse
import os

from native_session import NativeSession
from smoke_scenario import Checks
from target_info_scenario import prepare

CASES = [
    ('keyboard-button-analog', {'hud.keystrokes.key-style': 'keyboard',
                                'hud.keystrokes.mouse-style': 'button',
                                'hud.clock.type': 'analog'}),
    ('arrow-icon-digital-date', {'hud.keystrokes.key-style': 'arrow',
                                 'hud.keystrokes.mouse-style': 'icon',
                                 'hud.clock.type': 'digital', 'hud.clock.date': True}),
    ('no-space-digital-24h-no-date', {'hud.keystrokes.space': False,
                                      'hud.clock.type': 'digital', 'hud.clock.date': False,
                                      'hud.clock.24-hour': True}),
    ('cps-only', {'hud.keystrokes.cps-only': True}),
]

DEFAULTS = {'hud.keystrokes.key-style': 'keyboard', 'hud.keystrokes.mouse-style': 'button',
            'hud.clock.type': 'analog'}


def run(resources, allow_stale):
    with NativeSession(resources, 'hud-modes-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        document = {'schema': 1,
                    'modules': {'hud.keystrokes': {'enabled': True}, 'hud.clock': {'enabled': True}},
                    'settings': {'hud.keystrokes.x': 8, 'hud.keystrokes.y': 40,
                                 'hud.clock.x': 200, 'hud.clock.y': 40}}
        for index, (label, settings) in enumerate(CASES):
            document['settings'].update(settings)
            session.seed(document)
            session.launch(creative=True)
            if index == 0:
                prepare(session, checks)
            else:
                session.call('wait-ready', '40')
            session.call('tp', '0.5', '100', '0.5')
            session.call('look', '0', '30')
            session.call('wait', '10')
            session.capture(f'{label}-idle')
            session.call('key', 'w', 'down')
            session.call('mouse', 'left', 'down')
            session.call('wait', '3')
            session.capture(f'{label}-w-and-left-held')
            session.call('mouse', 'left', 'up')
            session.call('key', 'w', 'up')
            session.call('wait', '2')
            session.close()
            saved = session.saved()
            checks.check(f'{label} settings persisted',
                         all(saved['settings'].get(k, DEFAULTS.get(k)) == v for k, v in settings.items()),
                         {k: saved['settings'].get(k) for k in settings})
            document = saved
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}')
        return ok


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?', default=os.environ.get('HORIZON_RESOURCES'))
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    if not args.resources:
        parser.error('pass the resources directory or set HORIZON_RESOURCES')
    raise SystemExit(0 if run(args.resources, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
