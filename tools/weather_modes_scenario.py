#!/usr/bin/env python3
"""Weather's two modes in a fresh isolated creative world on the integrated
server, measured from the host's frames like legit_render_scenario.

* Raining under a clear server sky: the sky frame greys while the server
  stays clear; off, the clear sky (the strengths saved on enable) is back.
* Clear under a raining server sky (relaunched with the mode seeded): the
  rainy frame clears while the server keeps raining; off, the rain is back.

Results go to result.json and measurements.json, frames next to them.
"""
import argparse
import json
import os

from legit_render_scenario import Frames, diff, luma
from native_session import NativeSession
from smoke_scenario import Checks

BINDS = {'legit.weather': 'KeyK'}


def prepare(session, r):
    session.call('time', '6000')
    session.call('chat', '/gamerule advance_time false')
    session.call('chat', '/gamerule advance_weather false')
    session.call('chat', '/difficulty peaceful')
    r.wait(20)
    r.arena()


def mode(r, name, server, other):
    """Frames of the sky under `server` weather with Weather off, on and off
    again; `other` is the server weather the mode should look like."""
    r.reset()
    r.call('look', '0', '-45')
    r.call('chat', f'/weather {other}')
    # The client's rain strength moves 0.01 a tick.
    like = r.frame(f'{name}-server-{other}', settle=140)
    r.call('chat', f'/weather {server}')
    off = r.frame(f'{name}-off', settle=140)
    r.toggle('legit.weather', True)
    on = r.frame(f'{name}-on', settle=20)
    flag = r.call('state')['weather']
    r.toggle('legit.weather', False)
    after = r.frame(f'{name}-after', settle=20)
    m = {'sky_luma_like_off_on_after': [luma(like), luma(off), luma(on), luma(after)],
         'diff_off_on': diff(off, on), 'diff_on_like': diff(on, like), 'diff_off_after': diff(off, after),
         'server_weather_while_on': flag}
    r.m[name] = m
    return m


def run(resources, allow_stale):
    with NativeSession(resources, 'weather-modes-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {},
                      'settings': {'legit.weather.binds': ['KeyK'], 'legit.weather.weather': 'Raining'}})
        session.launch(creative=True)
        r = Frames(session, checks, BINDS)
        prepare(session, r)
        m = mode(r, 'raining', 'clear', 'rain')
        checks.check('Raining: the clear sky frame turns rainy (changed, like the server rain frame)',
                     m['diff_off_on'] > 8 and m['diff_on_like'] < m['diff_off_on'] * 0.3, m)
        checks.check('Raining: the server weather itself stays clear',
                     not m['server_weather_while_on'].get('raining'), m['server_weather_while_on'])
        checks.check('Raining disabled: the clear sky is back', m['diff_off_after'] < m['diff_off_on'] * 0.3, m)
        session.call('quit')
        session.process.wait(timeout=15)
        session.log.close()
        session.process = None
        doc = session.saved()
        doc['settings']['legit.weather.weather'] = 'Clear'
        session.seed(doc)
        session.launch(creative=True)
        r.wait(20)
        checks.check('Clear mode seeded', session.saved()['settings'].get('legit.weather.weather') == 'Clear')
        m = mode(r, 'clear', 'rain', 'clear')
        checks.check('Clear: the rainy sky frame clears (changed, like the server clear frame)',
                     m['diff_off_on'] > 8 and m['diff_on_like'] < m['diff_off_on'] * 0.3, m)
        checks.check('Clear: the server keeps raining', m['server_weather_while_on'].get('raining'),
                     m['server_weather_while_on'])
        checks.check('Clear disabled: the rain is back', m['diff_off_after'] < m['diff_off_on'] * 0.3, m)
        log = (session.root / 'game.log').read_text(errors='replace')
        checks.check('no panics in the log', 'panicked' not in log and 'poisoned' not in log)
        (session.root / 'measurements.json').write_text(json.dumps(r.m, indent=2, default=str))
        ok = all(c['ok'] for c in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(c["ok"] for c in checks.results)}/{len(checks.results)}',
              session.root)
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
