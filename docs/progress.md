# Progress

## 2026-10-28 — implementation start

Initialized Git and committed the unmodified plan. Both adjacent references match the plan and are clean. Read Horizon's contribution/data isolation rules and the native packaging/CLI implementation.

Created reproducible static inventory tooling, discovery evidence and a machine-readable feature ledger. Entries remain `not-started`; only explicitly approved online exclusions have completion-like status. Phase 0 remains open: behavioral specification and discovery completeness need manual review.

Built the pinned mixin tool after inspecting its CLI execution/trust boundary and manifests. No reference Java code/tools have been run. No upstream files have been edited.

Next: a modular testable runtime, deterministic source packer/export/hash audit, real tick/input/HUD plan and host compilation. Prepared 26.3 resources were not found at the plan's standard Horizon location; gameplay/visual validation will need the user's compatible resources. Do not generate or download Mojang assets.

## Native integration slice

Added a standard-library-only modular runtime with coordinates/FPS enabled states, toggle/hold/release binding logic, repeat suppression, panic cleanup, HUD snapshot formatting and first-client-tick diagnostic. Current native controls are F8 (coordinates), F9 (FPS), F10 (panic); all displays start disabled. These controls/layout are integration scaffolding pending the complete reference UI/settings implementation.

Added Rust xtask for deterministic module flattening, content-hash source map, committed-source export, source drift detection, exact mixin tool pin/cleanliness enforcement, real plan/apply, and an AST audit that permits only exact expected hooks/module. Repeated staging reported `stage_changed: false`. Source export compares the entire tree against a fresh export of the committed pin; adjacent checkouts remain clean.

Validation: standalone 10 runtime + 2 audit tests passed; emitted payload's 10 tests passed; standalone Clippy with `-D warnings` passed. Exact pinned client `cargo check --locked --offline -p horizon-client --no-default-features --bin horizon-client` passed after correcting adapter types. Host reports three dead-code warnings for APIs not yet exposed through adapters. Full executable build and resource-backed scenarios are in progress; no feature marked validated and Phase 0/1 gates remain open.

The user supplied the Horizon home; compatible prepared resources were located in the launcher nightly's `.local/horizon-client/26.3`. Use only read-only resources and new scratch homes, never that player's actual home/world data.
