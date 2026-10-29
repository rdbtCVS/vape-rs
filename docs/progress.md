# Progress

Current handoff: [status.md](status.md). Entries below are chronological; later milestones supersede earlier pending-work notes.

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

## Owned native client runtime

User explicitly directed continuing until the complete plan is done, including the reported tooling dependency. Implemented local development tool commit e83e49f4ceca0dbb3156617d6d86f3996c2341cc atop PR #2: state supports lifetime-only owners without weakening bridge/type/const-generic or layout/public-construction guards. Constructor transformations append only the new initializer to visible literals and preserve original macros/`?`; hidden macro constructors must fail the mandatory staged Cargo check. Exported the change as tools/mixin-patches/0001-lifetime-state.patch with SHA-256 in compatibility.json. Staging reconstructs and verifies the patched tool from the clean base in target/mixin-source; neither reference checkout is edited or used dirty.

Replaced the thread-local runtime with an owned Arc/Mutex state on each Game. Temporary handle clones end the Game field borrow before host effects; all input/HUD/GUI adapters use that instance. The AST audit now validates the state declaration, removes only its exact field, final constructor initialization, initializer and lifetime-correct accessors, and restores the original host AST alongside the existing hooks/module. Added negative initializer audit coverage and independent runtime ownership/move coverage.

Validation: all 70 tool CLI integration tests passed, tool fmt/Clippy passed; 33 runtime + 4 audit tests and 33 emitted tests, mod fmt/Clippy passed. Exact native state plan/apply/audit and actual pinned host check passed. Optimized host binary built from the same production source (the new ownership regression test was added under cfg(test) afterward). Re-ran real settings/profile/chord/panic/restart/corrupt-file scenarios on both execution paths, all passed with clean exits. Evidence: docs/evidence/client-ownership-validation.json.

Public native packaging is explicitly gated: the launcher cannot apply a development tool patch from a mod bundle, so tools/bundle.py refuses a misleading artifact while patches are configured. A published compatible tool pin remains a release dependency. No new upstream issue/PR/message has been sent. Continue with session generation/invalidation and transient cleanup before stateful gameplay; the full specification and complete module/UI parity remain unfinished.

## Session generations and real transition cleanup

Added typed client session epochs, generation-qualified entity handles and context/death observation. Added seven native transition hooks (open/close, dimension, respawn, join/leave and remote dimension replacement). Invalidations release held/chord state, close owned GUI when appropriate and retain ordinary toggle preferences. Profile replacement and panic use the same cleanup path. No world names/addresses are included in mod epoch diagnostics. Repeated unchanged frames do not churn generations. This establishes the lifecycle dependency; future action/rotation/inventory/packet systems still need their own cleanup integration and real scenarios.

Validation: 35 runtime + 4 audit tests, 35 emitted tests, fmt/Clippy, exact 17-hook/state/module audit, pinned host check and optimized executable build passed. Actual native lifecycle scenarios passed cleanly in integrated-server and in-frame modes: Nether travel, kill/death, UI respawn, pause, save/quit and second-world open, with held FPS release, retained coordinates preference, reactivation in next world and strictly increasing epoch diagnostics. Inspected next-world capture. Evidence: docs/evidence/lifecycle-validation.json. Native manifest binding rules rejected an enum literal binding during implementation; moved constants inside small typed helpers instead of widening the binding contract.

Continue the shared registry/action ownership and retained UI/settings/HUD behavior, then the full inventory/combat/movement/packet/AI feature plan. No complete module/visual parity or distribution claim is made.

## Rich coordinate layouts and live compass

Replaced fixed per-module booleans/bindings with a registry of implemented module states. Added actual Compass registration/binding/persistence/UI. Coordinates now render live biome text, direction arrows and dividers with horizontal/vertical source dimensions. Biome lookup uses actual player-height client chunks as the explicit modern 3D interpretation. Compass draws a wrapped live-yaw strip with 5-degree ticks, cardinal/15-degree labels, center arrow and heading. FPS follows its fixed 50x20 centered frame. Pure layouts accept actual host font measurement.

