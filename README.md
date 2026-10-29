# vape-rs (development)

Independent Rust-native Horizon mod rewrite. The full scope and completion gates are in [plan.md](plan.md); the current handoff is [docs/status.md](docs/status.md). The rewrite is incomplete. Twenty-seven modules are connected to the native client: seven HUD widgets, AimAssist (Simple mode), TriggerBot, Velocity, SafeWalk, NoHurtCam, Fullbright, Clear Water, StorageESP (partial), AutoTool, AutoArmor, AutoTotem (partial), Refill, ChestSteal, FastPlace, AutoFish, NoJumpDelay, NoClickDelay, Sprint, Parkour (experimental) and Panic. Local settings/profiles/friends, macros, typed editors, binding capture and HUD placement are connected. Recent batches have compilation evidence; full gameplay/visual parity and release gates remain open. AI behavior and the online product are explicitly excluded; SilentAura’s PID and other non-AI features remain in scope.

References are read-only. No Java, JNI, process injection, account service or remote backend is part of this runtime. The adjacent reconstructed Java repository is used only for static behavioral inspection.

## Development

Use Rust 1.99.0 (rust-toolchain.toml), Git and tar; Python 3 is used only for inventory/ZIP development tools. First populate approved dependencies with `cargo fetch --locked`, then:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo run -p xtask --locked --offline -- payload-test
```

The AST packer reads conventional Rust module files under src and emits one source-only payload plus a content-hash source map under target. The emitted payload's tests compile in an isolated Cargo fixture with its committed lockfile. The runtime uses serde_json 1.0.151 and the native clock adapter uses chrono, both already declared and locked in the pinned host; staging verifies that contract and never injects a dependency.

The mixin pin currently uses [upstream PR #2](https://github.com/horizon-rust/horizon-mixins/pull/2), authorized for development before merge. The exact base commit remains fixed in compatibility.json; merging the PR does not automatically change this pin. Staging also applies the reviewed, SHA-256 pinned patch under tools/mixin-patches for lifetime-owned state and preserved constructors. These patches are exported from local development commit e83e49f4ceca0dbb3156617d6d86f3996c2341cc. `xtask stage` reconstructs them from the clean base; it never builds a dirty development checkout. Distribution is gated until a published tool pin contains this support: the launcher cannot apply tool patches from a mod bundle. `tools/bundle.py` rejects that incompatible package.

For native staging, review the pinned mixin sources before building them:

```sh
git clone https://github.com/horizon-rust/horizon-mixins target/upstream/horizon-mixins
git -C target/upstream/horizon-mixins fetch origin 29a8f8f84b253ea89422c0049ff5e8bfda36f1fb
git -C target/upstream/horizon-mixins checkout --detach 29a8f8f84b253ea89422c0049ff5e8bfda36f1fb
cargo +1.95.0 fetch --locked --manifest-path target/upstream/horizon-mixins/Cargo.toml
cargo run -p xtask --locked --offline -- stage ../horizon
cargo fetch --locked --manifest-path target/horizon/modded/Cargo.toml
cargo run -p xtask --locked --offline -- check ../horizon
cargo run -p xtask --locked --offline -- build ../horizon
```

Staging exports the committed Horizon pin from compatibility.json, ignores the original working tree, checks pristine source hashes, runs the pinned planner/transformer and audits the generated changes. Wrong pins, source drift, unexpected edits and stale bundle files fail loudly. All exports/builds remain under target. `check` type-checks the actual client binary; `test` compiles/runs embedded runtime tests; `build` compiles the client using Horizon's fast profile with audio/controllers disabled. These execute reviewed native Rust and its build scripts with normal user privileges.

Run the binary only with your own compatible prepared 26.3 resources and fresh scratch homes, as required by Horizon's AGENTS.md. Never point HORIZON_HOME at real player data for tests. See [docs/hooks.md](docs/hooks.md) for integration boundaries. Right Shift opens the custom local GUI. Its pages expose module settings/binding capture, local profiles, local friend/enemy records and a local macro editor. F8/F9 are default HUD bindings and F10 disables all active modules. Modules start disabled; saved preferences restore on restart. Text rendering uses the host atlas; reference typography/layout parity remains unverified.

The compatible prepared resource directory located for this workspace is:

```text
<launcher-dir>/versions/nightly-2026-10-28-9b772b2/.local/horizon-client/26.3
```

Use it read-only; every executable game run must use fresh isolated homes and working directory through tools/native_session.py. `xtask build` writes `target/host-cargo/fast/vape-rs-build.json` beside the executable (bundle hash, Horizon/mixin pins, tool patches, profile/features, cargo version, executable hash, source commit/dirty flag). `play.py` and every scenario refuse to launch an executable whose identity is missing or differs from the current sources (`--allow-stale` overrides with a warning), and copy the identity into the session as build.json. Post-merge scenarios: `tools/smoke_scenario.py` and `tools/lan_scenario.py` (two owned processes over localhost LAN); see docs/evidence/postmerge-validation.json. For a visible manual test in a new disposable survival world:

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

`python3 tools/bundle.py` is gated while development tool patches are required. Once a published compatible tool pin is recorded, it produces a deterministic source-only `.hnmod` and SHA-256 in dist without upstream game source or resources. Current development uses `xtask stage`; no compatible distributable is claimed. Installation through the launcher's native-mod build route and cross-platform support remain unvalidated. Native mods run with full OS privileges; installation uses the launcher's explicit native-code trust step.

## Tracking work

- [Feature ledger](docs/features.csv): retained entries count only when validated with real host evidence.
- [Static discovery](docs/discovery.json): source declarations/hashes; not a completed behavior specification.
- [Progress](docs/progress.md), [decisions](docs/decisions.md), [hooks](docs/hooks.md).

Before a release, finish the behavioral/UI audit, session ownership, gameplay/remote-client scenarios, performance measurement, provenance and fresh native installation tests. No asset redistribution rights are inferred from the reference's root CC0 notice.
