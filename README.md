# vape-rs (development)

Independent Rust-native Horizon mod rewrite. The full scope and completion gates are in [plan.md](plan.md). The rewrite is incomplete: the current executable slice provides a client tick diagnostic and keyboard-controlled coordinates/FPS text. The complete reference UI, gameplay modules, persistence and release validation are still pending. No distribution-ready parity claim is made.

References are read-only. No Java, JNI, process injection, account service or remote backend is part of this runtime. The adjacent reconstructed Java repository is used only for static behavioral inspection.

## Development

Use Rust 1.99.0 (rust-toolchain.toml), Git and tar; Python 3 is used only for inventory/ZIP development tools. First populate approved dependencies with `cargo fetch --locked`, then:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo run -p xtask --locked --offline -- payload-test
```

The AST packer reads conventional Rust module files under src and emits one source-only payload plus a content-hash source map under target. The emitted payload's tests compile independently with rustc, without host dependencies.

The mixin pin currently uses [upstream PR #2](https://github.com/horizon-rust/horizon-mixins/pull/2), authorized for development before merge. The exact commit remains fixed in compatibility.json; merging the PR does not automatically change this pin.

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

Run the binary only with your own compatible prepared 26.3 resources and fresh scratch homes, as required by Horizon's AGENTS.md. Never point HORIZON_HOME at real player data for tests. See [docs/hooks.md](docs/hooks.md) for integration boundaries. The temporary integration keys are F8 (coordinates), F9 (FPS), F10 (disable both). Displays start disabled. Text rendering uses the host atlas; reference typography/layout parity remains unverified.

The development scenario driver accepts a prepared resource directory:

```sh
python3 tools/scenario.py /absolute/path/to/prepared/26.3
python3 tools/scenario.py /absolute/path/to/prepared/26.3 --in-frame
```

It uses a fresh working directory and isolated homes for each run, then captures input/HUD states and quits the test game. Screenshots require visual inspection; successful commands alone do not establish rendering parity.

`python3 tools/bundle.py` produces a deterministic development `.hnmod` and SHA-256 in dist. The source-only archive includes no upstream game source or resources. It is an experimental source bundle, not a completed public release. Installation through the launcher's native-mod build route and cross-platform support remain unvalidated. Native mods run with full OS privileges; installation uses the launcher's explicit native-code trust step.

## Tracking work

- [Feature ledger](docs/features.csv): retained entries count only when validated with real host evidence.
- [Static discovery](docs/discovery.json): source declarations/hashes; not a completed behavior specification.
- [Progress](docs/progress.md), [decisions](docs/decisions.md), [hooks](docs/hooks.md).

Before a release, finish the behavioral/UI audit, session ownership, gameplay/remote-client scenarios, performance measurement, provenance and fresh native installation tests. No asset or model redistribution rights are inferred from the reference's root CC0 notice.
