"""Owned native game process with disposable working directory and homes."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target/host-cargo/fast/horizon-client'
CTL = ROOT / 'target/host-cargo/fast/horizon-ctl'
BUILD_MANIFEST = ROOT / 'target/host-cargo/fast/vape-rs-build.json'
# Fields that must match the current sources; commit/dirty are informational.
IDENTITY_KEYS = ('bundle_sha256', 'horizon_revision', 'horizon_mixins_revision',
                 'horizon_mixins_patches', 'profile', 'features')


class StaleBuild(RuntimeError):
    pass


def verify_build(allow_stale=False):
    """Return the executable's build identity after checking it against the
    current sources. Raises StaleBuild unless allow_stale is set."""
    if not BINARY.is_file():
        raise StaleBuild('no executable; build first: cargo run -p xtask --locked --offline -- build')
    problems = []
    try:
        built = json.loads(BUILD_MANIFEST.read_text())
    except (OSError, ValueError):
        built = {}
        problems.append('no build identity beside the executable (built before identities, '
                        'or the last build failed)')
    if built:
        for path, key in ((BINARY, 'executable_sha256'), (CTL, 'control_sha256')):
            digest = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
            if built.get(key) != digest:
                problems.append(f'{path.name} differs from the one its identity describes')
        current = json.loads(subprocess.run(
            ['cargo', 'run', '-q', '-p', 'xtask', '--locked', '--offline', '--', 'identity'],
            cwd=ROOT, check=True, capture_output=True, text=True).stdout)
        for key in IDENTITY_KEYS:
            if built.get(key) != current.get(key):
                problems.append(f'{key}: built {built.get(key)!r}, sources {current.get(key)!r}')
    if problems and not allow_stale:
        raise StaleBuild('executable does not match current sources; rebuild with '
                         'cargo run -p xtask --locked --offline -- build\n  ' + '\n  '.join(problems))
    for problem in problems:
        print(f'warning: stale build: {problem}', flush=True)
    return {**built, 'stale': problems}


class NativeSession:
    def __init__(self, resources, prefix='scenario-', allow_stale=False):
        self.build = verify_build(allow_stale)
        self.resources = Path(resources).resolve(strict=True)
        if json.loads((self.resources / 'horizon-resources.json').read_text()).get('minecraft') != '26.3':
            raise ValueError('prepared Minecraft 26.3 resources required')
        base = ROOT / 'target/scenarios'
        base.mkdir(parents=True, exist_ok=True)
        self.root = Path(tempfile.mkdtemp(prefix=prefix, dir=base))
        self.env = os.environ.copy()
        for variable, folder in [('HORIZON_HOME', 'game'), ('HORIZON_LAUNCHER_HOME', 'launcher'),
                                 ('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config')]:
            path = self.root / folder
            path.mkdir()
            self.env[variable] = str(path)
        # Single player always runs on Horizon's integrated server thread; an
        # inherited environment must not switch that off.
        self.env.update(HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_AUDIO='1',
                        HORIZON_SERVER_THREAD='1')
        self.socket = self.root / ('ctl.json' if os.name == 'nt' else 'ctl.sock')
        self.process = None
        self.log = None
        self.commands = []
        self.current = self.root / 'game/native-mods/vape-rs/current.json'
        (self.root / 'build.json').write_text(json.dumps(self.build, indent=2) + '\n')
        print(self.root, flush=True)

    def __enter__(self):
        return self

    def __exit__(self, kind, value, traceback):
        try:
            self.close()
        except Exception:
            if kind is None:
                raise

    def call(self, *args, timeout=45):
        result = subprocess.run([str(CTL), '--socket', str(self.socket), *args],
                                cwd=self.root / 'game', env=self.env, text=True,
                                capture_output=True, timeout=timeout)
        self.commands.append({'args': args, 'code': result.returncode,
                              'stdout': result.stdout, 'stderr': result.stderr})
        (self.root / 'commands.json').write_text(json.dumps(self.commands, indent=2))
        if result.returncode:
            raise RuntimeError(f'{args}: {result.stdout} {result.stderr}')
        return json.loads(result.stdout)

    def launch(self, creative=True, world='NativeScenario', extra=()):
        if self.process is not None:
            raise RuntimeError('close the owned process before restarting')
        args = [str(BINARY), '--resources', str(self.resources),
                *(('--world', world) if world else ()), '--offscreen', '--width', '800', '--height', '600',
                '--render-distance', '2', '--no-pause', '--control-socket', str(self.socket),
                *extra]
        if creative:
            args.append('--creative')
        self.socket.unlink(missing_ok=True)
        self.log = (self.root / 'game.log').open('a')
        self.process = subprocess.Popen(args, cwd=self.root / 'game', env=self.env,
                                        stdin=subprocess.DEVNULL, stdout=self.log,
                                        stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 30
        while not self.socket.exists():
            if self.process.poll() is not None:
                raise RuntimeError(f'game exited: {self.root / "game.log"}')
            if time.monotonic() > deadline:
                raise TimeoutError('owned game did not open its control socket')
            time.sleep(0.1)
        self.call('wait-ready', '40')
        self.call('wait', '2')

    def close(self):
        if self.process is None:
            return
        process = self.process
        try:
            if process.poll() is None:
                self.call('quit', timeout=15)
                process.wait(timeout=15)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            self.log.close()
            self.process = None
        if process.returncode:
            raise RuntimeError(f'owned game exited with {process.returncode}')

    def capture(self, name):
        self.call('screenshot', str(self.root / (name + '.png')))

    def seed(self, document):
        self.current.parent.mkdir(parents=True, exist_ok=True)
        self.current.write_text(json.dumps(document, indent=2) + '\n')

    def saved(self):
        return json.loads(self.current.read_text())
