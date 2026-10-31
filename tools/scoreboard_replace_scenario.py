#!/usr/bin/env python3
"""Scoreboard "Replace scoreboard text" host scenario in a fresh world.

A sidebar objective "Vape Board" with three scores is shown by the moved
Scoreboard while the player looks straight down at a stone floor. The
session relaunches with three replacement settings:
- none (the baseline);
- "board" -> "B": at or above 1.16.5 the reference draws the title
  unreplaced but centres it by the replaced text's width, so only the title
  line moves;
- "alpha" -> "Zulu": rows are drawn unreplaced, so nothing changes.
Changed pixels are measured in the left half of the screen, where the panel
sits. Results go to result.json.
"""
import argparse
import os

from PIL import Image, ImageChops

from native_session import NativeSession
from smoke_scenario import Checks
from target_info_scenario import prepare

SETTING = 'hud.scoreboard.replace-text'


def changed(session, a, b, threshold=48):
    """Changed pixel count and their bounding box in the left half."""
    first = Image.open(session.root / (a + '.png')).convert('RGB')
    second = Image.open(session.root / (b + '.png')).convert('RGB')
    box = (0, 0, first.width // 2, first.height * 3 // 5)
    diff = ImageChops.difference(first.crop(box), second.crop(box))
    mask = diff.convert('L').point(lambda v: 255 if v * 3 > threshold else 0)
    return sum(1 for v in mask.getdata() if v), mask.getbbox()


def look_down(session):
    session.call('tp', '0.5', '100', '0.5')
    session.call('look', '0', '90')
    session.call('wait', '20')


def relaunch(session, replacements):
    session.close()
    document = session.saved()
    document['settings'][SETTING] = replacements
    session.seed(document)
    session.launch(creative=True)
    session.call('wait-ready', '40')
    session.call('time', '18000')
    look_down(session)


def run(resources, allow_stale):
    with NativeSession(resources, 'hud-scoreboard-replace-', allow_stale=allow_stale) as session:
        checks = Checks(session.root)
        session.seed({'schema': 1, 'modules': {'hud.scoreboard': {'enabled': True}},
                      'settings': {'hud.scoreboard.x': 8, 'hud.scoreboard.y': 40}})
        session.launch(creative=True)
        prepare(session, checks)
        for command in ('scoreboard objectives add vape dummy "Vape Board"',
                        'scoreboard objectives setdisplay sidebar vape',
                        'scoreboard players set Alpha vape 12',
                        'scoreboard players set Bravo vape 7',
                        'scoreboard players set Charlie vape 3'):
            session.call('command', command)
            session.call('wait', '2')
        relaunch(session, [])
        shown = session.call('status').get('sidebar') or {}
        checks.check('the sidebar shows the objective',
                     shown.get('objective') == 'vape'
                     and shown.get('scores') == {'Alpha': 12, 'Bravo': 7, 'Charlie': 3}, shown)
        session.capture('baseline')
        session.call('wait', '10')
        session.capture('baseline-again')
        steady = changed(session, 'baseline', 'baseline-again')
        checks.check('the view is steady between frames', steady[0] == 0, steady)

        relaunch(session, [['board', 'B']])
        checks.check('the replacement persisted',
                     session.saved()['settings'].get(SETTING) == [['board', 'B']],
                     session.saved()['settings'].get(SETTING))
        session.capture('title-replacement')
        count, box = changed(session, 'baseline', 'title-replacement')
        # One GUI-scale-2 text line is 18 pixels; title and rows are over 70.
        checks.check('only the title line moves', count > 0 and box and box[3] - box[1] <= 20,
                     {'pixels': count, 'bbox': box})

        relaunch(session, [['alpha', 'Zulu']])
        session.capture('row-replacement')
        count, box = changed(session, 'baseline', 'row-replacement')
        checks.check('rows are drawn unreplaced', count == 0, {'pixels': count, 'bbox': box})
        ok = all(r['ok'] for r in checks.results)
        print('PASS' if ok else 'FAIL', f'{sum(r["ok"] for r in checks.results)}/{len(checks.results)}',
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
