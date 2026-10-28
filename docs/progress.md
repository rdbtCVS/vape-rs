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

## Native build and input tooling concern

The pinned Horizon executable and horizon-ctl built with `cargo build --locked --offline --no-default-features --bin horizon-client --bin horizon-ctl --profile fast` (3m11s). This is an actual host binary with the runtime payload and three hooks, not a standalone fixture. The source manifests are restored to that working translated-key integration.

A scratch launch rejected the incorrect test flag `--distance`; scenario tooling now uses source-verified `--render-distance`. No player data was touched. Resource manifests from the launcher nightly report Minecraft 26.3. Gameplay/render scenarios have NOT passed.

Tracing control input showed normal `horizon-ctl key … tap` calls `key_pressed` directly, bypassing `mapped_key`; its repeat path uses mapped_key. The same raw `debug_key` boundary sees both real keyboard events and controlled key presses/releases. An attempted HEAD on `game::debug_keys::Game<'_>::debug_key` failed in the pinned native planner with `E_UNSUPPORTED_MACRO` (method contains opaque macros). The diagnostic is recorded in docs/evidence/physical-input-plan-error.json. The failed experiment was removed from active manifests; no fake successful key scenario or no-op adapter was substituted.

User requested stopping on concerns. Asked whether to extend and pin a development fork of the mixin tool with regression coverage, or retain the pin and investigate other hook boundaries. Awaiting that direction before continuing input integration. Full implementation remains unfinished; no feature is validated. Next work must resume the decision and Phase 0/1 gates, then the remaining complete plan.

Development packaging, CI and a scratch-only scenario driver were added. The development bundle is source-only and deterministic; native launcher installation and real rendering are not yet validated. README documents actual commands and limits. No release or registry submission has been made.
