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

At the user's request, filed the upstream HEAD/body-macro support request: https://github.com/horizon-rust/horizon-mixins/issues/1. The report records both pins, the rejected target/diagnostic, why the shared physical-key boundary matters, and requested regression coverage. No tool fork or implementation pin change has been authorized by filing this issue.

Development packaging, CI and a scratch-only scenario driver were added. The development bundle is source-only and deterministic; native launcher installation and real rendering are not yet validated. README documents actual commands and limits. No release or registry submission has been made.

## Build on upstream PR #2

User explicitly authorized treating the upstream PR as merged for development. Inspected PR #2 and pinned its exact head `29a8f8f84b253ea89422c0049ff5e8bfda36f1fb` (no local fork changes). This supersedes the earlier pending tooling decision. Horizon remains `e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083`.

Moved client-input from translated `mapped_key` to shared physical `debug_key`, passing the host key/action enum. Added the new source file to pinned hashes and the exact AST audit. The bundle's native manifest derives the new tool revision from compatibility.json. README records the PR pin and its development status.

Validation so far: all 5 upstream macro-filtered integration tests passed, including preservation/negative cases; native plan/apply succeeded on the previously rejected target; exact source audit passed; 12 standalone tests, emitted runtime's 10 tests, formatting and standalone Clippy passed. Actual pinned client cargo check passed. Executable rebuild and isolated scenarios follow; no full module parity claim is made.

### PR integration validated in the running client

The PR resolves the original macro rejection. Added one narrow cancellable HEAD on the host control-key parser because its existing parser omits F8/F9/F10. The helper maps just those three names and leaves all other names to the original function. Updated the source audit to verify this fourth hook, including free-function/cancellable-hook coverage. It does not toggle modules itself; tests send the actual keys through the shared physical handler.

Corrected scenario isolation: for a custom executable, horizon-ctl inherits its caller's working directory. The driver now supplies a fresh scratch working directory as well as fresh HORIZON_HOME, HORIZON_LAUNCHER_HOME, XDG_DATA_HOME and XDG_CONFIG_HOME. The earlier test's generated cache/random test name/world were moved from the project root into that failed scenario's ignored target directory. No existing worlds or player settings were touched. Changed the repeat scenario to hold the key while repeating, then release it; added F1 hide/restore captures.

Validation: 13 standalone tests, emitted payload's 10 tests, 5 upstream macro integration tests, standalone fmt/Clippy, pinned host check and optimized client/ctl build passed. The exact audit removes four expected HEAD hooks and one support module and restores all original ASTs/other input hashes.

Actual native game scenarios completed in fresh worlds for both default integrated-server and --in-frame paths: 20 successful control commands each, one client-start diagnostic each, seven real rendered captures each. Inspected captures demonstrate disabled defaults, actual coordinates/FPS rendering, held-key repeat suppression, F1 hide/restore, panic removal, and no toggle from a pause-screen press. Both games quit via their own control sockets. Artifacts and capture hashes are recorded in docs/evidence/pr2-native-validation.json.

Coordinates/FPS ledger rows are now `implemented-core`, not `validated`: full reference layout/typography, biome/direction details, settings/UI/persistence and complete parity remain unfinished. The source bundle was rebuilt with the exact PR head in its native manifest (7 source files, SHA-256 in the validation report). Adjacent reference checkouts remain clean.

Next dependency: finish Phase 0 behavioral specification and lifecycle/session ownership, then the complete settings/binding/profile/UI foundation. PR #2 need not merge before this development continues; no automatic pin movement is needed on merge. Public release, launcher installation, independent remote-server scenarios, cross-platform coverage and performance remain open.

Inventory regeneration preserves the new evidence/status fields; CSV output now uses LF line endings so Git whitespace checks pass reproducibly. README/CI explicitly fetch the pinned PR commit before checkout, including when it is not on the default branch.

## Shared settings, profiles and binding foundation

Resumed the original complete plan after the user correctly pointed out that stopping at the PR integration was premature. No module subset or prototype endpoint replaces the original scope.

Added typed boolean/number/range/choice/color/text/list definitions, conditions, validation, normalization diagnostics and unknown-setting round-trips. Added local JSON schema/version handling, import without activation, named profile CRUD/import/export APIs, private atomic file replacement, bounded reads and preservation of invalid current files. Runtime adapters load/save instance-owned current preferences and apply actual HUD settings. No dependencies were injected into Horizon: runtime JSON uses the host's existing locked serde_json 1.0.151, audited at staging. Emitted payload tests now use a separately committed Cargo lockfile with that dependency.

Static tracing corrected binding fidelity: multiple keys are a chord, not alternatives. Core supports toggle/held chord behavior, repeat suppression, release of any member, context/focus cleanup and cleanup using the old mode when changing a binding. Module activation choices match reference TOGGLE/HELD; release-edge machinery remains internal for event/macros work. Full mouse capture and event cancellation remain open.

Validation: 25 runtime tests plus 3 tooling audit tests passed, emitted payload's 25 tests passed, formatting/Clippy and exact native source audit passed, and actual pinned host check/optimized build passed. Real native profile scenarios passed in both integrated-server and --in-frame paths, including restart, chord press/release, panic persistence, unknown data preservation, and corrupt-file preservation while the client remains usable. Inspected rendered artifacts show real vertical layout, scale/color/background edits and retained settings after restart. Evidence: docs/evidence/settings-profiles-validation.json. Partial cross-cutting ledger rows are implemented-core, not validated.

