#!/usr/bin/env python3
"""Scoreboard HUD host scenario in a fresh isolated world.

Sets up a sidebar objective with three scores, then captures the vanilla
sidebar (module off), the moved sidebar with numbers hidden (module on, the
default) and with Show Score Numbers on, and the GUI preview. The sidebar
contents are checked through the host status, its placement through
screenshots. Results go to result.json.
"""
import argparse
import os

from native_session import NativeSession
from smoke_scenario import Checks
from target_info_scenario import prepare


def sidebar(session):
    return session.call('status').get('sidebar')


def relaunch(session, enabled, numbers):
    session.close()
    document = session.saved()
    document['modules']['hud.scoreboard'] = {'enabled': enabled}
    document['settings']['hud.scoreboard.show-score-numbers'] = numbers
    session.seed(document)
    session.launch(creative=True)
    session.call('wait-ready', '40')
    session.call('wait', '10')


def run(resources, allow_stale):
    with NativeSession(resources, 'hud-scoreboard-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {'hud.scoreboard': {'enabled': False}},
                      'settings': {'hud.scoreboard.x': 8, 'hud.scoreboard.y': 40}})
        session.launch(creative=True)
        prepare(session, checks)
        for command in ('/scoreboard objectives add vape dummy "Vape Board"',
                        '/scoreboard objectives setdisplay sidebar vape',
                        '/scoreboard players set Alpha vape 12',
                        '/scoreboard players set Bravo vape 7',
                        '/scoreboard players set Charlie vape 3'):
            session.call('command', command[1:])
            session.call('wait', '2')
        session.call('wait', '10')
        shown = sidebar(session) or {}
        checks.check('the sidebar shows the objective',
                     shown.get('objective') == 'vape'
                     and shown.get('scores') == {'Alpha': 12, 'Bravo': 7, 'Charlie': 3}, shown)
        session.capture('vanilla-sidebar-module-off')

        relaunch(session, True, False)
        checks.check('the objective survives the relaunch',
                     (sidebar(session) or {}).get('objective') == 'vape', sidebar(session))
        session.capture('moved-numbers-hidden')
        session.call('key', 'rshift', 'tap')
        session.capture('gui-preview')
        session.call('key', 'escape', 'tap')

        relaunch(session, True, True)
        checks.check('Show Score Numbers persisted',
                     session.saved()['settings'].get('hud.scoreboard.show-score-numbers') is True)
        session.capture('moved-numbers-shown')
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
