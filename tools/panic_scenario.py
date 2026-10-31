#!/usr/bin/env python3
"""Panic in a fresh isolated world, through its default F10 bind.

Fullbright, Sprint and NoHurtCam start enabled; each has its own bind. Every
check reads the saved module state the runtime writes.

- Re-enable off (the default): F10 disables every module, and a second F10
  leaves them disabled. Panic itself never stays enabled.
- Relaunch with Re-enable on: F10 disables them, and a second F10 restores
  exactly that set.
- The reference's `U(mod)` reset: after a Panic, toggling any other module
  forgets the saved set. The next F10 disables what is enabled now, and the
  one after it restores only that.

Results go to result.json.
"""
import argparse

from movement_scenario import relaunch
from native_session import NativeSession
from smoke_scenario import Checks

BINDS = {'render.fullbright': 'KeyJ', 'combat.sprint': 'KeyK', 'render.no-hurt-camera': 'KeyL'}
MODULES = set(BINDS)


def on(session):
    return {m for m, v in session.saved()['modules'].items() if v.get('enabled')}


def press(session, key):
    session.call('key', key, 'tap')
    session.call('wait', '4')
    return on(session)


def run(resources, allow_stale):
    with NativeSession(resources, 'panic-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {m: {'enabled': True} for m in MODULES},
                      'settings': {f'{m}.binds': [k] for m, k in BINDS.items()}})
        session.launch()
        session.call('wait-ready', '40')
        session.call('wait', '10')
        start = on(session)
        checks.check('the three modules start enabled', start == MODULES, sorted(start))

        # --- Re-enable off (default) -----------------------------------------
        first = press(session, 'f10')
        checks.check('F10 (default bind) disables every module', not first, sorted(first))
        second = press(session, 'f10')
        checks.check('with Re-enable off a second F10 restores nothing', not second, sorted(second))
        checks.check('Panic does not stay enabled', 'utility.panic' not in first | second)

        # --- Re-enable on ----------------------------------------------------
        relaunch(session, {'utility.panic.re-enable': True})
        for m in MODULES:
            press(session, BINDS[m][3:].lower())
        back = on(session)
        checks.check('relaunched with Re-enable on and the modules re-enabled', back == MODULES, sorted(back))
        first = press(session, 'f10')
        checks.check('Re-enable on: F10 disables every module', not first, sorted(first))
        second = press(session, 'f10')
        checks.check('Re-enable on: a second F10 restores exactly that set', second == MODULES, sorted(second))

        # --- Another module's toggle forgets the saved set -----------------
        first = press(session, 'f10')
        checks.check('a third F10 disables them again', not first, sorted(first))
        toggled = press(session, 'j')
        checks.check('Fullbright toggled on through its bind after the Panic',
                     toggled == {'render.fullbright'}, sorted(toggled))
        cleared = press(session, 'f10')
        checks.check('after that toggle F10 disables again rather than restoring', not cleared, sorted(cleared))
        restored = press(session, 'f10')
        checks.check('the next F10 restores only Fullbright, not the older set',
                     restored == {'render.fullbright'}, sorted(restored))

        log = (session.root / 'game.log').read_text(errors='replace') if (session.root / 'game.log').exists() else ''
        checks.check('no panics or runtime poison in the log', 'panicked' not in log and 'poisoned' not in log)
        ok = all(r['ok'] for r in checks.results)
        print(f"{'PASS' if ok else 'FAIL'} {sum(r['ok'] for r in checks.results)}/{len(checks.results)} {session.root}")
        return ok


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('resources')
    parser.add_argument('--allow-stale', action='store_true')
    args = parser.parse_args()
    raise SystemExit(0 if run(args.resources, args.allow_stale) else 1)


if __name__ == '__main__':
    main()