Added a shared host-specific HUD painter support module used by both gameplay and editor previews. Updated the audit to verify all declared support module sources against current editable source (including expanded runtime) and restore every exact module, rather than assuming one. No unimplemented catalogue entries are registered. Source geometry is authored; copied fonts/image assets are not bundled, and exact visual/localization parity remains open.

Validation: 38 runtime + 4 audit tests, 38 emitted tests, fmt/Clippy, exact native 17-hook/state/two-module audit, actual pinned host check and optimized build passed. Native detail scenarios passed on both host paths with all four cardinal headings, biome/coordinate layouts, Compass bind/GUI toggling, restart and F1 captures. Inspected north/horizontal captures. Existing HUD drag/snap/scale/restart and bound movement/attack isolation scenarios were rerun on both paths and passed cleanly with the new painter. A first detail scenario used the wrong yaw JSON path; corrected to actual player.yaw. Evidence: docs/evidence/hud-details-validation.json; specification: docs/specs/hud-layouts.md.

Continue retained rendering/GUI widgets and shared gameplay ownership, then the remaining complete feature ledger. Module rows remain implemented-core, not full validated parity. Public launcher packaging still needs a published tool pin containing the development patch.

## NoHurtCam native render module

Added the actual retained NoHurtCam module with common activation/bind/profile/UI behavior and no invented HUD-placement settings. A narrow cancellable HEAD on Game::bob_matrix dispatches to the host's existing camera math with a copied render snapshot and suppressed hurt phase. Actual health, timers, options and physics remain untouched; view bobbing and death roll are retained. Disabling continues through the original method. Specification: docs/specs/no-hurt-camera.md.

Validation: 38 runtime + 4 audit tests, 38 emitted tests, fmt/Clippy, exact 18-hook/state/three-module audit, pinned host check and optimized build passed. Two actual embedded native camera tests passed (`cargo test ... --bin horizon-client game::vape_camera::tests`), checking hurt removal, retained view bob/death and unchanged input snapshot. Real damage/GUI/bind/restart/panic scenarios passed on both host paths and exited cleanly. Inspected enabled-hurt capture. The first integrated-server scenario observed health before the asynchronous server update; fixed it to wait for authoritative damage/hurt state. Evidence: docs/evidence/no-hurt-camera-validation.json. Module remains implemented-core while full shared UI/icon fidelity is open.

Continue shared action/inventory ownership and AutoTool's full settings/state machine, without pruning weapon swap, delays, guards or interruption cleanup. Other UI/HUD/render modules and all remaining phases stay in scope.

## User scope change — exclude AI behavior

Removed AI behavior from plan.md's retained features and implementation/release gates. SilentAura remains in scope with PID and its other non-AI behavior. Marked the AI submode, both AI model options and the cross-cutting inference/assets row excluded-approved. Inventory regeneration enforces those approved exclusions while preserving historical source discovery and all other implementation evidence. No AI runtime code or assets have been implemented, so no runtime removal was necessary. This decision supersedes prior progress references to future AI work.

Validation: regenerated the static inventory twice and verified stable ledger/discovery output, four approved AI exclusions and retained PID/module status. No game test is needed for this scope/documentation change. The rest of the full implementation plan remains active.

## AutoTool hotbar/action baseline

Added typed bounded hotbar slots, generation/owner/priority/serial leases, conditional selection actions and stale/user-change authorization checks. Implemented AutoTool's pure selection/delay/swap-back/guard state machine and all nine retained settings, with no dummy options. Pending actions and claims reset on disable, profile/panic, world/session and context transitions. Added friend-use behavior for inventory guards through the existing local people data. Native adapters snapshot nine actual slots/target/tool stats, use host metadata/enchantment calculations, and select through the same field and packet ordering as normal host hotbar input. No inventory items are moved or simulated. Specification/modernization details: docs/specs/auto-tool.md.

