#!/usr/bin/env python3
"""Run only authored native client in fresh homes with caller-supplied resources.

Captures are evidence for manual visual inspection, not pixel-parity assertions.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(resources, in_frame=False):
    resources = resources.resolve(strict=True)
    if json.loads((resources / 'horizon-resources.json').read_text()).get('minecraft') != '26.3':
        raise ValueError('prepared Minecraft 26.3 resources required')
    base = ROOT / 'target/scenarios'
    base.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix='in-frame-' if in_frame else 'integrated-', dir=base))
    env = os.environ.copy()
    for variable, folder in [('HORIZON_HOME', 'game'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                             ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
        path = scratch / folder
        path.mkdir()
        env[variable] = str(path)
    env.update(HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_AUDIO='1')
    ctl = ROOT / 'target/host-cargo/fast/horizon-ctl'
    game = ROOT / 'target/host-cargo/fast/horizon-client'
    prefix = [str(ctl), '--socket', str(scratch / 'ctl.sock')]
    commands = []

    def call(*args, timeout=45):
        result = subprocess.run(prefix + list(args), env=env, cwd=scratch / 'game', text=True, capture_output=True, timeout=timeout)
        commands.append({'args': args, 'code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
        (scratch / 'commands.json').write_text(json.dumps(commands, indent=2))
        if result.returncode:
            raise RuntimeError(f'{args[0]} failed: {result.stdout} {result.stderr}')
        return result.stdout

    print(scratch, flush=True)
    try:
        launch = ['launch', '--game', str(game), '--resources', str(resources), '--world', 'NativeSlice',
                  '--creative', '--offscreen', '--wait', '--width', '800', '--height', '600',
                  '--render-distance', '2', '--no-pause']
        if in_frame:
            launch.append('--in-frame')
        call(*launch, timeout=60)
        (scratch / 'state.json').write_text(call('state'))
        call('screenshot', str(scratch / 'disabled.png'))
        call('key', 'f8', 'down')
        call('key', 'f9', 'tap')
        call('screenshot', str(scratch / 'enabled.png'))
        call('key', 'f8', 'repeat')
        call('screenshot', str(scratch / 'repeat.png'))
        call('key', 'f8', 'up')
        call('key', 'f1', 'tap')
        call('screenshot', str(scratch / 'hidden-hud.png'))
        call('key', 'f1', 'tap')
        call('screenshot', str(scratch / 'restored-hud.png'))
        call('key', 'f10', 'tap')
        call('screenshot', str(scratch / 'panic.png'))
        # GUI input must not toggle a display.
        call('pause')
        call('key', 'f8', 'tap')
        call('resume')
        call('screenshot', str(scratch / 'after-screen.png'))
        print('captured disabled/enabled/repeat/F1/panic/screen states', flush=True)
    finally:
        try:
            call('quit', timeout=15)
        except (RuntimeError, subprocess.TimeoutExpired):
            pass
    return scratch


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    parser.add_argument('--in-frame', action='store_true')
    args = parser.parse_args()
    run(args.resources, args.in_frame)
