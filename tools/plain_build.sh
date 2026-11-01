#!/usr/bin/env bash
# A horizon-client without vape-rs, from the pinned pristine export, with the
# modded build's profile and features: the "mod disabled" perf baseline.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
exec 9>target/host-build.lock
flock 9
if [[ ! -d target/plain-cargo ]]; then
  cp -a target/host-cargo target/plain-cargo.tmp
  rm -f target/plain-cargo.tmp/fast/vape-rs-build.json target/plain-cargo.tmp/fast/horizon-client
  mv target/plain-cargo.tmp target/plain-cargo
fi
cd target/horizon/pristine
nice -n 19 cargo build --locked --offline -p horizon-client --no-default-features \
  --bin horizon-client --bin horizon-ctl --profile fast --target-dir ../../plain-cargo
cd "$ROOT"
python3 -I - <<'PY'
import hashlib, json, pathlib
root = pathlib.Path.cwd()
exe = root / 'target/plain-cargo/fast/horizon-client'
plan = json.loads((root / 'compatibility.json').read_text()) if (root / 'compatibility.json').exists() else {}
(root / 'target/plain-cargo/fast/plain-build.json').write_text(json.dumps({
    'schema': 1, 'what': 'horizon-client without vape-rs (pristine pinned export)',
    'horizon_revision': plan.get('horizon_revision'), 'profile': 'fast', 'features': '--no-default-features',
    'executable_sha256': hashlib.sha256(exe.read_bytes()).hexdigest()}, indent=2) + '\n')
print('plain build ok', exe)
PY
