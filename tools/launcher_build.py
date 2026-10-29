#!/usr/bin/env python3
"""Build a .hnmod the way Horizon's launcher does (launcher/src/native_mods.rs,
`build` at the pinned revision), without touching any launcher or game home.

Steps, as the launcher takes them: unpack the bundle with the registry's path
rules; `cargo install` horizon-mixins at the bundle's `mixins` commit from its
published repository; download GitHub's archive of the bundle's Horizon commit;
`horizon-mixins apply`; then `cargo build --locked --profile release` of
horizon-client with default features (plus `horizon-client/mods` when that
revision has it). Everything stays under target/launcher-build. The build
identity beside the executable lets tools/native_session.py run it
(VAPE_RS_BUILD=launcher).

Usage: python3 -I tools/launcher_build.py [BUNDLE.hnmod] [--toolchain 1.99.1]
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import time
import tomllib
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'target/launcher-build'
MIXINS_GIT = 'https://github.com/horizon-rust/horizon-mixins'
HORIZON_REPO = 'horizon-rust/horizon'
# launcher/src/rust_toolchain.rs VERSION at the pinned Horizon revision.
TOOLCHAIN = '1.99.1'
IDENTITY = OUT / 'vape-rs-build.json'
BINARY = OUT / 'target/release/horizon-client'


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def revision(text):
    return len(text) == 40 and all(c in '0123456789abcdef' for c in text)


def unpack(bundle, into):
    """horizon_mods::index::native::Bundle::read, then installed_mods' unpack."""
    files = {}
    with zipfile.ZipFile(bundle) as archive:
        if len(archive.infolist()) > 512:
            raise SystemExit('native bundle has too many entries')
        for info in archive.infolist():
            if info.is_dir():
                continue
            name, mode = info.filename, info.external_attr >> 16
            if ((mode & 0o170000) not in (0, 0o100000) or '\\' in name or ':' in name
                    or name.startswith('/') or any(p in ('', '.', '..') for p in name.split('/'))):
                raise SystemExit(f'invalid native bundle path: {name}')
            files[name] = archive.read(info)
    if sum(map(len, files.values())) > 64 << 20:
        raise SystemExit('native bundle expands beyond 64 MiB')
    native = json.loads(files['horizon-native.json'])
    if (native.get('schema') != 1 or not revision(native.get('horizon', ''))
            or not revision(native.get('mixins', ''))):
        raise SystemExit('invalid native manifest')
    prefix = f'mixins/{native["id"]}/'
    for name in files:
        if name != 'horizon-native.json' and not (name.startswith(prefix) and name.endswith(('.rs', '.toml'))):
            raise SystemExit(f'unexpected native source: {name}')
    source = tomllib.loads(files[prefix + 'horizon-mixin.toml'].decode())
    if (source['mod']['id'], source['mod']['version'], source['compatibility']['horizon_revision']) != (
            native['id'], native['version'], native['horizon']):
        raise SystemExit('native manifest and mixin identity/revision differ')
    if into.exists():
        shutil.rmtree(into)
    for name, data in files.items():
        path = into / native['id'] / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    return native, files


def run(command, **kwargs):
    print('+', ' '.join(map(str, command)), flush=True)
    subprocess.run(command, check=True, **kwargs)


def tool(toolchain, rev):
    root = OUT / 'tools' / rev[:12]
    binary = root / 'bin/horizon-mixins'
    if not binary.is_file():
        run(['cargo', f'+{toolchain}', 'install', '--locked', '--git', MIXINS_GIT, '--rev', rev,
             '--root', root, 'horizon-mixins-cli'])
    return binary


def source(rev):
    """GitHub's archive of the commit; only files and folders are unpacked."""
    folder = OUT / 'source' / rev
    if (folder / '.horizon-revision').is_file():
        return folder
    url = f'https://codeload.github.com/{HORIZON_REPO}/tar.gz/{rev}'
    print(f'downloading {url}', flush=True)
    with urllib.request.urlopen(url) as response:
        data = response.read()
    partial = OUT / 'source' / f'.{rev}.partial'
    if partial.exists():
        shutil.rmtree(partial)
    partial.mkdir(parents=True)
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
        members = [m for m in archive.getmembers() if m.isfile() or m.isdir()]
        archive.extractall(partial, members=members, filter='data')
    top = next(p for p in partial.iterdir() if p.is_dir())
    (top / '.horizon-revision').write_text(rev)
    if folder.exists():
        shutil.rmtree(folder)
    top.rename(folder)
    shutil.rmtree(partial)
    return folder


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('bundle', type=Path, nargs='?')
    parser.add_argument('--toolchain', default=TOOLCHAIN)
    args = parser.parse_args()
    bundle = args.bundle or max((ROOT / 'dist').glob('*.hnmod'), key=lambda p: p.stat().st_mtime)
    OUT.mkdir(parents=True, exist_ok=True)
    IDENTITY.unlink(missing_ok=True)
    installed = OUT / 'installed.incoming'
    native, files = unpack(bundle, installed)
    # build(): the bundle folder is mixins/<id> of the unpacked .hnmod, copied in.
    mods = OUT / 'mods.incoming'
    if mods.exists():
        shutil.rmtree(mods)
    shutil.copytree(installed / native['id'] / 'mixins' / native['id'], mods / native['id'])
    started = time.monotonic()
    horizon_mixins = tool(args.toolchain, native['mixins'])
    tree = source(native['horizon'])
    stage = OUT / 'stage'
    if stage.exists():
        shutil.rmtree(stage)
    run([horizon_mixins, 'apply', '--workspace', tree, '--mods', mods, '--out', stage.resolve()])
    command = ['cargo', f'+{args.toolchain}', 'build', '--locked', '--profile', 'release']
    manifest = tomllib.loads((stage / 'crates/horizon-client/Cargo.toml').read_text())
    features = 'horizon-client/mods' if 'mods' in manifest.get('features', {}) else None
    if features:
        command += ['--features', features]
    command += ['-p', 'horizon-client', '--bin', 'horizon-client']
    run(command, cwd=stage, env={**os.environ, 'CARGO_TARGET_DIR': str(OUT / 'target')})
    # Same digest as xtask's bundle_sha256 over target/bundle.
    tree_hashes = {name: sha256(data) for name, data in sorted(files.items())}
    identity = {
        'schema': 1,
        'route': 'launcher',
        'hnmod': bundle.name,
        'hnmod_sha256': sha256(bundle.read_bytes()),
        'bundle_sha256': sha256(json.dumps(tree_hashes, separators=(',', ':')).encode()),
        'horizon_revision': native['horizon'],
        'horizon_mixins_revision': native['mixins'],
        'profile': 'release',
        'features': features,
        'toolchain': subprocess.run(['cargo', f'+{args.toolchain}', '--version'], check=True,
                                    capture_output=True, text=True).stdout.strip(),
        'executable_sha256': sha256(BINARY.read_bytes()),
        'seconds': round(time.monotonic() - started),
    }
    IDENTITY.write_text(json.dumps(identity, indent=2) + '\n')
    print(f'{BINARY}\nbuild identity: {IDENTITY}')


if __name__ == '__main__':
    main()