Validation: 45 runtime + 4 audit tests, 45 emitted tests, fmt/Clippy, exact 19-hook/state/four-module audit, pinned host check and optimized build passed. Actual AutoTool baseline scenarios passed cleanly in both host paths: stone broke in eight ticks with a cobblestone drop, diamond pickaxe durability changed while the golden pickaxe stayed unused, swap-back ran, manual hotbar input canceled restoration, panic stopped switching, GUI enable and restart preferences worked. Inspected manual-selection capture. Fixture failures were corrected by creating the platform before the final teleport, waiting for block updates and clicking within the real centered World sidebar rectangle. Evidence: docs/evidence/auto-tool-baseline-validation.json.

AutoTool and hotbar arbitration rows are implemented-core, not fully validated. Continue native weapon/guard/context interruption cases immediately, then remaining inventory transaction adapters/modules and the rest of the retained plan. AI remains explicitly excluded by the latest user decision.

## AutoTool option/interruption scenarios

The already-running expanded AutoTool suite completed before the user's testing preference changed. Fourteen fresh native scenarios passed across the two host paths: pending cancellation on disable/menu, no replay across dimension changes, sneak/use/recent-damage guards and cooldown expiry, mouse requirement disabled without synthetic attacks, weapon swapping enabled/disabled, instant/delayed selection and Sharpness changing the preferred weapon. These used the 2f0b433 implementation. Evidence: docs/evidence/auto-tool-guards-validation.json. Nearby-player/friend threat combinations and cross-module combinations remain open.

User now requests implementation focus with fewer tests and explicitly permits subagents when optimal. Keep quick compile checks as needed, defer broad gameplay/visual suites until larger feature batches are implemented, and preserve final validation gates. A worker is implementing retained UI editors and then the macro engine while root continues modules/adapters. AI remains excluded.

## Typed editor implementation batch

Worker implemented retained choice popups, quantized drag number sliders, ordered dual-handle range sliders, ARGB channel popup sliders/swatches and click/drag text caret selection. Existing Paint/Action APIs are preserved. Pointer text placement currently uses compact character-cell metrics; exact proportional-font placement remains open and documented in docs/specs/typed-editors.md.

Validation at this stage is quick offline workspace cargo check, rustfmt and whitespace checks only. Gameplay/visual regression is deferred under the user's implementation-first preference; these edits are not promoted to fully validated parity. The worker continues the macro engine independently while root implements more live HUD modules.

## Widgets, local macros and inventory implementation batch

Following the user's request, implementation is now batched ahead of validation. No new gameplay/visual scenarios or broad test suites were run for this batch. Quick runtime/native compilation and exact source staging remain the integration checks; compile evidence does not promote any row to `validated`.

Implemented four additional native HUD modules: Clock (analog/digital/date/24-hour), Keystrokes (key/mouse styles, spacebar, CPS-only), Armor Status (host item models, compact/detailed durability/broken warnings), and Potion Status (actual effects, positive/negative filters, duration/progress). Gameplay and editor use the same painter/snapshots. Registry/binds/profiles/layout settings are connected. Authored icon geometry, typography, localization and platform date conventions remain parity work. Specification: docs/specs/hud-widgets.md.

Implemented local profile macros with a bounded engine, named physical chords, command/item classification, randomized delays/double clicks, hotbar ownership, manual-selection interruption and cleanup. Added a native Macros editor with create/update/remove, binding capture and delay options. Native effects use the existing chat/command path and normal remote use-input queue or local item-use path. No backend or OS automation is involved. The fishing-rod early hook shortcut and custom/localized item-name matching remain open; regular delayed fishing-rod use is connected.

