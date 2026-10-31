#!/usr/bin/env python3
"""Deterministic source-only development bundle; no game data is ever packaged."""
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]


PINS = ('horizon_revision', 'horizon_mixins_revision', 'minecraft', 'protocol')


def matrix_entry(compatibility, version):
    """The version's compatibility-matrix entry; stable releases never change pins."""
    matrix = json.loads((ROOT / 'compatibility-matrix.json').read_text())
    entries = [entry for entry in matrix['releases'] if entry['version'] == version]
    if len(entries) != 1:
        raise ValueError(f'compatibility-matrix.json needs exactly one entry for {version}')
    entry = entries[0]
    moved = [key for key in PINS if entry[key] != compatibility[key]]
    if moved and entry['channel'] != 'development':
        raise ValueError(f'{version} is published with other pins ({", ".join(moved)}); bump the version')
    if moved:
        raise ValueError(f'update the development entry in compatibility-matrix.json: {", ".join(moved)}')
    return entry


def bundle():
    compatibility = json.loads((ROOT / 'compatibility.json').read_text())
    if compatibility.get('horizon_mixins_patches'):
        raise ValueError('Distribution needs a published mixin tool pin containing the development patches. Use xtask stage for local development; a base-tool .hnmod would be incompatible.')
    subprocess.run(['cargo', 'run', '-p', 'xtask', '--locked', '--offline', '--', 'pack'], cwd=ROOT, check=True)
    base = ROOT / 'target/bundle'
    native = json.loads((base / 'horizon-native.json').read_text())
    release = matrix_entry(compatibility, native['version'])
    files = sorted(path for path in base.rglob('*') if path.is_file())
    if len(files) > 512 or sum(path.stat().st_size for path in files) > 64 * 1024 * 1024:
        raise ValueError('native reader entry/expanded-size limit exceeded')
    for path in files:
        relative = path.relative_to(base)
        if path.is_symlink() or (str(relative) != 'horizon-native.json' and
                (not str(relative).startswith(f'mixins/{native["id"]}/') or path.suffix not in ('.rs', '.toml'))):
            raise ValueError(f'unexpected native file: {relative}')
    output = ROOT / 'dist' / f'{native["id"]}-{native["version"]}.hnmod'
    output.parent.mkdir(exist_ok=True)
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in files:
            info = zipfile.ZipInfo(str(path.relative_to(base)), date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, path.read_bytes())
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix('.hnmod.sha256').write_text(f'{digest}  {output.name}\n')
    metadata = {
        'schema': 1,
        'artifact': output.name,
        'sha256': digest,
        'id': native['id'],
        'version': native['version'],
        'channel': release['channel'],
        **{key: compatibility[key] for key in PINS},
        'rust': compatibility['rust'],
        'mixin_rust': compatibility['mixin_rust'],
        **{key: release[key] for key in ('validated_platforms', 'exercised_platforms', 'unvalidated_platforms')},
    }
    output.with_suffix('.compatibility.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(f'{output}: {len(files)} source files, SHA-256 {digest}')
    return output


if __name__ == '__main__':
    bundle()