Traced client foundation/UI source contracts in docs/specs/client-foundation.md, including the actual 400×240 main frame and retained Modules/Friends/Profiles sections. Next: implement and integrate those local UI interactions, then continue the complete feature ledger and remaining plan. The full port is not complete.

## Native local GUI foundation

Added the source-specified 400×240 main frame with 40 header, Modules/Friends/Profiles sections, category sidebar, module search/favorites, module setting sidecar, typed edits and key/chord capture. Reference single/multi binding capture is event-driven rather than spawning a polling thread. Local profile buttons perform actual save/load/rename/confirmed-delete/import/export; imported files never activate modules. Local people editing supports names/UUID strings, aliases, friend target flags and enemy exclusive flags. These are partial feature rows, not full source/UI parity.

Integrated physical text/mouse/IME events, custom HUD rendering, game/control key capture, mouse bindings, and a frame lifecycle hook so GUI close/save continues while world ticks are paused. An owned vanilla pause state suspends gameplay input; the native GUI replaces that surface's vertices and restores Playing/capture only after the close event is consumed. Glyph upload patches remain intact. Added `rshift` to the narrow native control-key name helper. Profiles list is cached rather than scanning disk every rendered frame.

The audit now checks every declaration against the plan (target, typed bindings, cancellation and exact helper source), removes all ten expected hooks and the exact support module, and compares all affected ASTs/other inputs. It requires pinned hashes for every affected source file. This avoids a hardcoded hook count as the complete port adds integrations.

Validation: 29 runtime and 3 audit tests, emitted payload tests, fmt/Clippy, exact native plan/apply/audit, actual pinned client check/optimized build all passed. Actual GUI scenarios passed in both integrated-server and --in-frame paths, with 28 captures each: real setting/bind edits, local profile CRUD/import/export/load, friend/enemy edits, GUI movement/attack isolation, safe closing/reopening and restored HUD settings. Captures were inspected. A test initially called the gameplay-only `mouse` command on a paused screen; its expected host rejection was corrected to an actual outside-window pointer click. Evidence: docs/evidence/gui-foundation-validation.json.

Adjacent Horizon HEAD advanced independently to b7df7bd9e0b2b61cebd73d88194acd0b5956c6ed during development. Staging/builds remain on the original committed e1b2b0… pin; subsequent host inspection uses that pristine export. No adjacent checkout was edited.

Continue immediately with the HUD drag/snap/scale editor and remaining reference widget behavior, then the remaining shared runtime and full feature plan. Fonts/icons/animations and full visual parity are still open; no prototype completion claim is made.

## Native HUD editing

Added native widget dragging, viewport bounds, screen/widget edge snapping (Shift bypass), wheel scaling, settings navigation and enable/disable controls. Disabled widgets appear only as dim editor previews using actual host values; previewing does not enable them. Placement and scale use normal settings persistence. Reference minimum FPS/coordinate panel dimensions are retained; full widget contents, typography and visual parity remain unfinished.

Validation: 30 runtime + 3 audit tests, emitted payload's 30 tests, fmt/Clippy, ten-hook exact source audit, pinned host check and optimized build passed. Real HUD editor and bound-input capture scenarios passed in both integrated-server and in-frame paths, including restart persistence, Shift snapping bypass, no movement from a bound W key and no attack from a bound left mouse button. Inspected actual editor/scaled captures. Evidence: docs/evidence/hud-editor-validation.json. No row was promoted to full `validated` parity.

Next: complete text editing/selection and modal input isolation, then remaining typed controls, widget details, shared lifecycle and the complete feature ledger. Full plan remains active.

## Native text editing and modal isolation

Added character-indexed caret navigation (arrows/Home/End and word navigation), Shift selection, forward/backward deletion, replacement, visible selection/caret and clipped horizontally scrolling fields measured with the actual host font. Added explicit copy/cut/paste through Horizon's clipboard trait; no clipboard reads occur without the user's paste shortcut. Invalid typed setting edits retain their buffer for correction. Profile delete confirmation blocks background clicks and typing, including events arriving before a new frame. Extended the test key-name helper only for Home/End/Delete.

Validation: 32 runtime + 3 audit tests, emitted payload's 32 tests, fmt/Clippy, exact ten-hook native audit, actual pinned host check and optimized build passed. Real scenarios passed in both execution modes with private Xvfb clipboards, demonstrating selection/replacement/deletion, actual profile save, copied/cut/pasted alias persistence and modal isolation/cancel. Captures inspected. Initially removing all display variables failed winit startup; private Xvfb fixed isolation. Two concurrent xvfb-run invocations raced for one display during cleanup; the affected case was rerun sequentially and exited cleanly. Evidence: docs/evidence/text-editor-validation.json. Full typed widgets, click-to-position caret, fonts/icons/animation and all-module parity remain open. Corrected the HUD-editor ledger row to implemented-core with its actual preceding milestone evidence.

### Next lifecycle dependency: confirmed native state limitation

Before stateful gameplay, the runtime must stop sharing a thread-local singleton across Game instances. A plan-only minimal probe on the committed host export rejects state on lifetime-only `Game<'a>` with E_BRIDGE_TARGET (generic owners unsupported). A probe on adjacent nongeneric `chat::UiState` rejects its wholly-public fields with E_STATE. Neither changes the host or weakens construction guards. Evidence: docs/evidence/lifecycle-state-probes.json; human-readable request: docs/upstream/lifetime-state-request.md. Existing HUD/UI remain usable, but per-client ownership/session lifecycle is unfinished. User requested stopping on concerns; raise this tool contract gap before extending/pinning another tooling change. No new upstream message has been sent.