Implemented AutoArmor, Refill and ChestSteal planners and native dispatch using live menu contents, fingerprints/session/state IDs, the normal local/remote menu click pipeline, and a serialized frame-based randomized-delay scheduler. AutoArmor scores actual host armor attributes/protection. Refill opens/closes inventory and honors its retained item/arrangement options; ChestSteal honors menu/best/keep/shuffle/blacklist options. Open/close behavior uses the normal inventory screen lifecycle. Pending clicks require changed live state; a queued/sent click is never called server acceptance. Harmful splash potion classification and exact reference scoring/selection parity remain open. Creative inventory automation is currently guarded out pending its distinct menu protocol. Specifications: docs/specs/inventory-planners.md and docs/specs/inventory-host.md.

Validation: runtime workspace cargo check passed; exact native plan/apply/audit passed (19 HEAD hooks, one owned state, six support modules), and pinned horizon-client cargo check passed. A separate current-source audit confirmed the staged sources still match. Evidence: docs/evidence/widgets-inventory-macros-compile.json. No executable rebuild, gameplay suite or visual test was run. The seven new module rows and macro feature are implemented-core only; full plan and parity gates remain open.

## FastPlace, Fullbright and inventory metadata

Added FastPlace with retained All/Blocks/Projectiles filters (either hand) and 0–4 tick delay (default 1). It only shortens the existing host repeat-use delay, preserves item cooldowns/placement rules and yields to active item macros. It runs at the existing client tick hook, with all gameplay guards. No synthetic use is produced. Fixed the initial worker draft's default/timer/numeric extraction during integration before commit.

Added Fullbright's default Night Vision and Gamma modes, Gamma-only Fade, live renderer ownership/restoration and immediate world/profile/panic cleanup. Night Vision is a client render override rather than a fake gameplay potion; real potion effects and saved options remain untouched. Gamma retains 0.4/10ms fading toward 10.0. Shader-pack-specific uniforms and real visual parity remain open (docs/specs/fullbright.md).

Replaced the initial inventory potion-ID approximation with actual decoded potion contents: Refill selects splash healing/regeneration and explicit soups; ChestSteal recognizes the reference's harmful splash effects. Inventory requests now verify the carried cursor fingerprint, and local revision fingerprints include item IDs and cursor contents. Macro chords accept unrelated held movement keys while still requiring a physical edge on one of the bound keys.

Validation: runtime cargo check and exact native stage/audit passed (20 HEAD hooks, one state, seven support modules); pinned horizon-client cargo check passed. No tests/gameplay/visual suites run, per the implementation-first preference. Evidence: docs/evidence/fast-place-fullbright-compile.json. Fullbright/FastPlace are implemented-core; complete module parity and the full plan remain open.

## AutoFish, Clear Water and input delays

Added real AutoFish cast/reel/recast behavior, ground/caught options, hotbar rod replacement and delayed main-inventory refill. Native bite detection reads actual local/remote DATA_BITING. A HEAD observer retains only this client's authoritative hook-target metadata in session-owned mod state, enabling remote caught-entity behavior and the macro fishing shortcut without editing Horizon's hook model. Rod replacement uses normal menu transactions and waits for changed revision after sending. Remote on-ground state is not mirrored by the pinned host; full remote Recast-ground fidelity remains open (docs/specs/auto-fish.md).

Extended shared claims to use-input and container resources. Macros have priority over fishing use; fishing/refill/armor/chest actions serialize container ownership, and tool preemption invalidates lower-priority hotbar claims. Shared combat guards now survive AutoTool being disabled or a menu opening; only session cleanup resets them. Normal macro completion restores the selected slot; safe GUI/focus/panic restoration remains a follow-up.

Added Clear Water using a guarded cancellable HEAD and preserved pinned native fog calculation. It clears only underwater environmental fog to the existing terrain distance, preserves native colors/caps and leaves blindness/darkness intact. The attempted RETURN approach was rejected by the tool's closure guard; the implemented HEAD keeps the original body for every disabled/non-water case. The temporary unused RETURN-audit experiment was removed.

