# Development guide

Use Rust 1.99.0 (rust-toolchain.toml), Git and tar; Python 3 is used only for inventory/ZIP development tools. First populate approved dependencies with `cargo fetch --locked`, then:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo run -p xtask --locked --offline -- payload-test
```

The AST packer reads conventional Rust module files under src and emits one source-only payload plus a content-hash source map under target. The emitted payload's tests compile in an isolated Cargo fixture with its committed lockfile. The runtime uses serde_json 1.0.151 and the native clock adapter uses chrono, both already declared and locked in the pinned host; staging verifies that contract and never injects a dependency.

The mixin tool is pinned to published, unpatched horizon-mixins main (`07dab17`, compatibility.json), the same commit the `.hnmod` names, so the launcher builds exactly what development stages. The published tool can't add a field to `Game<'_>`, so the runtime is owned per thread by the `client-owner` support module; only the window thread's client Game uses it.

For native staging, review the pinned mixin sources before building them:

```sh
git clone https://github.com/horizon-rust/horizon-mixins target/upstream/horizon-mixins
git -C target/upstream/horizon-mixins checkout --detach 07dab17387480ac03829584d9d1ee9912c9bb515
cargo +1.95.0 fetch --locked --manifest-path target/upstream/horizon-mixins/Cargo.toml
cargo run -p xtask --locked --offline -- stage ../horizon
cargo fetch --locked --manifest-path target/horizon/modded/Cargo.toml
cargo run -p xtask --locked --offline -- check ../horizon
cargo run -p xtask --locked --offline -- build ../horizon
```

Staging exports the committed Horizon pin from compatibility.json, ignores the original working tree, checks pristine source hashes, runs the pinned planner/transformer and audits the generated changes. Wrong pins, source drift, unexpected edits and stale bundle files fail loudly. All exports/builds remain under target. `check` type-checks the actual client binary; `test` compiles/runs embedded runtime tests; `build` compiles the client using Horizon's fast profile with audio/controllers disabled. These execute reviewed native Rust and its build scripts with normal user privileges.

Run the binary only with your own compatible prepared 26.3 resources and fresh scratch homes, as required by Horizon's AGENTS.md. Never point HORIZON_HOME at real player data for tests. See [docs/hooks.md](hooks.md) for integration boundaries. Right Shift opens the custom local GUI. Its pages expose module settings/binding capture, local profiles, local friend/enemy records and a local macro editor. F8/F9 are default HUD bindings and F10 disables all active modules. Modules start disabled; saved preferences restore on restart. Text rendering uses the host atlas; reference typography/layout parity remains unverified.

The compatible prepared resource directory located for this workspace is:

```text
<launcher-dir>/versions/nightly-2026-10-28-9b772b2/.local/horizon-client/26.3
```

Use it read-only; every executable game run must use fresh isolated homes and working directory through tools/native_session.py. `xtask build` writes `target/host-cargo/fast/vape-rs-build.json` beside the executable (bundle hash, Horizon/mixin pins, profile/features, cargo version, executable hash, source commit/dirty flag). `play.py` and every scenario refuse to launch an executable whose identity is missing or differs from the current sources (`--allow-stale` overrides with a warning), and copy the identity into the session as build.json. Post-merge scenarios: `tools/smoke_scenario.py` and `tools/lan_scenario.py` (two owned processes over localhost LAN); see docs/evidence/postmerge-validation.json. For a visible manual test in a new disposable survival world:

```sh
python3 tools/play.py
# Optional creative starting mode:

python3 tools/play.py --creative
```

The launcher uses the resource directory above by default; supply another as a
positional argument if needed. Each run gets fresh homes, preferences and world
under target/scenarios/manual-*. It opens a normal visible window and retains
that instance and game.log after quitting. Right Shift opens the GUI; all modules
start disabled, F8/F9 toggle coordinates/FPS and F10 disables all modules. Start
with the HUD/editor, AutoTool, Sprint, Parkour, AutoArmor, AutoTotem and Panic,
which have host scenarios; the other newer modules still need gameplay validation.
AutoArmor, Refill and ChestSteal are guarded out in creative, so use survival for inventory tests.
Rebuild after source changes with `cargo run -p xtask --locked --offline -- build`.

The development scenario driver accepts a prepared resource directory:

```sh
python3 tools/scenario.py /absolute/path/to/prepared/26.3

```

It uses a fresh working directory and isolated homes for each run, then captures input/HUD states and quits the test game. Screenshots require visual inspection; successful commands alone do not establish rendering parity.

## Packaging and installing the .hnmod

```sh
python3 tools/bundle.py                 # dist/vape_rs-<version>.hnmod and .sha256
python3 -I tools/launcher_build.py      # optional: build it the way the launcher does
VAPE_RS_BUILD=launcher python3 tools/smoke_scenario.py /path/to/prepared/26.3
```

`bundle.py` packs the source-only bundle: `horizon-native.json` plus `mixins/vape_rs/` (only `.rs` and `.toml`, no game source or resources), with fixed timestamps so the same sources give the same bytes. The mod id is `vape_rs`, because the registry's ids allow only lowercase letters, digits and `_`. Horizon's own registry check accepts it (`cargo horizon-mod check dist/vape_rs-0.1.0-dev.hnmod`, built from the pinned Horizon). `launcher_build.py` follows the launcher's `native_mods::build` under target/launcher-build: it unpacks with the registry's path rules, runs `cargo install --git` for horizon-mixins at the bundle's pin, downloads GitHub's archive of the pinned Horizon commit, applies the mods, and builds `--profile release --features horizon-client/mods` with the launcher's Rust 1.99.1. `VAPE_RS_BUILD=launcher` makes the scenario tools run that executable.

To play it with Horizon's launcher (0.2.27 or later, with an installed Nightly for Minecraft 26.3 to reuse the game files of):

```sh
mkdir -p ~/vape_rs && cd ~/vape_rs && unzip /path/to/vape_rs-0.1.0-dev.hnmod
horizon-launcher native-build ~/vape_rs --trust-native-code
```

The launcher builds the game on Horizon `cc7bcbf` (the bundle pins it; the first build downloads Rust and takes 20 to 30 minutes) and points its "Native mods" instance at the result. Native mods run with your full OS privileges; read the source before trusting it. Settings are kept in the instance's `native-mods/vape-rs` folder. Only the Linux x86-64 build route has been exercised; Windows and macOS are untested.

## Tracking work

- `docs/features.csv` and `docs/discovery.json` are generated locally by `tools/inventory.py` from a private reference checkout and are not published.
- [Progress](progress.md), [decisions](decisions.md), [hooks](hooks.md), [status](status.md).

Before a release, finish the behavioral/UI audit, session ownership, gameplay/remote-client scenarios, performance measurement, provenance and fresh native installation tests.
