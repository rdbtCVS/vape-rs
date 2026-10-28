#!/usr/bin/env python3
"""Open the development client in a fresh, visible, isolated test instance."""
import argparse
from pathlib import Path
import subprocess

from native_session import NativeSession, ROOT

DEFAULT_RESOURCES = Path(
    '<launcher-dir>/versions/'
    'nightly-2026-10-28-9b772b2/.local/horizon-client/26.3'
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', nargs='?', type=Path, default=DEFAULT_RESOURCES,
                        help='prepared Minecraft 26.3 resources (read-only)')
    parser.add_argument('--in-frame', action='store_true',
                        help='use the alternative in-frame single-player path')
    parser.add_argument('--creative', action='store_true',
                        help='start in creative; inventory automation requires survival')
    args = parser.parse_args()
    binary = ROOT / 'target/host-cargo/fast/horizon-client'
    if not binary.is_file():
        parser.error('build first: cargo run -p xtask --locked --offline -- build')
    try:
        session = NativeSession(args.resources, prefix='manual-', in_frame=args.in_frame)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    with session:
        command = [str(binary), '--resources', str(session.resources),
                   '--world', 'VapeManual', '--width', '1280', '--height', '720',
                   '--render-distance', '6', '--control-socket', str(session.socket)]
        if args.in_frame:
            command.append('--in-frame')
        if args.creative:
            command.append('--creative')
        print('Right Shift: GUI; F8/F9: coordinates/FPS; F10: panic.', flush=True)
        print(f'Log: {session.root / "game.log"}', flush=True)
        print('Fresh survival world; quit normally to finish. Test files are retained.', flush=True)
        session.log = (session.root / 'game.log').open('a')
        session.process = subprocess.Popen(
            command, cwd=session.root / 'game', env=session.env,
            stdin=subprocess.DEVNULL, stdout=session.log, stderr=subprocess.STDOUT,
        )
        try:
            session.process.wait()
        except KeyboardInterrupt:
            # NativeSession closes only its owned process, through its socket
            # when available, and keeps this disposable instance for inspection.
            pass


if __name__ == '__main__':
    main()