Added NoJumpDelay and NoClickDelay against the actual native jump gate and attack-miss timer, under gameplay guards. The obsolete NoClickDelay 1.8.9 gate is explicitly modernized against Horizon's unified current miss timer; normal combat cooldowns and collisions remain host-owned.

Validation: runtime cargo check, exact native stage/audit (22 HEAD hooks, one owned state, nine support modules) and pinned client cargo check passed. No gameplay/visual suites or broad tests run. Evidence: docs/evidence/fishing-water-delays-compile.json. All four module rows are implemented-core only; complete plan/parity/distribution gates remain open. BlockOverlay source exploration is unregistered pending a complete geometry insertion path; it is not claimed implemented.

## Current batch close-out and documentation handoff

At the user's request, finished the current macro-cleanup work and stopped starting new features. Macro completion and safe GUI/focus/panic/profile cleanup now emit a separate restoration effect. A valid same-session hotbar claim and unchanged selection are required; manual selection (including after cleanup was queued), preemption, death and world/dimension/connection replacement discard stale restoration. Owned use input is released. Added three focused runtime cases for menu cleanup, manual selection and panic versus dimension changes.

Updated README.md, plan.md, decisions, the complete hook catalogue, macro specification and feature evidence. New docs/status.md lists all 18 connected modules, actual evidence levels, compatible resource path, unfinished fidelity/dependencies and ordered next work. AI remains excluded; the rest of the full plan remains open. Historical prototype/pending notes above are superseded only where a later milestone provides evidence.

BlockOverlay remains unregistered. Moved its partial outline/options code out of shipping source paths into documentation-only drafts, preserving the exact proposed fishing-line outline hook and transparent-fill/tool capability dependency in docs/specs/block-overlay.md. These drafts were not compiled or gameplay validated and do not promote the feature ledger status. No new mixin-tool patch or host source change was made for them.

Close-out verification: 48 runtime tests and 4 audit tests passed; workspace formatting and Clippy (all targets, warnings denied) passed. Corrected six lint findings in the recent batch without broadening feature scope. Exact final native stage/audit passed with 22 HEAD hooks, one owned state and nine support modules, restoring original host ASTs and other input hashes. The final pinned-client compilation result is recorded in docs/evidence/macro-cleanup-closeout.json. No new executable build, game/visual suite, remote scenario or distribution validation was run. The adjacent Horizon checkout has unrelated working-tree changes; exports still use only the immutable pinned commit under target, and no adjacent changes were made here.

## Visible isolated manual testing

In response to the user's usability/testing question, rebuilt the current native client from the close-out runtime (source commit 3f0462a). Exact staging/audit passed again; optimized fast-profile build passed (1m17s compile, existing 17 dead-code warnings). This replaces the older executable under target/host-cargo/fast/horizon-client. No new game execution or visual/gameplay validation is claimed.

Added tools/play.py as a visible manual entry point using the existing NativeSession isolation. Each invocation allocates fresh homes and working directory under target/scenarios/manual-*, uses approved prepared resources read-only, opens a normal survival world, retains its log/world/preferences for inspection, and closes only its owned process. Optional --in-frame and --creative are available; the three inventory automation modules require survival. CLI help passed. The development build has audio/controllers disabled. README and docs/status.md now include the exact launch command and controls. Evidence: docs/evidence/manual-playtest-build.json.

## Inventory consumable Refill fidelity batch

Corrected Refill hotbar-clear allow-list handling to retain one stack per item,
initialized scatter's random state so it does not remain zero, and wait for an
empty user cursor before starting a new transaction. Existing revision and
container claims still govern each click. The optional reference double click,
custom item matching, remaining inventory modules, and host scenarios remain
open. No new module or full port is claimed.

Validation: 51 runtime and 4 audit tests passed; 51 emitted payload tests,
formatting and Clippy (all targets, warnings denied) passed. Exact native
stage/audit passed with 22 hooks, one state and nine support modules; the pinned
client cargo check passed. No game execution was performed.
