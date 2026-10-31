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

Actual native game scenarios completed in fresh worlds on the integrated server: 20 successful control commands, one client-start diagnostic, seven real rendered captures each. Inspected captures demonstrate disabled defaults, actual coordinates/FPS rendering, held-key repeat suppression, F1 hide/restore, panic removal, and no toggle from a pause-screen press. Both games quit via their own control sockets. Artifacts and capture hashes are recorded in docs/evidence/pr2-native-validation.json.

Coordinates/FPS ledger rows are now `implemented-core`, not `validated`: full reference layout/typography, biome/direction details, settings/UI/persistence and complete parity remain unfinished. The source bundle was rebuilt with the exact PR head in its native manifest (7 source files, SHA-256 in the validation report). Adjacent reference checkouts remain clean.

Next dependency: finish Phase 0 behavioral specification and lifecycle/session ownership, then the complete settings/binding/profile/UI foundation. PR #2 need not merge before this development continues; no automatic pin movement is needed on merge. Public release, launcher installation, independent remote-server scenarios, cross-platform coverage and performance remain open.

Inventory regeneration preserves the new evidence/status fields; CSV output now uses LF line endings so Git whitespace checks pass reproducibly. README/CI explicitly fetch the pinned PR commit before checkout, including when it is not on the default branch.

## Shared settings, profiles and binding foundation

Resumed the original complete plan after the user correctly pointed out that stopping at the PR integration was premature. No module subset or prototype endpoint replaces the original scope.

Added typed boolean/number/range/choice/color/text/list definitions, conditions, validation, normalization diagnostics and unknown-setting round-trips. Added local JSON schema/version handling, import without activation, named profile CRUD/import/export APIs, private atomic file replacement, bounded reads and preservation of invalid current files. Runtime adapters load/save instance-owned current preferences and apply actual HUD settings. No dependencies were injected into Horizon: runtime JSON uses the host's existing locked serde_json 1.0.151, audited at staging. Emitted payload tests now use a separately committed Cargo lockfile with that dependency.

Static tracing corrected binding fidelity: multiple keys are a chord, not alternatives. Core supports toggle/held chord behavior, repeat suppression, release of any member, context/focus cleanup and cleanup using the old mode when changing a binding. Module activation choices match reference TOGGLE/HELD; release-edge machinery remains internal for event/macros work. Full mouse capture and event cancellation remain open.

Validation: 25 runtime tests plus 3 tooling audit tests passed, emitted payload's 25 tests passed, formatting/Clippy and exact native source audit passed, and actual pinned host check/optimized build passed. Real native profile scenarios passed on the integrated server, including restart, chord press/release, panic persistence, unknown data preservation, and corrupt-file preservation while the client remains usable. Inspected rendered artifacts show real vertical layout, scale/color/background edits and retained settings after restart. Evidence: docs/evidence/settings-profiles-validation.json. Partial cross-cutting ledger rows are implemented-core, not validated.

Traced client foundation/UI source contracts in docs/specs/client-foundation.md, including the actual 400×240 main frame and retained Modules/Friends/Profiles sections. Next: implement and integrate those local UI interactions, then continue the complete feature ledger and remaining plan. The full port is not complete.

## Native local GUI foundation

Added the source-specified 400×240 main frame with 40 header, Modules/Friends/Profiles sections, category sidebar, module search/favorites, module setting sidecar, typed edits and key/chord capture. Reference single/multi binding capture is event-driven rather than spawning a polling thread. Local profile buttons perform actual save/load/rename/confirmed-delete/import/export; imported files never activate modules. Local people editing supports names/UUID strings, aliases, friend target flags and enemy exclusive flags. These are partial feature rows, not full source/UI parity.

Integrated physical text/mouse/IME events, custom HUD rendering, game/control key capture, mouse bindings, and a frame lifecycle hook so GUI close/save continues while world ticks are paused. An owned vanilla pause state suspends gameplay input; the native GUI replaces that surface's vertices and restores Playing/capture only after the close event is consumed. Glyph upload patches remain intact. Added `rshift` to the narrow native control-key name helper. Profiles list is cached rather than scanning disk every rendered frame.

The audit now checks every declaration against the plan (target, typed bindings, cancellation and exact helper source), removes all ten expected hooks and the exact support module, and compares all affected ASTs/other inputs. It requires pinned hashes for every affected source file. This avoids a hardcoded hook count as the complete port adds integrations.

Validation: 29 runtime and 3 audit tests, emitted payload tests, fmt/Clippy, exact native plan/apply/audit, actual pinned client check/optimized build all passed. Actual GUI scenarios passed on the integrated server, with 28 captures: real setting/bind edits, local profile CRUD/import/export/load, friend/enemy edits, GUI movement/attack isolation, safe closing/reopening and restored HUD settings. Captures were inspected. A test initially called the gameplay-only `mouse` command on a paused screen; its expected host rejection was corrected to an actual outside-window pointer click. Evidence: docs/evidence/gui-foundation-validation.json.

Adjacent Horizon HEAD advanced independently to b7df7bd9e0b2b61cebd73d88194acd0b5956c6ed during development. Staging/builds remain on the original committed e1b2b0… pin; subsequent host inspection uses that pristine export. No adjacent checkout was edited.

Continue immediately with the HUD drag/snap/scale editor and remaining reference widget behavior, then the remaining shared runtime and full feature plan. Fonts/icons/animations and full visual parity are still open; no prototype completion claim is made.

## Native HUD editing

Added native widget dragging, viewport bounds, screen/widget edge snapping (Shift bypass), wheel scaling, settings navigation and enable/disable controls. Disabled widgets appear only as dim editor previews using actual host values; previewing does not enable them. Placement and scale use normal settings persistence. Reference minimum FPS/coordinate panel dimensions are retained; full widget contents, typography and visual parity remain unfinished.

Validation: 30 runtime + 3 audit tests, emitted payload's 30 tests, fmt/Clippy, ten-hook exact source audit, pinned host check and optimized build passed. Real HUD editor and bound-input capture scenarios passed on the integrated server, including restart persistence, Shift snapping bypass, no movement from a bound W key and no attack from a bound left mouse button. Inspected actual editor/scaled captures. Evidence: docs/evidence/hud-editor-validation.json. No row was promoted to full `validated` parity.

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

Validation: 35 runtime + 4 audit tests, 35 emitted tests, fmt/Clippy, exact 17-hook/state/module audit, pinned host check and optimized executable build passed. Actual native lifecycle scenarios passed cleanly on the integrated server: Nether travel, kill/death, UI respawn, pause, save/quit and second-world open, with held FPS release, retained coordinates preference, reactivation in next world and strictly increasing epoch diagnostics. Inspected next-world capture. Evidence: docs/evidence/lifecycle-validation.json. Native manifest binding rules rejected an enum literal binding during implementation; moved constants inside small typed helpers instead of widening the binding contract.

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

Added tools/play.py as a visible manual entry point using the existing NativeSession isolation. Each invocation allocates fresh homes and working directory under target/scenarios/manual-*, uses approved prepared resources read-only, opens a normal survival world, retains its log/world/preferences for inspection, and closes only its owned process. Optional --creative is available; the three inventory automation modules require survival. CLI help passed. The development build has audio/controllers disabled. README and docs/status.md now include the exact launch command and controls. Evidence: docs/evidence/manual-playtest-build.json.

## Targeting and rotation primitives (partial batch)

Added pure session-qualified target filtering and distance, yaw, armor, threat and health ordering, plus bounded shortest-path yaw/pitch stepping. Candidate input explicitly carries friend, enemy, team, visibility and health observations; none is yet wired to native entity snapshots. Mixed player/non-player armor or threat sorting falls back to distance for the whole eligible set: the reference pairwise fallback can be non-transitive, so this is an explicit deterministic modern interpretation. Reference AimAssist maps Threat to its armor-value comparator and Armor to its equipment-value comparator; host score construction must preserve that mapping when connected. No dependent module or rotation/input/action host hook is implemented in this slice.

Validation: workspace fmt and Clippy passed; 51 runtime and 4 audit tests passed, including three new focused tests; emitted payload 51 tests passed. Exact native stage/audit passed (22 hooks, one state, nine support modules), and pinned horizon-client cargo check passed after correcting a payload namespace import. No executable gameplay scenario was run. Evidence: docs/evidence/targeting-foundation.json. The feature ledger is unchanged because these isolated primitives do not establish an integrated feature. Continue live entity snapshots, reference score calculation, rotation ownership and normal host action/input paths before enabling combat modules.

## Sprint host-input baseline

Registered Sprint and its default-off Cancel Invis setting. The existing client tick adapter feeds the host sprint key used by local and remote movement, tracks its per-client synthetic ownership and preserves a physically held key during cleanup. Host sprint eligibility and movement remain in the normal native path. This is an incomplete baseline: shared movement arbitration, Scaffold integration, reference condition parity and controlled-input interactions remain open. No live targeting/rotation snapshots or other requested modules were added.

Validation: workspace formatting and Clippy (all targets, warnings denied) passed; 52 runtime and 4 audit tests passed, including a focused per-client ownership/defaults test; 52 emitted runtime tests passed. Exact stage/audit passed with 22 hooks, one state and nine support modules; pinned client check result is recorded in docs/evidence/sprint-core.json. No executable build or resource-backed scenario was run. Sprint is implemented-core only; no full port or host validation is claimed.

## World render foundation batch

Added host-independent world-space line/box primitives and a bounded projectile
integrator. The integrator applies the reference's 0.99f drag before gravity,
accepts a host collision query, stops at the returned impact and caps work at
4096 steps. Returning the impact position instead of the reference's
post-collision full-step vertex is an intentional modern interpretation; the
caller must select the nearest block/entity impact. This is a foundation only:
there is no native line adapter, entity/block snapshot, world-text or filled
geometry path, projectile launch/collision adapter, module registration or
visual validation. None of the assigned feature rows has been promoted.

Validation: 52 runtime tests (including four new focused cases), four audit
unit tests, 52 emitted-payload tests, rustfmt and workspace Clippy with warnings
denied passed. Exact native stage/audit passed (22 HEAD hooks, one owned state, nine support
modules); the pinned client cargo check passed. No game executable or resource-backed scenario ran.
BlockOverlay's transparent-fill boundary remains unresolved.

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

## HUD client Panic batch

Added Panic as a bindable Utility module, retaining the existing F10 default. The reference's optional Re-enable setting records the active modules and restores them on the next activation; a module change or session/profile transition clears the pending restore. Panic remains momentary, and imported profile state cannot trigger it. Reference default visibility remains open. No other HUD family modules, Text GUI or ClientSettings fidelity are claimed in this batch.

Validation: workspace formatting and Clippy (all targets, warnings denied) passed; 49 runtime and 4 audit tests passed, including a new restoration/invalidation case. Final payload tests passed (49); exact native stage/audit passed (22 HEAD hooks, one owned state, nine support modules), and the pinned client cargo check passed. No game executable or real host scenario was run, so Panic is implemented-core, not validated.

## HUD helper and legacy mouse triage

Reviewed MouseDelayFix against the pinned host's current-orientation attack rays and classified its 1.8.9-specific interpolated-look workaround as not applicable. Classified ScoreboardVisibleScorePredicate as an internal helper rather than a separate module; its owner filtering remains required by the unfinished Scoreboard. Details and limits are in docs/specs/hud-triage.md. No HUD module is newly registered and no host scenario or visual parity is claimed.

Validation: workspace fmt and all-target Clippy passed; 51 runtime and 4 audit tests passed, as did 51 emitted-payload tests. Exact staging and audit passed with 22 HEAD hooks, one owned state and nine support modules. Pinned-client cargo check passed. No executable game scenario was run.

## World-line host integration and partial StorageESP

Registered an initial StorageESP module with real loaded block-entity
enumeration and chest/furnace color and toggle settings. A cancellable HEAD on
`Game::fishing_line` preserves the pinned host fishing lines and appends
world-space outlines to the real renderer's depth-tested line batch. The
normal module registry, binds, profiles and panic apply. The specification in
`docs/specs/storage-esp-baseline.md` records the copied host source hash and
substantial missing behavior. This is an initial partial module, not parity:
fill and through-wall rendering, other containers and controls, open-chest
handling, model bounds, and double-chest behavior remain open. All other
requested render modules, entity/camera snapshots, text/quads and collision
queries remain unfinished; no shared targeting adapter was introduced.

Validation: 53 standalone runtime tests, four audit tests, 53 emitted-payload
tests, formatting, and workspace Clippy with warnings denied passed. Exact
native stage/audit passed with 23 HEAD hooks, one owned state and ten support
modules; pinned host client cargo check passed. The initial flattened-payload
import failed the host check and was corrected before the successful rerun.

The optimized pinned client and control tool built successfully. Fresh isolated
resource-backed integrated-server chest scenarios completed
and exited cleanly; enabled captures visibly showed the outline, and the
integrated disabled capture showed its removal after the binding toggle.
Artifacts remain under `target/scenarios/storage-esp-nlz84kyn` and
`target/scenarios/storage-esp-znyv8l1n`; they are not committed. The first
integrated attempt failed because its block-targeting call ran before the
server's block update arrived; the final scenario uses a fixed camera and was
rerun in a fresh home. These tests do not validate furnace, occluded storage,
other features or reference parity.

## AutoTotem survival transaction baseline

Added a host-connected AutoTotem baseline. It observes the actual offhand,
opens the survival player inventory when configured, and swaps the first
available totem through the normal menu engine using the shared container
claim, cursor fingerprint and revision gate. It closes only an inventory it
opened itself. Inventory-only and open/close options and a delay range are
persisted. Creative is guarded out. Silent opening, randomized slot choice,
extra timing randomization, the count display, and actions without a visible
inventory remain unfinished; this is not full reference parity.

Validation: 53 runtime tests, 4 audit tests and 53 emitted-payload tests passed;
formatting and all-target Clippy passed. Exact native stage/audit and pinned
client check results are recorded with this milestone. No game scenario was
run, so the feature remains implemented-core.


## HUD resource-name localization

The coordinates biome and potion effect labels now resolve their resource keys through Horizon's active language, falling back to readable resource IDs if a translation is absent. Namespace handling is covered by a focused runtime test. This is a partial localization improvement, not full HUD or client localization; translated glyph coverage and real host language scenarios remain open.

Validation: formatting and Clippy (all targets, warnings denied) passed; 50 runtime tests, 4 audit tests and 50 emitted payload tests passed. Exact native staging/audit passed (22 HEAD hooks, one owned state and nine support modules), and the pinned client cargo check passed. No game or visual scenario was run. ClientSettings, Text GUI, icons, typography and Windows date conventions remain unfinished.

## Movement-input and Parkour work in progress

Started a per-tick input request resolver and routed the existing Sprint injection through it. Added an experimental Parkour adapter that checks the host player collision box at the current position and one block ahead, and temporarily presses jump when both checks are clear. Its ownership cleanup preserves a physically held jump key. This is not a complete shared movement arbitration layer: other axes, competing module requests, controller input, lifecycle interactions and real host behavior remain to be integrated and validated. SafeWalk, KeepSprint, WTap, NoSlowdown and AutoLadder remain unimplemented. Parkour is not claimed shipped or validated.

Workspace formatting and Clippy passed; 53 runtime and 4 audit tests passed. The first exact native stage/audit and pinned-client check passed for the input foundation before Parkour was added; the updated exact stage/audit (22 hooks, one state, ten support modules), emitted 53 tests and pinned-client check also passed. No resource-backed scenario has been run for this work.

## Parallel implementation round integration (2026-10-29)

Ran combat/movement, render, inventory and HUD/UI workers concurrently in separate worktrees and merged their branches into main. Shipped this round: Sprint (baseline native sprint input), Panic (configurable bind and optional re-enable), partial StorageESP (depth-tested chest/furnace outlines through the world-line pipeline, with integrated-server host scenarios), partial AutoTotem (guarded survival offhand swap), experimental Parkour (host collision query + jump input), Refill selection/scatter/cursor fixes and localized biome/potion HUD names. Added unconnected groundwork: targeting filters/sorts/rotation stepping, world line/box/projectile primitives, clicker Normal/Extra timing and an initial movement-request resolver. Triaged MouseDelayFix (not applicable on the pinned host) and the scoreboard predicate (internal helper).

Most worker assignments ended well short of scope; AimAssist, TriggerBot, both clickers, SafeWalk/KeepSprint/WTap/NoSlowdown/AutoLadder, ESP/Tracers/Nametags and the other render modules, InventoryManager and the consumable modules, ClientSettings, Text GUI and the HUD-module family remain unimplemented with no external blocker identified. docs/status.md lists the remaining work per area.

Merge fixes: unioned the ModuleId registry (23 modules), combined the world-lines and parkour mixin support modules, and replaced an inner doc comment in the clicker timing file that xtask staging rejects. Updated hooks.md for the parkour support module.

Validation on integrated main: fmt and all-target Clippy (warnings denied) passed; 67 runtime, 4 audit and 67 emitted-payload tests passed; exact native stage/audit passed with 23 HEAD hooks, one owned state and 11 support modules; pinned client cargo check passed. The optimized executable was not rebuilt after the merge and no post-merge game scenario was run. Evidence: docs/evidence/parallel-round-integration.json.

## Post-merge validation, re-pin and shared input/inventory recovery (2026-10-29)

Re-pinned Horizon from `e1b2b0b` to Nightly `9669600` (`nightly-source/2026-10-28-9669600`, which includes the stable hook points and Windows native builds). All 23 hooks resolved unchanged, the official stage/audit/check passed and the source hashes were updated. Main HEAD `c6af857` was also evaluated: it plans and compiles, but it isn't a tagged Nightly. The stable hook points fit only tick/frame/HUD; the remaining hooks have no stable equivalent. Details and the packaging time-box are in docs/decisions.md and docs/evidence/repin-packaging-eval.json.

Inventory transactions are now bounded. The gate resolves each click by its visible slot/cursor effect, a revision change without effect, or a 1.5 s deadline, and stays stalled after a no-op until content changes. Planners close only inventories they opened. A 5 s scheduler watchdog recovers any planner that holds the container claim without progress. AutoFish uses the same gate. This also removes a latent remote hang: a vanilla server sends no state-id change for a click that matches the prediction.

Movement input has one final writer. It works from the host's canonical keys and toggle mappings, records host key events through the `mapped_key` and `control_key_down` adapters, never reads raw Left Ctrl/Space, and hands owned axes back on screens, pause and unfocused frames. Sprint and Parkour are ported onto it.

`xtask build` writes a build identity beside the executable. `play.py` and all scenarios refuse stale or unidentified executables, which corrects the earlier "current client was rebuilt" ambiguity. New fresh-home scenarios: `tools/smoke_scenario.py` (integrated server) and `tools/lan_scenario.py` (controlled localhost LAN with two owned processes). They found an AutoArmor bug: vanilla armor attribute modifiers use slot groups, so every piece scored 0 and upgrades never happened. Fixed.

The ledger gains a `kind` column. `tools/ledger_triage.py` resolved all 243 "inherited/HUD; review" rows from the reference text: 28 modules, 43 submodes, 27 submodules and 145 helpers. Sprint, Parkour, AutoTotem, AutoArmor and Panic move to `integrated` with state-machine/cleanup/conflict fields. None is `validated`, because reference parity remains open. The module registry is now one table (`src/core/registry.rs`; the generated values were checked identical against the old matches), and `ui/gui.rs` is split into `gui/{mod,input,render}.rs`.

Validation: fmt, Clippy (all targets, warnings denied), 85 runtime + 4 audit tests, 85 emitted-payload tests. Exact stage/audit: 23 HEAD hooks, one owned state, 11 support modules. Optimized build and horizon-ctl rebuilt from the same stage. Scenarios on that build: integrated 27/27, LAN 16/16 (docs/evidence/postmerge-validation.json). Not covered: window focus loss, real keyboard rebinding events, switching between two different worlds, and third-party servers.

Not done this round: the entity snapshot/rotation, packet, camera/world-text foundations and their consumers (next round's worker table is in docs/status.md); moving tick/frame/HUD to stable hook points; the packaging owner decision (user) and launcher install proof; and splitting `core/mod.rs` beyond the registry (1750 lines).

## Build tooling fixes (2026-10-29)

`xtask build` now compiles `horizon-ctl` in the same cargo invocation as the client and records its hash (`control_sha256`) in the build identity. `native_session.verify_build` checks both executables. The audit compares the plan with the packed `target/bundle` snapshot that plan/apply consumed, not the live sources. The build identity is taken from that snapshot after staging, so editing sources during a build can neither fail the audit nor mislabel the result.

Verified by making an AST edit to `mixins/vape-rs/parkour.rs` during staging. The build succeeded, the edit was absent from the staged source, `verify_build` refused the build as stale while the edit existed, and accepted it after the revert. xtask Clippy (warnings denied) and its 4 tests pass; the integrated smoke scenario passed 27/27 on the rebuilt executables.

AGENTS.md now says milestones are committed directly to main, with branches/worktrees only for subagent or parallel-worker tasks. Removed about 2 GB of re-pin/packaging evaluation output and 30 uncited debugging scenario directories from `target/`. Scenario directories cited in docs/evidence and manual playtests are kept. Re-running `tools/stock_bundle.py` needs the evaluated horizon-mixins revision rebuilt first (`git archive` from target/upstream/horizon-mixins, then `cargo +1.95.0 build -p horizon-mixins-cli`).

## Entity snapshots, target filter and AimAssist (2026-10-29)

Started the next round's Worker A dependency inline. `targeting::entities` holds owned per-frame entity views: ids are paired with the session epoch, positions are interpolated at the render partial tick, each view carries the tick box, health, invisibility, held/worn items with armor and weapon scores, and line of sight. It also holds the reference rotation math (`RotationUtil.C/a/p/H/T`). `targeting::filter` ports `EntityTargetFilterValue`: its seven toggles, local friends/enemies, ignore naked/invisible/behind walls and the summary text. `inventory::item_limit` ports the `ItemLimitData`/`ItemHelper` groups (swords, axes, pickaxes, shovels, blocks, fists, food, potions, beds, `slotN`, id/display name). The host module `vape_combat` builds snapshots from `Game::mobs` (remote players named through `remote_player_names`), plus LAN guests when hosting.

AimAssist's Simple mode is a step-for-step port of `AimAssistRotationSubModule`, including its forces, velocity buffers, boosts, drift and sensitivity scaling, with all reference settings and defaults. It runs from a new `aim-assist-look` HEAD on `pad_look`, between host mouse look and render, and turns through the host `Entity.turn`. Disabling it, changing a setting, a session change or a non-interactive context clears its state. Adaptive mode is not ported. Teammate and bot checks wait for AntiBot.

Validation: fmt and Clippy (all targets, warnings denied) pass; 95 runtime + 4 audit tests and 95 emitted-payload tests pass. Exact stage/audit: 24 HEAD hooks, one owned state, 12 support modules. Pinned client check and fast build pass. New fresh-home `tools/aim_assist_scenario.py`: integrated 14/14. It covers no turn while disabled, behind a screen, with Mobs off, or without attack held under Require mouse down; turning to within 0.6 degrees of a NoAI zombie; and profile restore across a restart. Smoke regression: 27/27. Evidence: docs/evidence/aim-assist.json. Not covered: Adaptive mode, player and LAN-guest targets, remote servers, strafe increase, vertical aim, and friends, enemies and walls in the host.

Next: TriggerBot and the clickers on the same snapshot; SafeWalk through an edge back-off hook (Worker C); the packet boundary (Worker B); world text/camera (Worker D).

## SafeWalk, TriggerBot and Velocity (2026-10-29)

SafeWalk ports the reference move-only sneak flag. A library-side support module (`vape_safe_walk`) marks the local `PlayerMotion` from the movement writer before each tick. A cancellable HEAD on the host's `staying_on_ground_surface` then makes Horizon's own edge back-off run, without a sneak key, crouch pose or slowdown. Direction Check is ported.

TriggerBot is a full port of the reference tick/input state machine: 50 ms press gate, synthetic click and release, Require mouse down with Ignore activation click, Air crits, Shield check, Select first hit with its 1 s grace, mouse-over delay, extra delay, and early-hit and miss chances. It reads the client attack-strength ticker and clicks through the host input path (`vape_macros::attack_once`).

Velocity ports the entity-motion and explosion rules, delayed Ticks scaling, Kite mode, the facing checks and the reference `scaleMotion` truncation. The inbound-packet boundary is an observe-only HEAD on `handle_packet`, because its body has macros. Replacements land right after the host's own assignment, guarded by the exact received motion. Explosion packets use a cancellable HEAD on `remote_explode`.

Host scenarios found two bugs, both fixed. TriggerBot initially read the integrated server's `attack_ticks` and landed one hit in 40 ticks. A disabled AimAssist reset Velocity every frame through a shared reset; resets are now per module, with a regression test.

Validation: fmt and Clippy pass; 102 runtime + 4 audit tests and 102 payload tests pass. Stage/audit: 27 HEAD hooks, one owned state, 13 support modules. Host check and fast build pass. Fresh-home scenarios on one build (docs/evidence/combat-movement-round.json):

| Scenario | Integrated |
| --- | --- |
| TriggerBot | 12/12 |
| AimAssist | 14/14 |
| SafeWalk | 13/13 |
| Velocity | 10/10 |
| Smoke | 27/27 |

AutoClicker's engine (Normal/Extra/Extra+ timing, hold/release cycle, jitter, break blocks) is drafted in `docs/drafts/clicker.rs` but not yet registered. (Registered 2026-10-30; the draft was removed.)

## Single player on the integrated server only (2026-10-29)

At the user's request, the mod supports and tests single player only on Horizon's default integrated server thread. All scenarios, `play.py` and `scenario.py` lost their alternative-path options, and `native_session.py` pins `HORIZON_SERVER_THREAD=1` so an inherited environment can't switch modes. Three hooks that only ever fired on the alternative path were removed: Velocity's two local knockback hooks and `session-dimension`, whose callers run only in the server Game here. That leaves 26 HEAD hooks, one owned state and 13 support modules. Evidence, ledger and docs now record integrated-server and LAN results only.

Validation after the removal: fmt, Clippy, 102 runtime + 4 audit tests and 102 payload tests pass. Stage/audit: 26 HEAD hooks. Fast build passes. On that build: AimAssist 14/14, TriggerBot 12/12, SafeWalk 13/13, Velocity 10/10, smoke 27/27, LAN 16/16 and lifecycle 7/7. Lifecycle's dimension invalidation now comes from `session-remote-dimension`. Evidence: docs/evidence/combat-movement-round.json (`single_player_only_build`).

## Re-pin to Horizon main (2026-10-29)

Re-pinned from Nightly `9669600` to Horizon main `cc7bcbf` (781 commits; Minecraft 26.3 / protocol 777 and all 26 hook targets unchanged; `NATIVE_HOOKS_API` 2). The drift was small. `hud.rs` moved into the new `horizon-client-render` crate (still re-exported as `horizon_client::hud`), so its pinned hash path changed. Clear Water's copied `frame_fog` snapshot gained the three new `FogInputs` fields and the Horizon fog toggle.

Validation: fmt, Clippy, 102 runtime + 4 audit tests and 102 payload tests pass. Stage/audit: 26 HEAD hooks. Host check and fast build pass. Passing scenarios:

- smoke 27/27, LAN 16/16, lifecycle
- AimAssist 14/14, TriggerBot 12/12, SafeWalk 13/13, Velocity 10/10
- profile, NoHurtCam, AutoTool, AutoTool guards, StorageESP, `scenario.py`

TriggerBot first scored 7/12: on this main a NoAI zombie burns in daylight, and Fire Resistance did not stop that damage. The scene now runs at night with `spawn_mobs` off.

Failing: the GUI, HUD editor and HUD detail scenarios fail in the same way on the previous pin, so they were already stale (GUI clicks and drags at fixed coordinates no longer have the expected effect). The text editor scenario under `xvfb-run` also fails at a GUI-dependent step; it was not re-run on the previous pin. Evidence: docs/evidence/repin-main.json.

## Shippable .hnmod on the published mixin tool (2026-10-29)

At the user's request, the project now builds as a `.hnmod` that Horizon's launcher can build. This settles the open packaging decision (docs/decisions.md).

- The tool pin moved to published, unpatched horizon-mixins main `07dab17`. At Horizon `cc7bcbf` it plans all 26 hooks; the four macro-target rejections recorded earlier no longer occur. The lifetime-state patch, the PR #2 pin and `tools/stock_bundle.py` are removed. CI builds the bundle instead of the patch test.
- The runtime owner is the `client-owner` support module: one `Arc<Mutex<Runtime>>` per thread behind the unchanged accessor, which now returns the `Arc` by value. The stock tool cannot put state on `Game<'_>`, and no lifetime-free Game field has a struct-literal constructor it could extend. The audit now rejects any owned state; its state stripper and test are gone (3 audit tests).
- The mod id is `vape_rs`: the registry's ids refuse hyphens. The settings folder is still `native-mods/vape-rs`.
- `tools/bundle.py` now emits `dist/vape_rs-0.1.0-dev.hnmod` (42 source files), and Horizon's own `cargo horizon-mod check` accepts it. The new `tools/launcher_build.py` follows the launcher's `native_mods::build` from the `.hnmod`: `cargo install --git` of the pinned tool with Rust 1.99.1, the GitHub archive of `cc7bcbf`, apply, and a release build with `horizon-client/mods`. `VAPE_RS_BUILD=launcher` makes the scenario tools run that executable.

Validation: fmt, Clippy, 102 runtime + 3 audit tests and 102 payload tests pass. Stage/audit: 26 HEAD hooks and 14 support modules on `07dab17`; host check and fast build pass. The launcher-route release build passes smoke 27/27. On the fast build: smoke 27/27, LAN 16/16, lifecycle, profile, AimAssist 14/14, TriggerBot 12/12, SafeWalk 13/13, Velocity 10/10. Evidence: docs/evidence/hnmod-packaging.json.

Not covered: an actual installed `horizon-launcher native-build` run (it writes to a real launcher home), Windows and macOS, and a tagged Horizon pin for a public release. The GUI, HUD editor and HUD detail scenarios were already stale before this change and were not rerun.

## Classic V4 click GUI (2026-10-30)

The user asked for the GUI to match the reference's classic Vape V4 click GUI, so the single window with a sidebar was replaced by the reference's frame layout:

- **Navigation frame.**
  - A "VAPE V4" header with search, settings (gear) and save (sync) icons. Search filters modules inside the navigation frame.
  - Combat/Render/Utility/World/Inventory/Network rows with icons, then a MISC section (Friends, Profiles with the selected-profile badge, Macros).
  - Footer: a Favorites star and a HUD-editor icon.
- **Frames.**
  - Every category, Favorites, Profiles, Macros, Friends and client Settings is a 110-unit frame with an icon, a title and a collapse chevron. Frames are draggable, raise on click and scroll when they don't fit.
  - A newly opened frame takes the top of the shortest column and keeps that spot.
  - Positions, open state and collapse state persist in `ui.frames` (the old `ui.window` is ignored). A loaded document without frames, such as an imported profile, keeps the current layout.
- **Module rows.**
  - A click toggles the module, which is accent-filled when on. Binds show right-aligned.
  - The dots, or a right click, expand inline settings with a favourite star.
  - Setting labels now truncate to the room left by their value.
- **Profiles.** CREATE NEW and IMPORT, then a name entry with an arrow submit. Rows show the active profile in white. A dots menu offers save, rename, export and delete; delete still uses the confirmation modal. The reference's online PUBLIC button is replaced by local import.
- **Macros and Friends** use the reference's "Type item name" entry with an arrow, and friend/enemy buttons, a "Use friends" switch and person rows.
- HUD and legit modules are listed in the HUD editor's centred frame.
- Registry categories now match the reference: AutoArmor/AutoTotem/Refill → Inventory, AutoFish → World, Sprint → Combat.
- Icons are authored 7×7 pixel bitmaps, not the reference images.
- The adapter draws scaled, aligned host-font text.

The six GUI-driving scenarios (GUI, HUD editor, HUD detail, text editor, AutoTool, NoHurtCam) were rewritten for the new coordinates. The rewrite surfaced two problems:

- The HUD editor's shift-drag grabbed the overlapping Keystrokes preview rather than Coords. The scenario now presses clear of it.
- The FPS widget's toggle sits under the centred HUD frame, so the scenario toggles FPS from its frame row.

Validation: fmt, Clippy, 104 runtime + 3 audit tests and 104 payload tests pass. Stage/audit: 26 HEAD hooks and 14 support modules; host check and fast build pass. On that build, in fresh isolated homes, all of these pass: GUI (11 checks), HUD editor plus binding capture, HUD detail, text editor (private Xvfb display), AutoTool, NoHurtCam, lifecycle, profile plus corrupt profile, and smoke 27/27. Evidence: docs/evidence/classic-gui-validation.json. Ledger: client.ui, client.profiles and client.friends → `integrated`.

Not covered: Macros frame editing, header search, and frame drag/collapse/scroll in a real session (unit tests only); GUI scales other than 1 and windows other than 800×600. The Text GUI overlay and notifications are not ported.

## Clickers, Reach and blatant movement modules (2026-10-30)

New core modules, all compile-only:

- AutoClicker and RightClicker. RightClicker adds a start delay and a sneak-only option.
- Reach, in Normal and Advanced chance modes.
- HitBoxes and Timer.
- Fly, using the 45° strafe speed logic with vertical speed.
- NoFall, in Normal and AntiCheat. AntiCheat sends a MovePlayerStatus true/false pair.
- NoSlowdown, with an item whitelist and deferred knockback.

Fly, NoFall, HitBoxes and Timer sit in the "Other" category, which the GUI lists only on an exact name search. NoSlowdown is "Hidden" and never listed.

New host hooks:

- entity-pick: Reach and HitBoxes ray range and box expansion.
- remote-attack: the Reach post-attack hook.
- timer-advance: a scaled `FixedStep::advance`.
- no-slowdown: suppresses the item-use slowdown.
- fly-move: replaces `move_self`.
- send-state, plus send-state-ground (MODIFY_ARG on the `on_ground` argument of `position_sender.update`).

The xtask audit now also verifies MODIFY_ARG declarations: selector, argument, ordinal and require. It strips their `({ helper })(arg)` wrappers when restoring the AST. `clock.rs` gained a pinned source hash.

Validation:

- `cargo check --all-targets` passes.
- Stage/audit passes with 33 hooks (32 HEAD + 1 MODIFY_ARG) and 17 support modules.
- The pinned horizon-client check passes with warnings only.
- No game scenario was run; these are not validated.

Not ported:

- Reach Misplace.
- HitBoxes interact-at clamp.
- NoSlowdown's Sprint re-enable.
- NoFall's jump-key detail.
- RightClicker's use claim and offhand whitelist.
- Reach and HitBoxes for LAN guests.

KeepSprint stays deferred. Evidence: `docs/evidence/clickers-reach-blatant-compile.json`.

## Click GUI interface font and GUI scale release (2026-10-30)

Text:

- The click GUI no longer scales the bitmap game font. All text and edit fields are drawn in Horizon's bundled interface font (Geist, through `ui_theme::Ui`), rasterized at the final on-screen size.
- A new support module, `smooth-text` in `ui_theme.rs`, exposes the theme's crate-private measure and line drawing as `Ui` methods.
- Fills, text and clips are now placed directly in screen space.
- Truncation and caret placement use a proportional advance estimate for that font instead of a fixed 6-pixel cell.

GUI scale: the slider's value still tracks the drag, but the layout only rescales on release. Rescaling mid-drag moved the slider under the cursor.

Validation:

- `cargo check --all-targets` and 108 lib tests pass.
- Stage/audit passes with 33 hooks and 18 support modules.
- The host check and fast build pass.
- A fresh isolated-home screenshot shows the main, Combat and Settings frames with crisp text and untruncated labels at GUI scale 1.
- The scale-on-release behaviour was not exercised in a live session.

## HUD/Game legit groups; Time Changer, Weather, Anti-AFK, Step, WTap (2026-10-30)

Category fix: the old "Legit" category put NoHurtCam, Clear Water,
NoJumpDelay and NoClickDelay under HUD. The reference's `HudModuleGroup`
splits legit modules into HUD (Coords, FPS, Compass, Clock, Keystrokes,
Armor Status, Potion Effects) and Game (the rest). The registry now uses
those two categories, and the HUD editor frame has the reference's
Favorite/All/HUD/Game tab bar (`HudModuleSelectorFrame`), defaulting to All.

New modules (compile-only):

- Time Changer (Game): the client world's time of day follows the hour
  setting every frame (the day count is kept).
- Weather (Game): Clear zeroes rain and thunder strength, Raining sets rain
  to 1; the strengths saved on enable come back on disable.
- Anti-AFK (World): random start delay, then small view turns and 100-200 ms
  walks towards a random point within a block (Keep close returns to the
  origin); the user's movement keys restart the delay. Walks go through the
  movement writer.
- Step (Other): a transient +0.4 `step_height` modifier while on the ground
  and not sneaking.
- WTap (Combat): on a remote attack against a (vulnerable) player, with the
  chance, forward is released through the movement writer after the release
  delay and handed back after the re-press delay. The writer runs again at
  attack time so the release hits the attack's tick.

Not ported: the HUD editor search box; Step's position packets and timer
slowdown; Anti-AFK silent aim. KeepSprint stays deferred.

Validation: `cargo check` passes; stage/audit passes with 33 hooks and 19
support modules; the pinned host check passes. No game scenario ran. The GUI
scenarios that click HUD-editor rows need their coordinates moved 16 units
down for the tab bar. Evidence:
`docs/evidence/legit-groups-wtap-afk-step-compile.json`.

## 2026-10-30 — Speed, LongJump, AntiFall (compile-only)

New modules, all on the client tick HEAD next to Fly:

- Speed (Hidden, as in the reference): Bhop ports BhopSpeed's stages
  (0.25 ground start, 2.149 jump multiplier, 0.66 and 1/159 decay, the
  vertical-collision stage reset) and AntiCheat B ports MineplexSpeed (jump
  on vertical collision, 0.42 strafe). Speed levels add 0.15 each and Jump
  Boost adds 0.1 per level to the jump.
- LongJump (Other): Boost 4.1, with Toggle turning it off on landing; ports
  the base/air speeds, the 0.666 and 1/50 decay and the movement yaw.
- AntiFall (Other): Speed Check and Fall Dist; above the void past Fall
  Dist, one pending pre-motion and then a lagback up by the fall distance
  (horizontal motion and fall distance zeroed). The 250 ms window or landing
  clears it, and so does the server position packet (now observed in the
  inbound hook).

The Fly move hook now binds the host's move, and a NaN component keeps the
host's value, so Speed and LongJump replace only the horizontal move unless
they jump. Fly takes precedence, then LongJump, then Speed.

Not ported: LongJump's vine/ice surface factors, the effect-duration gates,
AntiFall's Freecam check (Freecam not ported). OmniSprint is empty in the
reference (no behaviour). Strafe needs AimAssist's current target exposed.

Validation: `cargo check` passes; stage/audit passes with 33 hooks and 19
support modules; the pinned host check passes with 0 warnings. No game
scenario ran. Evidence: `docs/evidence/speed-longjump-antifall-compile.json`.

## 2026-10-30: tiled GUI frames, StorageESP through walls, Game group moved to Render

User-requested fixes before the next plan batch:

- Click-GUI frames tile in a grid of 116-unit columns. Each frame sits
  directly under the one above it in its column, so expanding module
  settings pushes the frames below down instead of overlapping them.
  Dragging a frame floats it under the pointer; releasing drops it into the
  nearest column at the row under the pointer. `ui.frames` stores
  `column`/`row` (old `x`/`y` positions are ignored, except the HUD editor
  frame's). Newly opened frames go to the bottom of the shortest column.
- StorageESP outlines now draw through walls. The fishing-line hook marks
  them with a negative width; a new cancellable HEAD hook on
  `horizon_client::gizmos::camera_relative` (support module `vape_on_top`)
  removes marked lines from the frame's depth-tested set and appends them to
  its always-on-top set on the next call. `render` itself cannot be hooked
  (closures/`?` are rejected by the mixin planner).
- NoHurtCam, Clear Water, NoJumpDelay, NoClickDelay, Time Changer and
  Weather moved from the HUD editor's Game group to the Render category; the
  HUD editor tabs are Favorite/All/HUD.

Validation: `cargo check`, fmt and lib Clippy pass; stage/audit passes with
34 hooks and 20 support modules; pinned host check passes. No game scenario
ran (implementation-first request), so through-wall rendering and the grid
drag are compile-only.

## 2026-10-30: ESP, Tracers, StorageESP container types

- StorageESP covers the reference toggles and colours for chests, trapped
  chests, hoppers, furnaces (plus blast furnaces and smokers), dispensers,
  droppers and shulker boxes, identified by block name. Ender chests have
  no host block entity and stay pending (needs a block scan), as do fill and
  Outline open.
- ESP (`render.esp`, 3D mode only): through-wall boxes around other
  players at their interpolated position, grown by the HitBoxes expansion
  with "Hitbox" and HitBoxes on, plus the true box in the inverse colour at
  alpha 70 with "Show Normal".
- Tracers (`render.tracers`): through-wall lines from the first-person eye
  (one block ahead) to players, mobs and animals at eye height, with
  per-group toggles, distance ranges and colours, Color by distance, and
  Highlight if focusing (players looking at you within 5 degrees, using new
  `EntityView::look` head angles).
- Friend/enemy recolour settings (`friends.recolor`/`friends.color`,
  `enemies.use-color`/`enemies.color`) appear in the Friends frame and
  apply to ESP and Tracers.
- The runtime's `world_lines` builds all three; `world-lines-hook`
  converts them to host lines that go through the existing on-top path.

Validation: `cargo check`, fmt and lib Clippy pass; stage/audit passes with
34 hooks and 20 support modules; pinned host check passes. No tests were
added or run and no game scenario ran (implementation-first request), so
all of this is compile-only.

## 2026-10-30: Trajectories

- Trajectories (`render.trajectories`) predicts the held bow (drawn power,
  at least 0.1; "Ghost Bow Charge" draws a full shot while not drawn),
  snowball, egg, ender pearl, splash/lingering potion, experience bottle
  and trident (after 10 use ticks) with the modern launch speeds, gravity
  and pitch offsets, through the existing `world::predict` integrator.
- Each step stops at the host's block clip (`Game::clip_ray`) or the
  nearest live entity box grown by 0.3. The arc takes the aiming colour
  when it ends on an entity, whose box gets the target colour; otherwise
  it uses the trajectory colour with a small box at the landing point.
- Pending: the shooter's own velocity, crossbows, and the reference's
  exact aiming/target colour semantics.

Validation: `cargo check`, fmt and lib Clippy pass; stage/audit passes with
34 hooks and 20 support modules; pinned host check passes. Compile-only, with no tests and no game scenario.

## 2026-10-30: NameTags and ItemESP

- New cancellable HEAD hook `name-tags-hook` on
  `game::names::Game::name_tag_geometry`, with support module
  `vape_name_tags` placed in `names.rs` so it can call the host's private
  `name_tags` and `name_tag_quads`. Tags are drawn with the host font and
  background into the see-through set (through walls) and scaled about the
  camera, so a tag's size follows the reference scale without new geometry.
- NameTags (`render.name-tags`): players, animals and mobs groups with
  Health (half-hearts, coloured by fraction), Distance, Effects (absorption,
  currently always 0 because the host mob has no absorption field), Max
  Distance, Strength Indicator (weapon plus armor score against the local
  player's), Ignore Invisibles, Hide bots (no AntiBot yet), Auto Scale,
  Scale and Opacity (applied when not in line of sight). Friends show
  their alias and the friend colour. While players render, the host's own
  player tags are left out. Pending: equipment row and enchantment
  overlay, creative `[C]` for remote players, Calculate Effects.
- ItemESP (`render.item-esp`): tags over dropped items with names,
  `xN` counts, Distance, Group Items (proximity groups rebuilt each frame),
  Auto Scale, Scale, Whitelist Only and Allowed Items.

Validation: `cargo check`, fmt and lib Clippy pass; stage/audit passes with
35 hooks and 21 support modules; the pinned host check passes (its existing
private-interface warnings are unchanged). Compile-only: no tests and no
game scenario ran.

## 2026-10-30: Outbound packet modules

- New cancellable HEAD hook `outbound-packet` on `game::remote::Game::send`
  (every serverbound play packet the client sends to a remote server), with
  support module `vape_packets` in `remote.rs`. The decision core is
  `src/modules/blatant/outbound.rs`; the host keeps held packets in send
  order with due times and releases them directly to the session.
- Blink (`blatant.blink`, Utility): outgoing only; every packet held until
  Blink turns off, leaves the world or Auto send reaches Send threshold;
  Breadcrumbs every fifth held movement packet while moving, drawn as world
  lines. Pending: Bi-directional, Spawn fake, the GUI-screen stop.
- FakeLag (`world.fake-lag`, Utility): Latency holds each packet for Delay
  ms; Repel holds an attack on a living target whose hurt time is within
  ceil((Delay + 0..100 ms) / 50) ticks and everything after it until the
  deadline or the next attack. Pending: Repel's pre-tick hurt-time flush;
  Dynamic is 1.7-only in the reference.
- NoItemRelease (`combat.no-item-release`, Other): drops RELEASE_USE_ITEM in
  any screen, or when the held item matches Allowed items (swords, food,
  potions by default).
- PotionSaver (`blatant.potion-saver`, Other): drops movement packets while
  effects are active, on the ground, with no movement input.
- Regen (`blatant.regen`, Other): below Health x2 with food above 16, on the
  ground, not jumping, in water or riding, every fifth tick (tick count % 5
  == 1) sends 40 airborne status packets. Modern servers do not heal per
  packet, so this is a literal port with no expected effect there.

Validation: lib build, fmt and lib Clippy pass; stage/audit passes with 36
hooks and 22 support modules; the pinned host check passes (0 errors, the
existing private-interface warnings unchanged). Compile-only: no tests and
no game scenario ran.

## 2026-10-30: AntiDebuff; InvWalk and KeepSprint blocked

- AntiDebuff (`render.anti-debuff`, Render): every client tick, Remove
  Nausea and Remove Blindness take those effects off the local player's
  client effect list; Remove Slowness (with Remove Effects) takes off
  slowness and its `minecraft:effect.slowness` movement-speed modifier.
  Difference from the reference: Remove Blindness without Remove Effects
  is render-only there, but here it also lifts the client's blind sprint
  block, because the host reads blindness from the same list. No new hook;
  it runs from the existing client tick hook (`vape_legit::anti_debuff`).
- InvWalk is blocked on a host capability. `Game::tick` gates movement keys
  with a local `playing = screen == Playing` before building `Keys`, and
  the existing hook kinds (HEAD, MODIFY_ARG on a call) cannot reach it.
- KeepSprint is not ported. The host applies no client-side attack
  slowdown (vanilla's motion x0.6 and sprint stop in `Player.attack`), so
  the module has nothing to modify. Revisit if the host adds it.

Validation: fmt and lib Clippy pass; stage/audit passes (36 hooks, 22
support modules, unchanged); the pinned host check passes with 0 errors.
Compile-only: no tests and no game scenario ran.

## 2026-10-30: Cancellable inbound packets; InvWalk route

Rechecked blockers 2 and 3 against upstream. Neither horizon main
(`fca2169de`) nor horizon-mixins main (`5140cdf`) changes them, and no
open PR in either repo touches them.

- The inbound boundary was misdiagnosed. The pinned tool (`07dab17`) keeps
  macro bodies under cancellable HEAD too (its
  `cancellable_head_preserves_macros_on_continue_and_skips_them_on_break`
  test), as long as the helper returns `core::ops::ControlFlow<Result<()>>`
  spelled as the target writes it. `inbound-packet` is now cancellable.
  Every path still continues, so behaviour is unchanged. JumpReset,
  KnockbackDelay, Backtrack and Bi-directional Blink now need only
  holding and replay, not a host or tool change.
- InvWalk no longer needs a Horizon PR. Body-rewriting hooks can't reach
  `playing` inside `Game::tick`, because the function has closures and `?`
  (still true on mixins main). But both the local and remote paths pass
  their `Keys` to `MotionWorld::controlled_simulation_tick` (`Keys` is
  `Copy`), and `mapped_key` keeps `self.keys` filled behind every screen
  except chat and options. The planned route: the `Game::tick` HEAD
  records InvWalk keys in the `motion-flags` support module, and a
  cancellable HEAD on `controlled_simulation_tick` re-runs it once with
  them, behind a re-entry guard.

Validation: stage/audit passes (36 hooks, 22 support modules); the pinned
host check passes with 0 errors. Compile-only: no tests and no game
scenario ran.

## 2026-10-30: GUI frames reflow off-screen overflow

User request: tiled click-GUI frames reorder themselves when they run below
the screen.

- Each frame, after placement, a reflow pass measures every open frame's
  full unscrolled height (collapsed frames count only their header). A
  frame whose bottom would pass the screen bottom (6-unit margin) moves to
  the bottom of the shortest other column it fits in whole; the frames
  below it move up. Frames that fit nowhere stay and scroll as before. The
  frame being dragged is excluded. Moves change the stored `column`/`row`,
  so they persist with the next `ui.frames` save, like newly opened frames.
- Fixed newly opened frames always landing in column 0: the shortest-column
  choice read `panel_rects` after they had been cleared for the frame. It
  now uses the same measured heights. This was why
  `navigation_opens_category_frames_whose_rows_toggle_modules` failed on
  the previous commit.

Validation: 109 lib tests pass (new
`frames_past_the_screen_bottom_move_to_a_column_with_room`), fmt and
all-target Clippy with `-D warnings` pass; stage/audit passes (36 hooks, 22
support modules); payload tests and the pinned host check pass. No game
scenario ran.

## 2026-10-30: inbound packet holding and replay; JumpReset and KnockbackDelay

User request: hold and replay packets on the cancellable `inbound-packet`
boundary, replay through `handle_packet` with a guard, a bounded queue
flushed on disable/disconnect/world change, connection-critical packets
kept out, and JumpReset and KnockbackDelay on top.

- `vape_packets` now owns an inbound queue (256 packets). Replay calls the
  host's `handle_packet` with a thread-local guard so the hook steps aside;
  Velocity and the other observers still see replayed packets. Overflow
  replays the whole queue. A critical packet flushes the queue first and
  passes. Disable, disconnect, world open/close, respawn and dimension change
  flush through new calls in the session adapters; with the connection gone,
  the queue is dropped instead.
- KnockbackDelay (`modules/combat/knockback.rs`): holds the local player's
  velocity packet and everything behind it for the air delay (fewer than 3
  grounded ticks) or the ground delay, with chance, water check and the
  reference's 475 ms re-hit gap. JumpReset: remembers a suitable velocity
  packet, waits until the player's motion shows it, then taps jump through
  the movement writer (`Owner::JumpReset`) after the accuracy roll.
- Not ported or interpreted: JumpReset's `PotionRegistry.Z` check (unknown
  effect), the reference's hit counter (read as grounded ticks), and its
  target search (any player we face within 6 blocks).

Validation: fmt, all-target Clippy with `-D warnings`, and 113 lib tests (4
new knockback tests) pass; stage/audit (36 hooks, 22 support modules) and the
pinned host check pass with 0 errors. Compile-only: no game scenario ran, so
features.csv marks both `implemented-core`, not validated.

## 2026-10-31: Backtrack, Blink Bi-directional, Repel pre-tick flush

User request: build Backtrack and Blink's Bi-directional mode on the inbound
hold/replay, add FakeLag Repel's hurt-time pre-tick flush, and test ordering,
flush/drop rules and reconnect cleanup in a controlled LAN scenario.

- Inbound queue entries now carry an owner (KnockbackDelay, Backtrack, Blink)
  and a due time never earlier than the entry before (order preserved across
  owners). Limit raised to 1024. Blink holds everything non-critical until it
  sends; turning Blink off, switching to Outgoing only, or auto send releases
  the queue.
- Backtrack (`modules/blatant/backtrack.rs`): follows the target's true
  position through every move/sync/teleport packet, holds updates while the
  target backs off along the line of sight, flushes on a new/removed target,
  our own velocity packet, no target, or disable/world change. Draws the
  server-position box. Latency is a range setting.
- Repel: tracks attack-to-damage delays (hurt event/animation) and flushes
  held outbound packets in the client tick when the target's hurt time falls
  to the expected delay.
- `tools/inbound_hold_scenario.py`: LAN host + guest, isolated homes. 27/27
  checks: held chat order, flush before a teleport, drop on disconnect (old
  lines never replay, new connection holds from empty), Backtrack lag and
  convergence, chat order behind held updates, disable release, Repel hold
  and deadline, no panics.

Validation: fmt, Clippy -D warnings, 118 lib tests, payload test, stage/audit
(36 hooks, 22 modules) and host check (0 errors) pass. Not covered by a game
scenario: Repel's hurt-time flush itself (unit test only); Blink fake player
still not ported; blink inbound delay differs from the reference (critical
packets pass).

## 2026-10-31: re-pin to Horizon main 8f91f89, horizon-mixins 0e9229d

User request: bump the pins to the latest remote main.

- compatibility.json and the mixin manifest name Horizon `8f91f896a5d47d034318569940e775d8bd854bd6` and horizon-mixins `0e9229d88d7b6ab18add048de680d873f04cd9b5`; source hashes refreshed; the deleted `game/mod_input.rs` hash removed.
- `mixins/vape_rs/name_tags.rs` passes the new `unit` argument (`NAME_TAG_UNIT`) to `name_tag_quads`.
- KeepSprint's Horizon dependency (remote-attack slowdown, #3251) is now in the pin.

Validation ([repin-main-2026-10-31.json](evidence/repin-main-2026-10-31.json)): fmt, all-target Clippy `-D warnings`, 118 lib tests and the payload test pass; stage/audit (36 hooks, 22 support modules) pass; host check and fast build pass. Fresh-home scenarios on the new executable: smoke 27/27, LAN 16/16, inbound hold/replay (LAN) 27/27.

## 2026-10-31: render worker (branch worker/render)

Render modules on real host data, plus two new hooks (38 hooks, 25 support modules):

- `overlay-hook` (HEAD, `fov::draw_spyglass`) and support module `vape_overlay`: screen-space overlays from a projection built from the gizmos camera with the host's FOV, view bobbing and nausea spin. Used by ESP 2D (bounding box, health bar, name, background, priority-only), Health, Arrows, the NameTags equipment row and creative marker, SpawnerFinder and MurdererFinder.
- `esp-outline-hook` (cancellable HEAD, `glowing::mob_outline`): ESP Outline goes through the host's glowing-outline pass.
- `vape_remote_tab`: player-list display names and game modes (creative marker, Use Displayname).
- StorageESP: fills, ender chests and shulkers (host block entities), outline-open. Explosions: TNT fill. Tracers: player/enemy/bot filters.
- Trajectories: vanilla shooter motion (horizontal only on the ground), loaded crossbows (arrow 3.15, firework straight 1.6 with no drag), drag-parameterised prediction.
- Search: StringList `id[=AARRGGBB]`, range, only-caves, tracers. Loaded chunks are scanned two per frame, sections skipped with `may_hold`, and the state match is cached. The cache resets when the query changes.
- SpawnerFinder: spawner block entities (`next_data().id()`), mob whitelist, distance-scaled label.
- MurdererFinder (World): players holding a listed item, cleared on world change; the chat callout is not ported.
- PropHunt (World): falling blocks (interpolated) and inbound `BlockUpdate` positions for 200 ticks, boxed and filled.

Validation (after merging main): fmt, all-target Clippy `-D warnings`, 148 + 3 lib tests and the payload test pass. Stage/audit (38 hooks, 25 support modules) passes. Host check passes (0 errors; the existing visibility warnings only). Not yet run: the fast host build and `tools/render_scenario.py` (on/off screenshot diffs for single player and LAN). This was stopped at the coordinator's wrap-up, so no render module from this branch is `validated` yet.
## 2026-10-31: validation — GUI reruns, movement scenario, NoSlowdown hook fix

Validation worker (branch `worker/validation`, pin 8f91f89 / 0e9229d).

- GUI scenarios rerun on the re-pinned build, all pass: gui 11, text editor 6
  (private display), HUD editor 9, binding capture 2, HUD detail 7, profiles
  6, corrupt profile 2, lifecycle 7 ([validation-gui.json](evidence/validation-gui.json)).
  `gui_scenario.py` coordinates follow the Render frame order and Friends rows.
- `tools/movement_scenario.py`: fresh survival home on the integrated
  server; each module toggled through its bind. Timer (client/server tick
  ratio 1.0 / 2.0 / 1.0), Fly (rise, hover, Speed 0.5, gravity back), NoFall
  Normal (13-block drop 10 -> 0) and AntiCheat (10 -> 1, the reference's
  3-block status pairs), Speed Bhop (15.2 vs walk 8.5 blocks per 40 ticks,
  hops) and AntiCheat B (16.8), LongJump (7.6 vs sprint jump 3.6, toggles
  off), Step (climbs a 1-block wall), AntiFall (lowest 142.4 vs 116 off;
  lagback zeroes horizontal motion only, as the reference, so on this
  vanilla-like server the player bobs below the edge), NoSlowdown, Anti-AFK
  (acts after the 10 s delay, stays put), no kick. Distances are scaled by
  the client ticks that really passed. **52/52**
  ([validation-movement.json](evidence/validation-movement.json)).
- Bug fixed: NoSlowdown had no effect after the re-pin (shield walk 1.24 on
  and off). Its hook targeted `MotionWorld::controlled_simulation_tick`, which
  the pinned `game.rs` no longer calls; it now targets
  `controlled_simulation_tick_with_sprint_window` (off 1.23 / on 6.32 / walk
  6.18 blocks per 30 ticks).

## 2026-10-31: validation — combat scenario, LAN PvP scenario, NoItemRelease and JumpReset fixes

Pinned Horizon 8f91f896a / horizon-mixins 0e9229d, fresh isolated homes.

- `tools/combat_scenario.py` (integrated server, survival): **39/39**
  (`combat-unwn75pj`). AutoClicker holds at 11.0 server-logged attacks/s
  for 10-12 CPS, stops on release and on disable; RightClicker 13 snowballs/s
  and FastPlace 16 (stack of 16 used up) against the host's 5/s repeat, back
  to 5 after; HitBoxes takes a click 0.25 beside a cow, misses off/after;
  NoItemRelease: a 14-tick eat finishes server-side (0 / 1 / 0 apples with
  off / on / after, 44-tick full hold eats 1).
- `tools/pvp_scenario.py` (LAN host plus a vape-rs guest): **23/23**
  (`pvp-guest-ht8rynoz`). KnockbackDelay 500 ms: the push lands 503 ms after
  the hurt (0 ms off); disabling mid-hold released it 41 ms after the tap; a
  push held over a disconnect never replays after reconnecting. JumpReset:
  push-arc peak 0.965 off and after, 1.252 on (a jump). WTap: 4 unsprinting
  ticks after a sprint hit against 1 off.
- Fixed: NoItemRelease matched `PlayerAction` action 5, which is DROP_ITEM in
  this protocol; RELEASE_USE_ITEM is 6 (`mixins/vape_rs/packets.rs`).
- Fixed: JumpReset never jumped. Its landed-motion check ran in
  `vape_packets::tick` after the same tick's input resolution, so the jump
  key came a tick later with the player in the air. The packet tick (held
  packet replay, JumpReset, outbound flush, Regen) now runs first at the
  client tick HEAD, as packets precede the tick in vanilla
  (`mixins/vape_rs/adapters/tick.rs`). inbound_hold_scenario to be rerun.
- Reach: the client pick extends (cow at 3.64 picked with 3.9, none off and
  after) but the server records no attack. Horizon's integrated/LAN server
  re-picks a guest attack with its own 3-block reach (`lan_actions`
  guest_attack -> look_at_entity + attack) instead of attacking the sent id
  as vanilla handleAttack does. Blocked on Horizon.
- Regen and PotionSaver: off and on measure the same (health +3 in 1 s; a
  speed effect 599 -> 519 ticks over 4 s). This server ticks the player once
  per server tick whatever movement packets arrive, so neither can change
  its clock here. Host-limited, not validated.
- Evidence: docs/evidence/validation-combat.json, validation-pvp.json.

## 2026-10-31: validation — render/legit frames, inbound hold rerun, Time Changer and AntiDebuff fixes

Pinned Horizon 8f91f896a / horizon-mixins 0e9229d, fresh isolated homes.

- `tools/legit_render_scenario.py` (integrated server, frames from the host's
  own screenshots): **40/40** (`legit-render-8bkfza5i`).
  - Time Changer hour 0: the client reads 18000 while the server stays at
    6002. Sky luma goes 185 -> 8 -> 189, and the server's time is back after.
  - Weather Raining under a clear server sky: sky luma 189 -> 93 -> 190. The
    server's weather stays clear.
  - Fullbright in a sealed room at midnight. Night Vision: luma 8.5 -> 70 -> 8.3.
    Gamma (relaunched with the mode): 8.3 -> 87 -> 8.2.
  - Clear Water under water: frame diff 20.5 on, 2.1 after a fresh dive.
  - AntiDebuff (Remove Effects on): slowness, nausea and blindness leave the
    client list, and walking goes back to 6.33 from 2.53 blocks per 30 ticks.
    Disabled, new debuffs stay and slow again.
  - ItemESP: diff 2.47 for an item behind a wall, 0.0 after.
  - NameTags (animals): diff 1.32 for a still cow, 0.10 after.
- Fixed: Time Changer left the forced time in place after disable. The modern
  server only sends the clock when it jumps, so the module kept writing over
  the only copy. It now tracks the real clock beside the written one and
  restores it on disable (`mixins/vape_rs/legit.rs`).
- Fixed: AntiDebuff removed slowness from the effect list, but the speed
  attribute modifier stayed (the server's attribute sync re-adds it), so
  walking stayed slow. The modifier is now stripped every tick while the
  module is on (`mixins/vape_rs/legit.rs`).
- `tools/inbound_hold_scenario.py` (LAN host plus a vape-rs guest): **27/27**
  (`hold-guest-p1v7oao5`) after the packet-tick reorder.
  - Blink Bi-directional, including the teleport flush and a reconnect with
    nothing replayed.
  - Backtrack: the pig is 3.42 behind its server position while held, 0.31
    without, and released at once on disable.
  - FakeLag Repel: the first hit is not delayed; the hit on a hurt target is
    held and lands by its deadline.
  - Test fixes: the Backtrack baseline now allows interpolation lag (< 1.2),
    and the Repel check is timed on two attacks three ticks apart.
- Evidence: docs/evidence/validation-legit-render.json, validation-hold.json.

## 2026-10-31: validation — utility modules, delay removals, a frame-time measurement

Pinned Horizon 8f91f896a / horizon-mixins 0e9229d, fresh isolated homes.

- `tools/utility_scenario.py` (integrated server, survival): **18/18**
  (`utility-kppox6bq`).
  - Refill moves 4 soups from the main inventory into the free hotbar slots in
    0.6 s, leaves the stone alone, closes the inventory and turns itself off.
    Off, nothing moves.
  - ChestSteal takes all three stacks and closes the chest in 0.59 s. Off, the
    open chest is left alone. Closed by the player mid-steal, the clicks stop
    with the screen (1 of 78 items taken, the rest stays in the chest).
  - Binds do not reach a menu screen, so the toggle is not used mid-steal.
  - AutoFish: a Lure III cast catches nothing in 40 s without the module. With
    it, the bite is reeled in and a catch arrives (24 s).
- `tools/delays_perf_scenario.py`: **39/39** (`delays-perf-_xnqd1x5`).
  - NoJumpDelay under a two-block ceiling: 7 / 21 / 7 take-offs in 60 ticks
    with off / on / after.
  - NoClickDelay: a click on a cow right after a missed click is dropped
    without the module (0/3), lands with it (3/3), and is dropped again after
    (0/3).
- Performance (ctl bench, 10 s each). The run had twelve render/HUD modules on
  (hud coordinates/fps/keystrokes/armor/potions, ESP, Tracers, StorageESP,
  NameTags, ItemESP, Trajectories, Fullbright) in front of 24 cows and
  8 chests.
  - CPU per frame: avg 1.75 -> 4.11 -> 1.86 ms for off / on / after.
  - CPU p99: 5.5 -> 40.5 -> 7.8 ms. Spikes like these with the modules on are
    a real cost and are not yet profiled.
  - The frame rate (118 / 30 / 30 fps) followed the compositor's pacing of the
    window and is not attributed to the modules. In an earlier run, frames
    with every module off after the bench ran at 30 fps too.
  - The check is on CPU time (within 3 ms of plain).
- Not covered: localization (client.localization is not-started, nothing to
  validate).
- Evidence: docs/evidence/validation-utility.json, validation-delays-perf.json.

## 2026-10-31: inventory worker, arrangement planners (InventoryManager, InvCleaner, AutoHotbar, ArmorSwitch)

- Snapshot additions (`mixins/vape_rs/transactions.rs`): per-item `ItemMeta` (custom name, potion contents, splash/lingering flag, food, block, hardness, enchantments, mining speed) and player `Vitals` (health, max health, selected slot, ground, effects, using item). `Click::Throw`/`ThrowStack` map to `Input::Throw` (button 0/1) and `Click::SwapHotbar(n)` maps to `Input::Swap`.
- `src/inventory/rules.rs`: InventoryManager rule model (matchers, categories, conditions, presets, slot and inventory filters, profiles). The JSON shape follows the reference's.
- `src/inventory/session.rs`: shared click session (open inventory, open wait, one gated click per snapshot, close wait, close). `src/inventory/arrange.rs`: shared armor and placement helpers.
- Planners: `inventory_manager.rs`, `inv_cleaner.rs`, `auto_hotbar.rs`, `armor_switch.rs` in `src/modules/utility/`. Each step re-plans from the snapshot.
- Runtime: `src/core/inventory_extra.rs` maps settings to planner options and steps to scheduler outcomes. The round-robin scheduler takes a module slice, so the four modules share the container claim, the stall watchdog and the gate with ChestSteal, AutoArmor, Refill and AutoTotem. On Key runs disable themselves when finished; ArmorSwitch always does. The reference "Activation" mode is stored as `<module>.trigger`, because `.activation` is the generic bind mode.

Validation: fmt, all-target Clippy `-D warnings`, 131 lib tests and the payload test pass; stage/audit (36 hooks, 22 modules) pass; host check reports 0 errors. No game scenario has run yet.
## 2026-10-31: inventory worker, use sequences, InventoryFill, Refill and AutoTotem extras

- **Throwpot, ThrowDebuff, AutoHeal: one use-sequence executor.** `src/modules/utility/throw_items.rs` holds the plans and the sequencer; `src/core/throw_runtime.rs` holds the claims and settings.
  - Throwpot (`inventory.throw-pot`): Type both/pots/soup, Mode dynamic/single, Delay, Scroll, Scroll delay, Random, Throw bowls.
  - ThrowDebuff (`inventory.throw-debuff`): Mode all/one-of-each/first, and Harming, Weakness, Poison and Slowness toggles.
  - AutoHeal (`inventory.auto-heal`, Hidden category):
    - Legit mode: select, use, swap delay, restore.
    - Bowl Mode stack/throw/none and Replace, as clicks through the shared session.
    - Silent mode and Potions are blocked: Horizon has no silent held-item or use API.
  - Host side: `vape_transactions::dispatch` turns the executor's effects into slot selection, `use_once` and drop.
- **InventoryFill** (`src/core/inventory_fill.rs`, `inventory.inventory-fill`): shift-clicks the hovered container slot while Shift and the left button are held, at the CPS setting.
- **Refill:**
  - Adds the double-click: a 20 % second shift-click, sent only when it can still move something.
  - A run now finishes, closing and disabling, once the inventory it worked in is closed. Before this fix it reopened the inventory. The host scenario found this.
- **AutoTotem** adds Random slot, Extra randomization (the reference's delay distribution) and the Show totem count value. Silent open and Silent move delay are blocked: Horizon has no API to click the player inventory without a screen.
- **Scenarios:**
  - `tools/throw_scenario.py`: Throwpot, ThrowDebuff, and AutoHeal on and then off.
  - `tools/inventory_scenario.py`: Refill interrupted and then completed, ChestSteal closed mid-run and then reopened, and AutoTotem Random slot.

Validation:
- fmt, all-target Clippy `-D warnings`, 140 lib tests, 3 integration tests and the payload test pass. The host build passes.
- `tools/throw_scenario.py` passed in survival in a fresh isolated home (evidence `docs/evidence/inventory-throw.json`):
  - Throwpot healed 10 → 20 HP with two of three Instant Health pots and restored slot 5.
  - ThrowDebuff one-of-each threw one poison and one weakness and kept the second poison.
  - AutoHeal Legit healed 6 → 15 HP using 2 of 4 pots; once disabled it threw nothing more at low health.
- `tools/inventory_scenario.py`: the first run failed on the Refill reopen bug above; the rerun is recorded in the next entry.

## 2026-10-31: inventory worker, recovery scenario through the bounded gate

- `tools/inventory_scenario.py` passed in survival in a fresh isolated home against the host build of 5204104 (evidence `docs/evidence/inventory-recovery.json`, scenario dir deleted after the run):
  - Refill (soup): closing its inventory mid-run disabled it with 1 of 3 hotbar soups moved and the screen back to playing; a second run completed (3 hotbar soups, 2 left in main) and closed the screen.
  - ChestSteal: escape after 3 of 6 diamonds left 3 in the chest; reopening took the remaining 3 and closed the chest.
  - AutoTotem (random slot): one of two main-inventory totems moved to the offhand and the screen returned to playing.
- AutoFish recovery has no scenario yet; it stays integrated.
- Validation on worker/inventory (main 604d679 already merged): `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` clean; `cargo test --workspace --locked --offline` 140 lib + 3 integration passed; `cargo run -p xtask --locked --offline -- payload-test` 140 passed.

## 2026-10-31: validation — merged main, verification

worker/validation was merged with main, which brings the inventory worker's
arrangement planners, use sequences and Refill extras. After the merge:

- `cargo fmt --check`, `clippy -D warnings` and `cargo test` pass
  (140 + 3 tests).
- `xtask payload-test`, `stage` and the host `check` against Horizon
  8f91f896a pass. The audit restores the original AST after removing 36 hooks
  and 22 support modules.
- The Refill run above was made before the merge. Main's Refill extras are
  covered by the inventory worker's own scenarios.

## 2026-10-31: combat worker, shared rotation, KillAura, SilentAura, KeepSprint

- `modules/combat/rotation.rs`: the shared rotation claim (priority owners,
  pending takeover on release) and the managed rotation. The managed rotation
  steps toward its target in 50 updates per tick with sensitivity-quantised
  deltas, then returns to the view before the claim is released.
- KillAura: swing/attack range, max angle (half on each side), max targets,
  Distance/Yaw/Armor/Threat/Health sort, APS, perfect swing, require mouse,
  GUI check, limited items, disable on death and use release/reuse around the
  attack.
- SilentAura: the PID rotation mode, Center/Closest target area, random aim,
  switch, shield check, extra swing and the clicker. Its clicker
  (SilentAuraClicker) ray-tests the managed rotation, not the view. AI modes
  stay excluded-approved.
- KeepSprint: runs at the `remote_player_attack` HEAD. It runs the host body
  once under a re-entry guard, then replaces the 0.6 cut and sprint stop with
  the configured slowdown and sprint. Includes buffer abuse and only-in-air.
- Hooks: `send-state-rotation` (MODIFY_ARG on the motion packet's rotation)
  and `keep-sprint` (HEAD, cancellable). New support modules: `vape_aura`
  (client tick) and `vape_combat_remote` (attack by id, miss, swing,
  KeepSprint). Stage/audit: 38 hooks, 24 modules.
- `tools/aura_scenario.py` (single player, NoAI 1000-health zombie): 16/16.
  KillAura hits off-crosshair inside its angle; it does not hit outside the
  angle, behind a screen, or after disable. SilentAura hits while the view
  stays unchanged, does not hit outside its angle, and stops when disabled.
- `tools/keep_sprint_scenario.py` (LAN host + guest, fresh homes): 13/13.
  Without KeepSprint, guest KillAura hits on the host show the ~0.78 speed
  dip. With KeepSprint at slowdown 1.0: 0 dips over 6 hits and sprinting
  throughout. After disable the dip returns.

Validation: fmt, Clippy -D warnings, 131 lib tests and the payload test pass.
Stage/audit (38 hooks, 24 modules) and the host check/build pass. Evidence:
docs/evidence/combat-aura.json and combat-keep-sprint.json.

Not ported or limited:
- The managed rotation does not cover the riding rotation packet.
- No third-person render of the managed rotation, and no movement fix.
- The mouse-over and block raytrace are not redirected; the wall check uses
  the target filter's visibility.
- SilentAura's Render type (Ring/Box) is not ported.
- A SilentAura miss in single player only swings.
- KeepSprint changes only hits on remote players, because that is where the
  host applies the cut.

## 2026-10-31: combat worker, merge of main and open combat items

Merged main (inventory planners, use sequences, recovery scenario) into
worker/combat. Conflicts were additive only (core module list, registry,
docs) and keep both sides.

Validation after the merge: fmt, Clippy -D warnings, 153 lib tests and the
payload test pass. Stage/audit pass (38 hooks, 24 support modules). Host
check: 0 errors.

HitSelect has been researched but not implemented. Notes for the next pass:
- The reference's two obfuscated tick constants are `(int)` casts of longs.
  They give COOLDOWN_TICKS = 8 (Right-click use cancel) and
  VELOCITY_RESET_TICKS = 7 (Normal).
- The attack-to-damage timing it needs is already tracked by
  `blatant::outbound` (expected hurt ticks, average hit delay).
- Cancelling the click locally can use the integrated `attack_clicks` count
  at the client-tick HEAD.
- On a server it needs a cancellable HEAD on `remote_start_attack`. The
  existing `remote-attack` HEAD is not cancellable, and
  `InteractionInput.clicks` is private.

Not started: HitSelect, BlockHit, HitFlick, AntiBot, HitSwap, AutoMace,
ShieldBreaker, CrystalAura, BowAimbot, AimAssist Adaptive, Reach Misplace,
and SilentAura's Render type.

## 2026-10-31: movement/world worker (Scaffold Legit, InvWalk, WindCharge, NoSlowdown sprint, Step)

- Scaffold Legit: the reference's edge sneak (`world_actions.rs` requests a Sneak
  override at the edge; the timing and whitelist follow `canActivate`).
  GodBridge and TellyBridge are not ported (they need the shared rotation
  owner). `modules/movement/rotation.rs` is a small isolated controller.
- InvWalk: the physically held movement keys (and Rotate's arrow keys) apply
  behind screens through the new `inv-walk-keys` HEAD on the controlled
  simulation tick. Limitations: the input packet's key byte and text-field
  check (see decisions.md).
- WindCharge: selects the charge, throws it at the feet with silent aim (UseItem
  rotation swap), jumps, restores the slot and disables itself.
- NoSlowdown: with Sprint on, `item_use_slows_sprint` is cleared too.
- Step: a recognised step resets Speed's stage and runs the 0.45 timer for
  100 ms. The reference's two intermediate positions are not sent: a 26.3
  server takes one position per client tick and disconnected the player
  ("Invalid move player packet received") in the first scenario run.
- LongJump vine/ice factors and NoFall's jump-key release: implemented,
  no scenario.
- xtask's audit peels stacked HEAD hooks in any order.
- `tools/world_movement_scenario.py` (fresh isolated creative home): 28/28
  checks: Scaffold Legit (baseline fall, edge sneak, stays on, runs with any
  item when the whitelist is off), InvWalk walk and Rotate, NoSlowdown full
  bow-walk speed (1.14 vs 5.60 blocks), Step over a one-block ledge without a
  kick (baseline blocked), WindCharge slot, launch to y 111.5, unchanged
  camera pitch, self-disable and restore. The sprint flag stays on while
  drawing even without NoSlowdown on this host, so the sprint re-enable is
  recorded, not proven.

Validation (after merging main): fmt, Clippy -D warnings, 153 lib tests,
payload test, stage/audit (37 hooks, 24 support modules), host check and
build pass; scenario 28/28 (docs/evidence/movement-world.json). Main's
tools/movement_scenario.py was not rerun on this branch.

## 2026-10-31: Text GUI module list (HUD worker)

- New HUD panel module `hud.text-gui` ("Text GUI", default off, anchored top-right) ports `TextGuiSettings` and the overlay. It lists enabled modules that have a reference `guiColor`. Sorting is by rendered length (name plus suffix, longest first) or alphabetical. Colour modes are Module color, Match GUI color (the GUI accent) and Custom color. Suffix modes are Basic, Extended and None. Further settings: shadow, row background, smooth/game font, watermark, custom text (`&` codes), hidden modules list, and slide/fade animation at the reference rate (±0.08 per 10 ms).
- Suffixes (`src/core/hud_extras.rs`) follow the reference value display formats:
  - Basic: NoFall, Speed and FakeLag mode; Blink's held-packet count.
  - Extended: Backtrack/AutoClicker/RightClicker/KnockbackDelay/Reach ranges, JumpReset chance, Velocity `h/v`, FakeLag `mode delay ms`, AimAssist and WTap values.
  - Extended falls back to Basic.
- Painted by `hud_painter.rs` with live text measurement. It anchors to the screen half it is in. A drop against the right edge stays right-anchored.
- Not ported: click-to-disable on rows, rounded row corners, the logo-image watermark (text `vape v4` instead), rainbow colours (no GUI colour/rainbow setting exists), the KnockbackDelay "Holding", FakeLag queued-red and AutoArmor "In Combat" states, and a list placed mid-right keeping its right edge when it shrinks.

Validation: fmt, Clippy -D warnings, 126 lib tests (3 new hud_extras, 5 text_gui), payload test, stage/audit (36 hooks, 22 support modules), host check and fast build pass. `tools/text_gui_scenario.py` (fresh homes, 7 restarts, 11 screenshots) passes; evidence in docs/evidence/hud-text-gui/. It covers: Basic/Extended suffixes, both sorts, three colour modes, plain font without background, left anchoring with watermark and custom text, hidden list, a bind removing a row, and the Text GUI's own bind.

## 2026-10-31: ClientSettings and notification toasts (HUD worker)

- `src/ui/client_settings.rs` defines the remaining ClientSettings and PublicProfileSettings values:
  - GUI bind indicator, Show tooltips and Search bar style (Floating/Integrated/None).
  - Notifications, with Toggle alert and Profile switch shown only while Notifications is on.
  - LWJGL-style bind text (`RSHIFT`, `LCONTROL + G`).
- `src/ui/notifications.rs` ports `NotificationManager`:
  - A bottom-right stack of at most 20 toasts. Each slides in from the right edge with the reference `interpolateToward` step.
  - The countdown starts when the toast is first drawn and shows as a progress bar. The toast slides out when it ends.
  - Boxes have the reference size and colours, plus an authored "i" badge.
  - Toggle alerts reuse one toast per module. Profile loads suppress them.
- `hud_painter::notifications` paints the stack after the HUD panels; it is a new call in the HUD adapter.
- Notifications raised:
  - Toggle alert (1.5 s): `Name` / `Enabled` or `Disabled`.
  - Profile swap (1.5 s): `Profile swap to name` / `N modules enabled`.
  - On first entering a world (5 s): `Loaded` / `Press <bind> to open the GUI`.
- Search bar styles:
  - Floating: a 110-wide frame at the top centre. Its magnifier focuses the field and turns into a clear button once there is a query; results are listed below it.
  - Integrated: the existing navigation-header search.
  - None: no search UI.
- Ctrl+F clears an active query or focuses the style's search field.
- Not ported or limited:
  - Show tooltips is stored but has no effect yet (there is no hover tooltip).
  - The GUI blur toggle is not ported: the host always blurs behind the menu.
  - Rainbow speed is not ported because there are no rainbow colours.
  - The legit-mode switch button, friend notifications and the online-account toasts are not ported.
  - Toasts are hidden while the click GUI is open, because the GUI pass replaces the HUD batch.
  - Toast messages are single-line; they cannot be clicked to dismiss.
  - The TAB bind view is superseded: every module row always shows its bind, as the v4 module cards do.

Validation:
- fmt, Clippy -D warnings, 132 lib tests (4 new), payload test, stage/audit (36 hooks, 22 support modules), host check (33 warnings, 0 errors) and fast build pass.
- `tools/client_settings_scenario.py` passes: fresh homes, 4 restarts, 12 screenshots. Evidence is in docs/evidence/hud-client-settings/. It covers:
  - the bind indicator;
  - the toggle alert on a real bind press, updated in place on the second press;
  - expiry;
  - the floating frame idle, with results for "sp", and cleared by Ctrl+F;
  - the integrated header search via Ctrl+F;
  - the None style ignoring Ctrl+F;
  - the bind indicator disabled;
  - the Notifications master switch suppressing toasts;
  - persistence across restarts.
- The profile-swap toast is unit-tested only.

## 2026-10-31: HUD modules: Target Info, Reach Display, Scoreboard, Inventory Blur; Keystrokes/Clock verification (HUD worker)

Added:
- Target Info (`hud.target-info`): name, health bar, hit and combo counters, damage and hits comparators, Show Hovered, a 3 s timeout, and a 1 s clear on death. Attacks are fed from the remote-attack and auto-tool attack adapters.
- Reach Display (`hud.reach-display`): the last landed hit's distance along the host pick ray, as "N.NN blocks", reset after 5 s.
- Scoreboard (`hud.scoreboard`, a movable HUD panel, Show score numbers): new HEAD hook `scoreboard-sidebar` on `scoreboard::Game::draw_scoreboard` cancels the vanilla sidebar. The panel paints the host's `hud_overlays::sidebar` draws from a cloned board with the numbers blanked.
- Inventory Blur (`render.inventory-blur`): new HEAD hook `inventory-blur` on `Game::menu_blur`, for the survival and creative inventories. The radius is the menu-blur option, or 5 when that option is off.
- Health: dropped at merge in favour of main's render Health (`src/modules/render/health.rs`, same id).

Not ported or limited:
- Freelook and Motion Blur are blocked on Horizon hooks (see decisions.md).
- Blockhit Animation and Hit Color are not applicable on 26.3.
- Panic default visibility needs per-module GUI visibility.
- Target Info:
  - No head preview.
  - Pots-used is always 0.
  - The damage comparator ignores enchantments.
  - The click GUI pass replaces the HUD batch, so HUD panels (and toasts) are not shown under the open GUI. The GUI preview captures show no panel.
- Scoreboard text replacements are not ported.
- Digital clock with date: with our font metrics, the date touches the minutes. The layout follows the reference coordinates.

Validation:
- Pre-merge, on the HUD branch: host check passed with 33 warnings and 0 errors, and the fast build passed. Five host scenarios passed in fresh isolated homes:
  - target_info 8/8
  - hud_readouts 7/7: Reach 1.73 and 2.54 blocks, then reset to 0.00. The Health colours were checked on the since-removed centre stack.
  - inventory_blur 3/3: blurred, sharp with the module off, creative blurred.
  - scoreboard 4/4: vanilla, moved with numbers hidden and shown, persistence.
  - hud_modes 5/5: keyboard/button/analog, arrow/icon/digital+date, no space bar with 24 h and no date, CPS-only, each idle and with W + left held.
- Evidence is in docs/evidence/hud-{target-info,readouts,inventory-blur,scoreboard,modes}/.
- Scenario fixes: survival worlds have no cheats, so commands now go through ctl `command` instead of chat. The 26.3 gamerule is `natural_health_regeneration`. Creative mode is switched with `gamemode`.
- After merging main: fmt, Clippy -D warnings, 194 + 3 tests and payload-test pass. Stage/audit restored 43 hooks and 29 support modules, and the host check passed with 68 warnings (all in other workers' merged code) and 0 errors. The fast build passes.
- Scenarios have not been rerun on the merged build.

## 2026-10-31: worker round merged into main

All six worker branches (inventory, validation, combat, movement, render,
HUD/UI) are merged into main at `4390bc3`; worktrees and branches removed.
Merge conflicts were append-only (registry entries, manifest blocks, hook
tables, dated doc sections) and kept both sides. HUD's duplicate Health was
dropped in favour of render's.

Validation on main `4390bc3`: fmt and all-target Clippy `-D warnings` clean;
194 lib + 3 integration tests and payload-test pass; stage/audit removes
exactly 43 hooks and 29 support modules; host check passes (verified by the
HUD worker on this exact tree; earlier merge commits `d33f53f`, the movement
merge and `3973987` were each host-checked on main). Ledger after the round:
64 validated, 32 integrated, 66 implemented-core, 6 blocked, 287 not-started.
Pending: render scenarios have never run; HUD scenarios ran on pre-merge
`ca264f4` and need a rerun on the merged tree.

## README screenshots (2026-10-31)

The README now shows docs/images/gui-all-modules.png (click GUI, all six
category frames open; World dragged off the floating search bar) and
docs/images/gui-hud-editor.png (HUD editor, all 11 HUD modules). Both were
captured from a fresh fast build of `545ca54` in a fresh isolated-home
session (`target/scenarios/readme-gui-vvbo88if`, 1920x1080 offscreen, GUI
scale 2, creative). They show layout only; no module is enabled and no
parity claim follows from them.

## 2026-10-31: upstream drift check, capability audit, compatibility matrix

Phase 8 release tooling, done on main while the round-2 workers run.

- `xtask drift <horizon> [revision] [--compile]` is a reviewable upstream-update check. It exports the candidate into target/drift (the pinned export and compatibility.json are untouched) and lists the hashed files that changed. It compares game version and protocol, then plans, applies and audits a scratch copy of the bundle that names the candidate. `--compile` fetches the candidate's locked dependencies and type-checks the client. It writes target/drift/report.json and fails whenever anything differs. `stage`, `export` and `audit` now share this code.
- `.github/workflows/drift.yml` runs the check with `--compile` on Horizon main every day and uploads the report. It has read-only permissions and never publishes or moves a pin.
- `pack` audits the shipped Rust (the runtime payload and every adapter and support module) at the token level, so macro bodies are covered too. It rejects `unsafe`, sockets (`std::net`, TCP/UDP/Unix sockets), `process::Command`, `extern "ABI"` blocks, `#[link]`, `std::ffi`, `include!`/`include_bytes!`/`include_str!`, `asm!`, `no_mangle`, `export_name` and `link_section`. The current tree passes. A unit test covers each rejection and the allowed look-alikes (an enum named `Command`, `include` and `link` used as names, `std::process::id`).
- compatibility-matrix.json lists every published mod version with its Horizon, mixin, game and protocol pins and its platforms. `bundle.py` refuses to build a stable version whose pins differ from its matrix entry, so a new pin pair needs a new version. It also writes `<artifact>.compatibility.json` (artifact SHA-256, pins, toolchains, platforms), which the release workflow uploads. The development entry lists no validated platforms. Linux x86-64 is the only platform exercised; Windows and macOS are listed as unvalidated.
- docs/development.md documents the update, rollback and settings-preservation process.

Drift result for Horizon main `5062f225` (362 commits after the pin; evidence docs/evidence/drift-5062f225.json):
- Minecraft 26.3 and protocol 777 are unchanged, and serde_json 1.0.151 is still provided.
- Six hashed files changed: `game.rs`, `game/control.rs`, `game/lan_particles.rs`, `game/remote.rs`, `game/sculk.rs` and `motion.rs`.
- All 43 hooks and 29 support modules plan, apply and audit cleanly, and `cargo check` of the client passes with 0 errors.
- A re-pin still needs a source review of the changed files and the scenarios. Upstream also turned its anticheat on by default, with a combat check that acts, so the blatant-module scenarios need checking against it.
- None of the client features our `blocked` rows wait for appear in the new commits: chat send, camera rotation, bone poses, mesher and entity-render hooks.

Gates on this tree: fmt; Clippy `-D warnings` (all targets); 194 lib + 4 xtask tests; payload-test; `xtask check` on the pin (stage/audit removed exactly 43 hooks and 29 support modules; 68 warnings, 0 errors); `bundle.py` (74 source files).

The drift report now also lists `changed_targets`: hooks whose function or call-site fingerprint differs from the pinned plan. For `5062f225` that is seven HEAD hooks: client-tick, gui-events, inbound-packet, session-connect and session-disconnect, plus no-slowdown and inv-walk-keys on `controlled_simulation_tick_with_sprint_window`. I reviewed each upstream diff.
- The client-tick, gui-events and session/packet hooks only gained body changes after their heads: the native-mod net channel, the chat tree, riding resets and candle-cake snuffing.
- The controlled tick is now a wrapper around a shared `local_tick`, which the server's new movement replay also uses. Our HEAD hooks stay on the client-only wrapper, so their semantics are unchanged.
- The re-pin is therefore a mechanical hash update plus scenario reruns.
- Upstream added an anticheat that drops attacks from a client tick with more attacks than swings, or hits on a second entity. It is on by default only on dedicated servers (`game/dedicated.rs` is the only caller of `Hub::set_anticheat`). A LAN world's hub holds none of it (`lan/anticheat.rs`), so our two-process LAN scenarios keep vanilla server behavior. A future dedicated-server scenario has to set `[anticheat]` in its scratch `server.toml`.

## 2026-10-31: online-removal audit (Phase 7)

The pack-time capability audit (tools/xtask/src/main.rs `capabilities`) now also rejects:
- the host's HTTP client crate `ureq`;
- `server_packs::download`;
- the `horizon_telemetry` and `horizon_friends` crates;
- string literals that contain `://`. Doc comments may still cite a URL.

The pinned client depends on `ureq` and uses it for server-pack downloads, so the old rule against sockets in the payload alone left a gap.

Removal audit on this tree:
- `git ls-files` lists no Java, Kotlin, C or C++ source, no JAR or class files, no Gradle build and no native libraries or executables. Tracked directories: src, mixins, tools, docs, .github and root metadata.
- `src/` and `mixins/` contain no URL, account, login, licence or HWID code. A hit like `server_packs` only reads `prompt` to keep modules idle while the host's pack prompt is open.
- The ledger carries all nine online-product rows (accounts, licensing, cloud and public profiles, reviews, parties, shared activity, shared inventories, remote sync) and the four AI rows as `excluded-approved`.
- Local profiles and the friend/enemy records stay local files.

Gates: fmt; Clippy `-D warnings` (xtask); 4 xtask tests, including the new denied cases (`ureq::get`, an `https://` constant, a telemetry push) and the allowed ones (a doc comment with a URL, `game.server_packs.prompt`); `xtask pack` on the real payload passes the stricter audit.

## 2026-10-31: per-module frame cost (tools/perf_scenario.py)

New `tools/perf_scenario.py` sets up the delays-perf scene: 24 cows, 8 chests, an item and a held snowball. It takes a 10 s host bench with nothing on, each of the twelve render/HUD modules alone, all of them, and nothing again. Checks: each module alone adds under 1 ms of average CPU per frame and all of them under 3 ms; every bench runs at the same frame cap; nothing costs anything once off.

The earlier perf record blamed the 118 → 30 fps drop on "compositor pacing". That was wrong. The host keeps vanilla's AFK limiter (`FramerateLimitTracker`: 30 fps a minute after the last player input), and ctl key presses don't reset it.
- Run 1 (`perf-q07fj7d4`, limiter on, 61/66): the frame rate fell during the hud.armor and hud.potions benches, then stayed at 30 fps for every later bench, including all modules off.
- At that cap, all twelve modules cost 1.92 ms avg / 3.47 ms p99 CPU against a baseline of 0.98–1.52 ms. The large per-module averages and p99s (hud.potions 6.6 / 30 ms, ESP 5.0 / 25 ms) came from the benches where the cap kicked in, not from the modules.

Both perf scenarios now seed the scratch `options.txt` with `inactivityFpsLimit:"minimized"`, as the host's own benchmark mode does.
- Run 2 (`perf-7q1ii6a8`, 63/67): 94–117 fps in every bench.
- The machine was at load 25 on 12 cores while the seven workers compiled. The two baseline benches differed by 1.2 ms, and all modules together (4.66 ms) cost less than ItemESP alone (5.47 ms), so the per-module failures (hud.coordinates, NameTags, ItemESP, Trajectories) are not attributable.
- The game also took over 15 s to quit under that load (harness timeout after the results were written).

Open: rerun `perf_scenario.py` on an idle machine after the round-2 merges. Performance stays unvalidated until then.

## 2026-10-31: settings-parity audit (tools/settings_parity.py)

`tests/settings_dump.rs` writes every setting definition (id, label, kind, default) to `$VAPE_RS_SETTINGS_DUMP`; it does nothing without that variable. `tools/settings_parity.py` parses the `*Value.create(...)` / `new *Value(...)` declarations from the ledger's `settings_and_defaults` text. It matches rows to our modules by display name through `src/core/registry.rs`, then compares labels, defaults and number ranges. 645 definitions were checked. Field-reference defaults (`this.analogMode`) and labels built at runtime are not compared.

The audit found reference settings missing from seven modules that were marked `validated`. Those rows are now `integrated` (validated 64 → 57), each with a decision note listing what is missing:
- KillAura: Show target, Attack Color.
- SilentAura: Show target, Attack color, Render type, Break blocks, Break blocks whitelist, Block breaking items.
- RightClicker: Hold to Click.
- Text GUI: Click disable, Rescale, and the Scale range (0.1–2.0 step 0.1; ours 0.5–4.0 step 0.05).
- ChestSteal: Check in menu, which defaults to false; ours defaults to true. Also best-only queueing of non-gear items with the player-inventory scan order, the grid-distance shuffle, and the slot travel delay.
- Blink: Spawn fake.
- Scoreboard: Replace scoreboard text.

Integrated rows with gaps:
- ESP: Enemy Only, Enemies List Only.
- AutoHeal: Inventory Only.
- AutoTotem: Silent open.
- SpawnerFinder: "Spawners". Ours is labelled "Spanner names", probably the same list under another name; to check.

Reach's Misplace and Disadvantage stay with the blocked Reach row. SilentAura's AI settings stay excluded.

Routed to the running round-2 workers:
- combat-a: KillAura and SilentAura.
- render: ESP.
- inventory: ChestSteal, AutoHeal, AutoTotem.
- hud-client: Text GUI and Scoreboard.
- movement-b: Blink's fake player (already in its brief).

Unassigned, for round 3: RightClicker Hold to Click, and the SpawnerFinder label.

Tests: `cargo fmt --check` and `cargo clippy --all-targets -D warnings` are clean; the dump test passes (1/1).

## 2026-10-31: settings-parity audit reads the reference sources

`tools/settings_parity.py` now parses declarations straight from each row's reference sources, read as text and never run; the ledger's `settings_and_defaults` text is the fallback. It counts a value as shown only when the reference passes its field to `addValue(...)` or `U(...)`. Values that are created but never added are hidden and run at their default; these are reported as "hidden in reference" rather than as gaps. It also:
- resolves ModeOption field defaults (`this.threeDimensionalMode`) to their labels ("3D");
- reads the default from the right argument when `ModeValue.create` has a description;
- skips `((Object)this)` owners;
- compares RandomValue ranges as well as NumberValue ranges;
- maps ClientSettings ("GUI") to our `client.*` settings.

Corrections to the previous entry:
- RightClicker "Hold to Click", ESP "Enemy Only" / "Enemies List Only" and AutoHeal "Inventory Only" are hidden in the reference, so they are not gaps. Our RightClicker always holds, which matches the hidden default (src/modules/combat/clicker.rs). module.rightclicker is back to `validated` with its original evidence.
- ESP's mode default matches ("3D"); the earlier "default differs" was a parse artefact.

New gaps found:
- ClientSettings: "Blur background" (default true) and "Rainbow speed" (0.1–10, default 1). Every reference ColorValue has a rainbow toggle; our color kind is a single ARGB value. module.clientsettings is lowered to `integrated`.
- SilentAura: the non-AI "Kiss" rotation mode ("Kiss speed" 5–90 default 30, "Kiss jitter" 0–1 default 0.15) and "Break blocks delay" (0–2000 ms, default 0–10). Also "Target color", next to the earlier Show target / Attack color.
- KillAura: "Target Color".
- AutoTotem: "Silent move delay", next to "Silent open".
- AutoArmor: Delay. The reference allows 1–200 ms with default 100–120; ours allows 100–200 with default 120.
- SpawnerFinder: the label is "Spawners". "Spawner names-whitelist" is only the storage id.

Routed:
- combat-a: Kiss mode, Break blocks delay and the target colors.
- inventory: AutoTotem.
- render: the SpawnerFinder label.
- hud-client: Blur background and Rainbow speed.
- Round 3 (target/worker-briefs/round3-plan.md): AutoArmor Delay.

Every other number and random range the audit can parse matches the reference.

Current report: 26 missing settings on integrated rows, all routed (3 are SilentAura's excluded AI settings). Plus 2 missing on the blocked Reach row, 2 range differences (AutoArmor Delay, Text GUI Scale), 1 default difference (AutoArmor Delay) and 9 values hidden in the reference. The validated count is 57 and integrated is 39.

Tests: `settings_dump` passes (1/1, 645 definitions).

## 2026-10-31: main's baseline scenario failures fixed

Seven host scenarios failed on main at 4ff7488, before any round-2 merge. Four were scenario bugs, one was a real Refill deviation, and two were environment problems:
- **delays_perf (NoJumpDelay):** the take-off windows were wall-clock, so a slow sample under load shrank them. The scenario now counts host ticks (`state.ticks`). A bonk under the ceiling lands in about six ticks, so the module turns the vanilla ten-tick spacing into a six-tick one: on needs 1.5× the take-offs and a median gap of 7 ticks or less, and off and after need a gap of at least 9. **NoClickDelay:** the miss-then-hit check released the button between the miss and the hit, which let the next tick clear the vanilla cooldown, so it held with the module off too. It now keeps the button held through the look.
- **inventory (Refill):** after the player closed the inventory before the next delayed click, Refill reopened it. The reference's `onTick` observes the screen every pre-tick and only delays the clicks, so a run whose inventory was shown and then closed finishes and disables. `Refill::observe` (src/modules/utility/refill.rs) now runs every tick from the inventory runtime, and the unit test `closing_before_the_next_step_still_finishes` covers it.
- **gui, auto_tool, no_hurt_camera:** fixed click positions went stale when modules were added or moved (NoHurtCam is in Render since 1336a94). The new `tools/gui_layout.py` computes category and module rows from the registry and `CATEGORIES`, and skips modules hidden by default once `src/core/visibility.rs` exists. The gui scenario's multi-bind click moved to y=95, below the newer "GUI bind indicator".
- **auto_tool_guards:** the nether landing used random terrain, so the block under the crosshair varied. The scenario now force-loads and clears the area and lays an obsidian floor first.
- **text_editor:** it now runs under `xvfb-run` with a private display, like the other clipboard checks.

Tests: fmt, Clippy and the workspace tests are clean (395 passed, 0 failed), and payload-test, stage, host check and host build pass. Host scenarios, each in a fresh isolated home: delays_perf 39/39, inventory, gui, auto_tool, auto_tool_guards, no_hurt_camera and text_editor pass (target/scenario-runs/20261031-181944, -182256 and -182518).

## 2026-10-31: HUD and client foundation (worker round 2)

Branch `t3/363c8469`, built from `9fd0493` against the Horizon pin `8f91f89`.

Added:
- Per-module visibility (`Mod.visible`, new `src/core/visibility.rs`):
  - Speed, Fly, Timer, InvCleaner, ThrowDebuff, AntiDebuff, InvWalk and AutoHotbar are hidden by default. Panic stays visible because of its F10 bind (decisions.md).
  - A frame header's edit button lists every module of the category behind a checkbox strip. `modules.<id>.visible` is saved only when it differs from the default.
  - A hidden module refuses its bind with a "Hidden Module" warning toast. Hidden enabled modules are switched off after a profile load.
  - Bind-only modules (the `UtilityMod` subclasses) refuse GUI clicks, and the row shows "MUST BE BOUND" or "USE VIA BIND".
- ClientSettings "Blur background" (default on): new cancellable HEAD hook `gui-blur` on `Game::menu_blur` (`mixins/vape_rs/adapters/gui_blur.rs`). It reuses the host menu blur, the same path as Inventory Blur, while the click GUI or the HUD editor is open.
- ClientSettings "Rainbow speed" (0.1..10, default 1, step 0.1) and the per-colour rainbow (new `src/core/rainbow.rs`):
  - Every colour setting has a hidden `<id>.rainbow` toggle, shown as a fifth Rainbow row in the colour popup.
  - While the toggle is on, the hue advances by Rainbow speed every 50 ms, using Java's HSB conversion. The cycled colour is written back without dirtying the profile.
- TextGuiSettings: Scale is now 0.1..2.0 in tenths, Rescale (default on) is stored, and the "None" suffix mode has a capture.
- A cleared GUI bind falls back to Right Shift (`ClientSettingsGuiBindBendable.setBoundInputs`).
- Scenario updates for the new defaults: movement, world_movement, legit_render and throw seed `visible: true` for the default-hidden modules they drive. gui_scenario.py has the new Fullbright row (AntiDebuff hidden) and the shifted multi-bind row (Blur background added).

Audits (no code):
- client.timing: `src/core/rng.rs` (xorshift), the Normal and Extra modes in `src/modules/combat/click_timing.rs`, and ExtraPlus in the clicker. The host measured 11 CPS at a 10–12 setting. A per-mode host distribution check is still open.
- client.localization: the reference ships English, Chinese, French, Portuguese and Spanish `.properties` tables, `FontOption.s()` translation, `NotoFontOption` and an `exportFramesConfig` template. None is ported. It stays not-started.
- client.performance: the reference has debug classes only, so it needs a scope decision and stays not-started.
- client.target-info: see the Target Info gaps below. It stays integrated.

Tests:
- fmt --check and Clippy -D warnings are clean. Workspace tests pass 204 + 3, and payload-test passes 204. New tests cover visibility (4), rainbow (3), the popup Rainbow switch, and the GUI bind fallback.
- Stage/audit: 44 hooks and 29 support modules, with the original AST restored. The fast host build passed with 68 warnings and 0 errors.
- Host scenarios, each in a fresh isolated home:
  - gui_blur_rainbow 6/6. The rainbow Text GUI colour cycles and is not saved by itself, and the popup switch stores and clears `<id>.rainbow`. With Blur on, the GUI capture fits a radius-4 blurred play capture 4× better than the sharp one. With Blur off, it fits the sharp one 11× better. Both comparisons first apply a linear fit, which absorbs the GUI's dim.
  - module_visibility 7/7
  - text_gui 11/11
  - client_settings 11/11
  - gui 11/11
  - hud_modes 5/5
  - target_info 8/8
  - movement 52/52
  - world_movement 28/28
  - legit_render 40/40
  - throw 4/4
- An earlier gui_blur_rainbow check compared absolute edge energy across two launches. That was unsound, because each launch shows a different scene and the GUI dims the world, so the check now compares within a session.
- main's `tools/settings_parity.py` (from `4ff7488`, reading the reference as text) was run on this tree:
  - ClientSettings has no missing, default or range difference.
  - TextGuiSettings misses only Click disable.
  - Scoreboard misses only "Replace scoreboard text".
- Evidence: docs/evidence/hud-client-r2-{blur-rainbow,visibility,text-gui,client-settings,gui,modes,target-info,settings-parity}/.

Ledger update: target/ledger-updates/hud-client.json (41 rows).

Open:
- Text GUI: Click disable (needs the HUD under the open GUI), rounded corners, the logo watermark (no redistributable asset), the per-module states, and right-edge anchoring while the list shrinks. "Match GUI color" uses the fixed accent.
- The colour popup edits ARGB channels instead of the reference's HSB picker.
- Not done yet:
  - the HUD and toasts under the click GUI;
  - hover tooltips;
  - the legit-mode button;
  - friend notifications;
  - Windows date conventions for Clock;
  - Scoreboard "Replace scoreboard text".
- Target Info: head preview, pot count, enchant-aware damage comparator.
- Freelook modes stay blocked (no camera-rotation hook).
- module.blockhitmode belongs to combat-a and was not touched.

## 2026-10-31: hud-client (round 2) merged

I merged t3/363c8469 (f26e51d) into main on top of the baseline fixes. Conflicts:
- `tools/gui_scenario.py`: Fullbright's row stays computed by `gui_layout.py`, which now leaves out default-hidden modules, and the multi-bind click is at y=113, below Blur background.
- docs: both sides kept.

I applied target/ledger-updates/hud-client.json (41 rows: 24 validated, 4 integrated, 6 blocked on the Freelook camera-rotation hook, 5 not-applicable-reviewed, 2 not-started). The evidence is in docs/evidence/hud-client-r2-*.

Tests on the merged tree: fmt, Clippy and the workspace tests are clean (415 passed, 0 failed), and payload-test, stage, host check and host build pass. Host scenarios, each in a fresh isolated home, all pass (target/scenario-runs/20261031-184300):

| Scenario | Result |
|---|---|
| gui_blur_rainbow | pass |
| module_visibility | pass |
| text_gui | pass |
| client_settings | pass |
| gui | 11/11 |
| hud_modes | 5/5 |
| target_info | 8/8 |
| throw | pass |
| legit_render | 40/40 |
| auto_tool | pass |
| no_hurt_camera | pass |
| movement | 52/52 |
| world_movement | 28/28 |

## 2026-10-31: clicker and movement debt (worker round 3)

Worker clicker-movement-debt, branch t3/63456628 from main 4ff7488. This round closes the scenario debt on LeftClicker modes, TriggerBot, Sprint, Velocity and SafeWalk, with LAN runs on two owned local processes. It also ports Sprint's reference latch.

Changed files:
- src/modules/combat/sprint.rs (new): the reference `Sprint.onPreEntityUpdate` latch, with 4 unit tests. The press is made only while the key is up, the player is not sprinting and not collided horizontally. It stays down until disable, a screen, or the user's own key release. Cancel Invis is gated on `allowFlying` (may fly). Cancel Invis and the movement-input claim only skip new presses.
- src/core/mod.rs: the `sprint` state, its reset on disable, and `Runtime::sprint_press` (append-only). src/modules/combat/mod.rs: `pub mod sprint;`.
- mixins/vape_rs/input_delays.rs: `resolve_movement` builds the Sprint tick from host state (sprinting, horizontal collision, invisibility, may fly). Before, it pressed every active tick.
- mixins/vape_rs/combat.rs: the LAN host's snapshot of each guest takes `blocking` from the server's model of the guest, so Shield check works against guests. It used to be always false.
- src/modules/combat/click_timing.rs: Normal stores its 1000/CPS delay as the last delay (reference).
- New scenarios:
  - tools/clicker_modes_scenario.py
  - tools/trigger_bot_options_scenario.py
  - tools/trigger_bot_lan_scenario.py
  - tools/velocity_lan_scenario.py
  - tools/sprint_safe_walk_lan_scenario.py

Scenarios (fresh isolated homes through native_session.py, HORIZON_SERVER_THREAD=1). Every run used one build: bundle 792ed691e319, built after the last source edit from 4ff7488 plus these changes, Horizon 8f91f89, mixins 0e9229d, fast profile.

- **clicker_modes: 18/18.** Integrated server, 10 CPS on a NoAI, Invulnerable cow for 30 s each, server TPS 20.
  - Normal: 9.79 CPS, gaps {1:2, 2:284, 3:15} ticks.
  - Extra: 9.45 CPS, 31 long gaps against Normal's 15.
  - Extra+: 10.73 CPS, with 112 under-interval gaps.
- **trigger_bot_options: 19/19.**
  - Defaults hit at full strength with no air attack.
  - Miss chance 100 gives one air attack per click. Early chance 100 hits at scale 0.92.
  - Limit to items (swords): no hit with dirt.
  - Select first hit: 0 hits while waiting, 4 after being hurt, 0 after a second looking away.
- **trigger_bot_lan: 34/34.** Host world opened to LAN plus a guest client; both players have shields.
  - Host TriggerBot: 12 hits on the open guest. While the server sees the guest's shield in use: 0 hits and 0 swings, and a physical click is fully blocked. 12 hits once the shield is lowered.
  - Guest TriggerBot, Shield check off: 12 hits on the raised shield, each blocked.
  - Guest TriggerBot, Shield check on: 0 hits and 0 swings behind the shield, 12 once lowered.
  - Select first hit: no hit until the host's click, then 8 hits back, none after looking away.
- **velocity_lan: 26/26.** Guest Velocity against the remote server's SetEntityMotion and the Explode knockback.
  - Off: pushes of 0.99 and 2.38 blocks. On: both cancelled.
  - Only when targeting: follows the reference, including the `applyBridgeVelocity` explosion quirk.
  - Kite 200%: 1.99 and 4.77 blocks. Always Kite: doubled while facing.
- **sprint_safe_walk_lan: 42/42.**
  - Sprint on host and guest: 5.51 b/s against a 4.32 b/s walk. A pause menu releases the key, and it is pressed again after resuming. Disable releases it. Cancel Invis blocks sprinting in invisible survival but allows it in invisible creative.
  - Guest SafeWalk: held at both edges without sneaking, Direction Check lets a forward walk off, and after a rejoin with it off a forward walk is held.

Feature ids (ledger update target/ledger-updates/clicker-movement-debt.json; evidence docs/evidence/clicker-movement-debt-r3-*):
- module.leftclicker.mode.normal and .extra: implemented-core → validated.
- module.triggerbot, module.sprint, module.velocity, module.safewalk: integrated → validated.
- module.backtrackpacketstate, module.backtrackpacketreplaystate, module.backtrackpacketqueueentry: not-started → validated. These helpers follow module.backtrack (docs/evidence/validation-hold.json); see decisions.md.
- module.autoclickerentitypositionstate: stays not-started. Only Reach's Misplace uses it, and Misplace is unported (Reach is blocked and in combat-b's scope).

Gates:
- `cargo fmt --all --check`: clean.
- Clippy `-D warnings` (workspace, all targets): clean.
- `cargo test --workspace`: 198 lib, 1 settings_dump, 4 xtask and 0 doc tests, all passing.
- `xtask payload-test`: 198/198.
- `xtask stage`: the audit restored the original AST after removing exactly 43 hooks and 29 support modules.
- `xtask check` on the pin, under the shared lock: the same 43 hooks and 29 support modules, 68 warnings, 0 errors.
- `xtask build` (fast): 68 warnings, 0 errors, bundle 792ed691e319. That is the bundle every scenario above ran on.
- `settings_parity.py`: no entries for these rows.
- No new hooks.

Failures on the way, all fixed in the scenarios:
- Velocity needs `mob_attack` damage, because `generic` gives no knockback. The players must stand off-axis because of the reference's yaw-to quirk.
- The SUN_LEN socket path limit needs short session prefixes.
- `looking_at` lists mobs only.
- The host's hurt-log entry carries its profile name, not "host".

Limits and next steps:
- TriggerBot's ShieldBreaker, HitSwap and AutoMace bypasses need those combat-b modules.
- Sprint's movement-input lock needs Scaffold Blatant/Telly.
- AutoClickerEntityPositionState follows Reach Misplace.

## 2026-10-31: clicker-movement-debt (round 3) merged

Merged t3/63456628 (head c28cff8) into main on top of hud-client's round 2 (608c800). The progress.md and decisions.md conflicts were append-only; both sides are kept.

Gates on the merged tree (`target/agent-tools/gates.sh --host`):
- `cargo fmt --check`: clean.
- Clippy `-D warnings`: clean.
- 423 tests passed, 0 failed.
- `xtask payload-test`: passed.
- `xtask stage`: passed.
- `xtask build` on the pin: passed.

Scenarios on the merged build, each in a fresh isolated home:
- velocity_lan: 26/26.
- combat: 39/39.
- gui: passed.
- clicker_modes: 18/18 (run 20261031-190725).
- trigger_bot_options: 19/19 (run 20261031-190725).
- trigger_bot_lan: 34/34 (run 20261031-190725).
- sprint_safe_walk_lan: 42/42 (run 20261031-190725).

The first merged run (20261031-185834) was made at a load average of about 15, with other workers' builds running. It failed four scenarios:
- **clicker_modes:** the control socket timed out at launch. It passed in the quiet rerun.
- **trigger_bot_options:** the launch timed out. It passed in the quiet rerun.
- **sprint_safe_walk_lan:** the host died before its first walk.
  - Cause: the scenario teleported the survival host to y=89 before the catch floor at y=88 was built. A slow world load let the host fall onto terrain without Resistance.
  - Fix: it now builds the floor, then teleports.
- **trigger_bot_lan:** 33/34. "A physical click is blocked by the guest's shield" saw no hurt.
  - Cause: the click was made before `hurts()` cleared the server's hurt log, so a quick server logged the blocked hit and the reset wiped it.
  - Fix: the click now runs inside the window, as the later Select-first-hit check already did.

Both fixes change only scenario timing; the checks themselves are unchanged.

Ledger: target/ledger-updates/clicker-movement-debt.json is applied. Every check in the five evidence results passed (18, 42, 34, 19 and 26). The ledger now has:
- validated: 90
- implemented-core: 64
- integrated: 37
- not-started: 246
- excluded-approved: 13
- blocked: 12
- not-applicable-reviewed: 10

## 2026-10-31: Panic validated (tools/panic_scenario.py)

New scenario tools/panic_scenario.py: a fresh isolated home on main 8987b8d (bundle c8e2ef58a77d, the merged build). It passed 12/12; evidence is in docs/evidence/coordinator-panic.

Fullbright, Sprint and NoHurtCam start enabled, each with its own bind.
- **Re-enable off (default):** the default F10 bind disables all three, and a second F10 restores nothing. Panic itself never stays enabled.
- **After a relaunch with Re-enable on:** F10 disables the set, and a second F10 restores exactly that set.
- **The reference `U(mod)` reset:** after a Panic, enabling Fullbright through its bind forgets the saved set. The next F10 disables Fullbright, and the one after it restores only Fullbright.

module.panic: integrated → validated (target/ledger-updates/coordinator-panic.json). The visible-by-default, F10-bound deviation stays as recorded in decisions.md.

## 2026-10-31: Workers resumed after an API 403; Freelook unblocked; re-pin anticheat note

All ten round-3/4 workers stopped at about 19:55 on a transient "API Error: 403 Not authorized". Their worktrees kept their commits and uncommitted edits, and each was resumed in its own thread with that state. The merged clicker-movement-debt and hud-client worktrees were removed: 0 commits ahead of main, clean.

Freelook (module.freelookhudmodule and six modes) was `blocked` as "needs a Horizon camera-rotation hook". That was wrong. movement-b's Freecam adds cancellable HEAD hooks on `perspective::Game::player_camera_entity` (a replacement camera entity) and `turn_player` (mouse turns), which is that capability. Freelook moves to movement-b and shares those hooks through one dispatcher each. The reference never passes Freelook's `perspectiveMode` to `addValue`, so Perspective is not a user setting and stays at 3rd Person.

The scope-coverage check (target/agent-tools/coverage.py over target/worker-briefs/round2-scope.txt, with the round-3/4 scopes added) leaves no open row unowned. The one exception is the helper `abstractblockinmovementcontroller`, whose status follows its owners.

Re-pin to `5062f225`: upstream's acting combat check (fdfa4fbfa: more attacks than swings in a client tick, or two targets in one tick) runs only on a dedicated server's hub. `dedicated.rs:873` is the only `set_anticheat` call outside tests, and `lan/anticheat.rs` says the hub holds none of it for a LAN world. The integrated-server and LAN scenarios therefore run without it. A dedicated-server run would see blatant modules flagged as designed, which is not a regression.

## 2026-10-31: HUD modules (worker round 3)

Worker hud-modules, branch t3/fdf14b2d from main 4ff7488, Horizon pin 8f91f896a (mixins 0e9229d). Armor Status, Potion Status, Coords, FPS and Compass now draw the reference HUD frames. NoHurtCam and both Weather modes are proven from host frames.

Changed files:
- `src/ui/hud.rs`: the reference layouts.
  - ArmorStatusHudFrame / ArmorStatusItemComponent: 20-tall detailed rows, and 22-tall compact rows 2 apart. The `getDurabilityPercent` edges are none / 1 / 100. Durability is coloured red → yellow → theme green. Low and no-durability rows get a warning fill; a broken piece shows the shield and broken bar for 4 s.
  - PotionEffectsHudFrame: 30-tall rows. A countdown ring empties anticlockwise from the top, going green, then orange (half left), then red (a quarter). Names and MM:SS durations sit at the reference positions.
  - CoordinatesHudFrame: Math.round positions and truncated text x. Has the `drawDirectionArrow` X/Z arrows, and the frame ends 12 past the Z arrow.
  - CompassStripComponent: 1400 tenth-degree steps from 70° left of the heading (wrapped yaw + 180), with a tick every fifth degree. Big and small points get an unbackgrounded shadow copy. `compassarrow` is drawn at half scale, tinted by the foreground.
  - Uses the reference theme colours. The foreground colour only tints what the reference draws in the editor foreground.
  - New tests: `armor_percent_follows_the_reference_edges`, `armor_rows_draw_detailed_and_compact_reference_layouts`, `potion_rows_count_down_with_reference_colours`, `compass_ticks_follow_the_tenth_degree_strip`.
- `src/ui/hud_state.rs`: a piece with no durability counts as low, as the reference's -1 does, so removing it also shows it broken.
- `src/core/registry.rs`: display name "Potion Status". `src/core/mod.rs`: the armor frame height uses ArmorStatusHudFrame's margin (4, or 2 when compact).
- `tools/hud_detail_scenario.py`: new `--part layouts` (the default `all` still runs the old details part first).
  - Live armor and effects on the player, captured at GUI scale 2 and 1 in two seeded setting sets: the defaults, then compact armor, background off, vertical coordinates, negative effects only and a cyan foreground.
  - Then the effects counting down, legs removed (broken, then gone), and every module off and back on through its bind.
- `tools/no_hurt_camera_scenario.py`: a real `damage` hit while facing a brick wall, frames over the hurt tilt compared with the still frame before the hit, for on / off / on again.
  - Command feedback is off, because the feedback lines scrolled the chat and polluted the diff.
  - The GUI toggle finds the Render frame and the NoHurtCam row through main's `tools/gui_layout.py` (resolved in the merge). The old fixed clicks toggled Reach Display after NoHurtCam moved frames in 1336a94.
- `tools/weather_modes_scenario.py` (new): Raining under a clear server sky and Clear under a raining one (seeded through a restart). Each captures off / on / off frames, plus a frame of the server weather the mode should look like.
- `tools/settings_parity.py`: `VAPE_RS_REFERENCE` names the reference from a worktree.
- `docs/decisions.md`: infinite effects, geometry icons, the Vape GUI scale factor, and foreground tinting.

Feature ids (ledger update target/ledger-updates/hud-modules.json, all `validated`):
- module.armorstatushudmodule
- module.potioneffectshudmodule
- module.coordinateshudmodule, plus .mode.horizontal and .mode.vertical
- module.fpsdisplayhudmodule
- module.compasshudmodule
- module.nohurtcamerahudmodule
- module.weatherchangerhudmodule.mode.clear and .mode.raining

The validated parent module.weatherchangerhudmodule gains the mode evidence and hook `client-frame`. module.hudmodule stays with hud-client (not-applicable-reviewed).

Scenarios, each in a fresh isolated home. Evidence is in docs/evidence/hud-modules-*. The final runs are on head 74a9aa4 (main 7e48db7 merged); the pre-merge runs are on base 4ff7488. The same three scenarios also passed after the first merge (608c800): `hud-layouts-8rz4urv1` 8/8, `no-hurt-camera-gxtq2c3t` 7/7, `weather-modes-6ww1o5th` 13/13.
- `hud_detail_scenario.py --part layouts`: 8/8 final (`hud-layouts-hxgrbp9r`, hud-modules-final-layouts) and 8/8 pre-merge (`hud-layouts-pokp7tue`, hud-modules-layouts).
  - 12 captures each, inspected. Every element sits where the reference draws it at both scales, the settings change it, and with every module off the host view is unchanged.
- `no_hurt_camera_scenario.py`: 7/7 final (`no-hurt-camera-jhph_tam`) and 7/7 pre-merge (`no-hurt-camera-hyms7sjv`).
  - Largest frame diff over the hurt tilt: 0.62 on, 20.88 off, 0.15 on again (pre-merge 0.51 / 21.27 / 0.10). Frames are level when on and tilted when off.
  - The bind, the GUI toggle, restart persistence and Panic all pass.
- `weather_modes_scenario.py`: 13/13 final (`weather-modes-ywj9gvqr`) and 13/13 pre-merge (`weather-modes-sm833i4s`).
  - Raining: sky luma off 191.2, on 93.9 (server rain 92.5), after 191.2. The server stays clear.
  - Clear: off 93.2, on 190.1 (server clear 192.5), after 93.5. The server keeps raining.
- Pre-merge regressions: `hud_detail_scenario.py --part details` 7/7 (`hud-details-1h6yuls5`), `hud_editor_scenario.py` 9/9 (`hud-editor-jg3sp6nx`), `hud_modes_scenario.py` 5/5 (`hud-modes-cpxfkdi3`), `hud_readouts_scenario.py` 7/7 (`hud-readouts-b2j7ahsx`).
- Settings parity: 645 definitions, no entries for these rows. Armor 1/1, Coordinates 1/1, Potions 2/2 and Weather 1/1 match; Compass, FPS and NoHurtCam declare no settings of their own.

Failures fixed along the way:
- Chat and the tutorial toast covered the panels; the layouts part hides them.
- Compass clamping and expired effects.
- The no-hurt-camera on-again pass measured 4.45 until command feedback was turned off.

Gates, final on 74a9aa4:
- `cargo fmt --all --check`: ok.
- clippy `-D warnings`: ok.
- `cargo test --workspace`: 213 + 1 + 4 + 0 passed, 0 failed (198 + 1 + 4 + 0 before the merges).
- payload-test: 213 passed.
- stage + audit: 44 hooks and 29 support modules, original AST restored. Main's `gui-blur` is the 44th; this branch adds none.
- host check: exit 0, 68 warnings, the same count as main.
- host build: exit 0.

Limits and open items:
- HUD text still uses the host bitmap game font and its shadow, not the reference TTF; Geist serves the GUI only. A HUD font switch is global styling across hud-client panels.
- At GUI scale 1 the 0.7× bitmap glyphs sample unevenly.
- Not ported: localization, the HUD editor alpha, and the armor bar glow and rounding.
- Not ported: the reference resets a hidden effect's ring maximum.
- The textures (armor, bar-1, compass*) are drawn as geometry and not shipped.

Next: a reference-font decision for the HUD and editor alpha, coordinated with hud-client.

## 2026-10-31: hud-modules (round 3) merged into main

Merged `t3/fdf14b2d` (head dc943ce) as 10bd7ad. The worker ported the reference HUD frames and validated ArmorStatus, PotionEffects, Coordinates (both modes), FPS, Compass, NoHurtCam and both Weather modes; 11 ledger entries, 10 of them implemented-core → validated (validated 91 → 101).

Gates on merged main (`gates.sh --host`): fmt, Clippy `-D warnings`, tests 431 passed / 0 failed, payload-test, stage/audit, host check and build.

Scenarios on merged main, each in a fresh isolated home: hud_detail 8/8, weather_modes 13/13, hud_editor 9/9, hud_modes 5/5, hud_readouts 7/7 and no_hurt_camera 7/7. The first no_hurt_camera run failed 5/7 at load average 10–12: with the module off, the first capture after the hit already fell past the ten-tick tilt (frame differences 1.49, 0.18, 0.22, 0.29, 0.33, against 21.15, 20.89, 15.09, 6.13, 1.67 on an immediate rerun). The scenario now measures the module-off pass up to three times when it misses the window. Every attempt stays in measurements.json, and the module-on passes are still checked once against the module-off peak. It passed 7/7 on the next run. Post-merge evidence: docs/evidence/main-hud-modules-{layouts,weather-modes,no-hurt-camera}.

Open, from the worker: HUD text still uses the host bitmap font (a cross-HUD decision shared with hud-client); localization, the HUD editor's transparency and the armor bar's glow are not ported.

## 2026-10-31: status and README refreshed; workers told to wrap up

- All nine round-3 workers were told to finish only their current item and to update progress.md, hooks.md and decisions.md (append-only). Each writes its ledger file, merges main `97dbaa7`, runs the gates and sends a final report naming its unfinished scope. Their branches are merged one at a time, with gates and scenarios rerun on main.
- docs/status.md was rewritten for main `97dbaa7`. It covers ledger counts (472 rows: 101 validated, 36 integrated, 54 implemented-core, 246 not-started, 12 blocked, 13 excluded-approved, 10 not-applicable-reviewed), the validated, integrated and implemented-core module lists, the work in flight, host blockers, gate and scenario evidence (431 tests, 44 hooks, 29 support modules), Phase 8 status and the re-pin candidate `5062f225`. Performance is still unvalidated until an idle-machine run.
- README "What works today" now lists only scenario-validated modules as tested. Integrated and partly connected modules are named separately. LongJump, Step and AntiFall are no longer advertised.
- New finding: 37 category-level reference classes have no ledger rows. Most are modules ModManager never registers (LongJump, AntiFall, Step, Strafe, Phase, Regen, OmniSprint, PotionSaver, AntiFML, DiscardedBlink, KnockbackTest, SelfDestruct, debug modules). The port ships LongJump, AntiFall and Step (compile-only, 2026-10-30) although the reference GUI never shows them. Open: decide whether to track them as extras or drop them for fidelity.
- Tests: docs only; no code changed. Gates for main `97dbaa7` are in the hud-modules entry above.

## 2026-10-31: combat part A (worker round 2)

Worker combat-a, branch `t3/1e32141e`. Main `97dbaa7` and the docs-only `8203df1` are merged in.

Changed files:
- Pure modules:
  - `src/modules/combat/hit_select.rs` (new)
  - `src/modules/combat/block_hit.rs` (new)
  - `src/modules/combat/hit_flick.rs` (new)
  - `src/modules/utility/target_filter.rs` (new)
  - small edits: `rotation.rs` (`Managed::tick_by`), `silent_aura.rs` (controller hand-off), `clicker.rs` (AutoClicker holds entity clicks that HitSelect would cancel), `blatant/outbound.rs`, `render/esp.rs` (TargetFilter recolour) and `targeting/filter.rs`
- Runtime glue: `src/core/combat_extra.rs`, `combat_runtime.rs`, `mod.rs`, `registry.rs` and `hud_extras.rs`.
- Host side:
  - five new hooks and three support modules: `mixins/vape_rs/hit_select.rs`, `block_hit.rs`, `teams.rs` and the `adapters/` entries
  - edits to `aura.rs`, `combat.rs`, `packets.rs` and `horizon-mixin.toml`
- Scenario tool: `tools/combat_a_scenario.py`.

Feature results (ledger updates in `target/ledger-updates/combat-a.json`):

| Feature | Rows | Status |
|---|---|---|
| HitSelect | `module.hitselect`, both modes (Normal, Right-click use cancel), helpers `module.wtapsprintresetmode` (with its KB reduction and Critical hits modes) and `module.wtaprightclickusecancelmode` | validated |
| BlockHit | `module.blockhit`, modes Manual/Predict/Auto/Lag, helpers `module.manualblockhitmode`, `predictblockhitmode`, `autoblockhitmode`, `lagblockhitmode` and `blockhitmode` | validated |
| TargetFilter | `module.antibot` (TargetFilter), helper `module.antibotentitycache` | validated |
| HitFlick | `module.hitflick`, helper `module.hitflickadaptiverotationcontroller` | implemented-core |

How each landed:
- **HitSelect** uses the reference wtap-package classes for its modes. It cancels through:
  - `hit-select-attack` (cancellable HEAD on `remote_start_attack`, priority 110)
  - `hit-select-use`
  - `hit-select-continue-attack`
- **BlockHit**:
  - On 26.3 it blocks with the off-hand shield (see decisions).
  - `block-hit-mouse` covers Manual and Auto.
  - `block-hit-send` covers Lag's held release-use queue.
  - Predict's damage notice comes from the inbound damage and entity events.
- **TargetFilter**:
  - Modes: Teams by server and Teams by color. Colours are detected from the player list, which is the reference's 26.3 path.
  - It is wired into the shared target filter, so KillAura, SilentAura, AimAssist and TriggerBot all honour it.
  - It recolours ESP.
- **HitFlick (pure port only)**:
  - Implemented: the flick phases, the adaptive controller speeds read on every update, and the managed rotation shared with SilentAura.
  - It shares the primary action claim with BlockHit Lag, and FakeLag Repel yields to either.
  - Not hooked in the host, and the aura passes an empty view, so it never leaves Idle in the host.

Scenarios. Each ran in fresh isolated homes through `tools/native_session.py`, on this branch's host builds of the work in progress. Each `build.json` records `source_dirty: true`: the HitSelect and TargetFilter runs on `574e57d`, the BlockHit runs on `bd28e42`. Those changes were committed as `bd28e42` and `e2efab5`; none of the runs postdate HitFlick `fa4aa29` or the merges. Evidence is in `docs/evidence/`:

| Evidence dir | Checks | What it shows |
|---|---|---|
| `combat-a-hitselect-normal-sp` | 9/9 | Normal cancels most clicks inside the invulnerability window; a cancelled click still swings |
| `combat-a-hitselect-rcuc-sp` | 9/9 | Right-click use cancel drops clicks on the invulnerable cow, without a swing |
| `combat-a-hitselect-kb-sp` | 9/9 | KB reduction: every click after the received knockback goes through |
| `combat-a-hitselect-crit-sp` | 9/9 | Critical hits: clicks while rising after the knockback are cancelled |
| `combat-a-blockhit-manual-sp` | 10/10 | Manual |
| `combat-a-blockhit-auto-sp` | 12/12 | Auto |
| `combat-a-blockhit-lag-lan-guest` | 12/12 | Lag on a LAN guest: the held release-use stalls the guest on the host |
| `combat-a-blockhit-predict-lan-guest` | 11/11 | Predict on a LAN guest |
| `combat-a-targetfilter-lan-guest` (plus `-lan-host`) | 20/20 | KillAura spares a same-team host and attacks one in another team; Teams by color detects red `#FF5555`, spares a red host and attacks a blue one; ESP captures |

Gates on head `5ed4f97`:

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| Clippy `-D warnings` | clean |
| `cargo test --workspace --locked --offline` | 238 + 1 + 4 + 0 passed, 0 failed |
| payload-test | 238 passed |
| stage + audit | ok: 49 hooks (47 HEAD + 2 MODIFY_ARG) and 32 support modules, original AST restored; this branch adds 5 hooks and 3 modules to main's 44/29 |
| host check | not run on this branch: the queued locked job was stopped at the coordinator's request; the coordinator runs `gates.sh --host` (host check and build) and these scenarios on main after the merge |
| host build | not run on this branch (same reason) |

Limits and open items in this scope:
- HitFlick host side:
  - the physical (`play_mouse`) and synthetic (`attack_once`) attack intercepts, which are the reference attack-cancellation adapters
  - Blink's packet hold
  - the `set_sprinting` cancel
  - the forward and sprint key owner
  - a scenario
- `module.physicalattackcancellationadapter` and `module.syntheticattackcancellationadapter` stay not-started. In the reference they serve HitSwap and AutoMace (combat-b) and HitFlick's intercepts.
- Not started:
  - SilentAura: the Ring and Box render modes, Show target, Target color and Attack color, Break blocks (with its whitelist, items and delay), Kiss rotation and `onSyntheticAttack`, plus `module.silentauraaimjitter` and `module.silentauraentityidcomparator`
  - KillAura's Show target, Target color and Attack color
  - AimAssist's Adaptive mode, with `module.aimassisttargetingsubmodule` and `module.aimassistworkerthread`
  - the riding-packet and third-person rotation in the combat rotation claim
- TargetFilter:
  - The Invalid Player Spawn notification is not ported.
  - The scenario exercised KillAura only. The other three consumers share the filter and have unit tests.
  - The 16 "Your team color" rows (`module.antibot.mode.*`), Auto-Detect and `module.antibotstatetracker` are legacy-only (< 1.21.11). They are left at not-started for a not-applicable review.
- BlockHit:
  - ItemMacroAction's BlockHit interaction is not ported.
  - The 0.25 s shield warm-up limits Predict.
- The primary action claim is two OR'd flags, not a shared claim object.
- The header on line 3 of docs/hooks.md still gives the old 38/25 count; the current count is in the note at its end.

Next: the HitFlick host intercepts, which would also give the attack-cancellation adapters a host side for combat-b, then the SilentAura render modes.

## 2026-10-31: inventory (worker round 2)

Fixed or ported:
- Refill: when the player closed the inventory inside the first click delay, Refill reopened it. Now `Refill::observe` runs every update, so finishing (close and disable) isn't paced by the delay. Test: `a_close_inside_the_first_delay_finishes_without_reopening`.
- AutoTotem "Silent open" and "Silent move delay":
  - The swap happens with no screen shown.
  - Movement, use, attack and break are held for the session plus one delay, through three new HEAD hooks (`auto-totem-use`, `auto-totem-attack`, `auto-totem-break`; see hooks.md).
- ChestSteal "Check in menu" now defaults to off. AutoHeal's hidden "Inventory Only" is not ported (decisions.md).
- InventoryManager / InvCleaner: the earlier commits are listed in decisions.md:
  - `d831f4e`: best armor
  - `4c1e905`: idles without a profile
  - `38949b6`: AutoHotbar any material

Validation:
- Gates:
  - fmt clean
  - all-target Clippy `-D warnings` clean
  - 216 lib + 3 integration tests pass
  - payload-test passes (216)
  - the fast host build passes, and stage/audit removes exactly 46 hooks and 29 support modules
- Host scenarios: three, all in survival on fresh isolated homes. Captures were inspected.
- `tools/inventory_planners_scenario.py`: 10/10 cases pass (`inv-plan-8y1g__lh`). Every case checks that item counts are conserved, nothing stays on the cursor, the selected slot is unchanged, the screen ends closed and the module turns off.
  - InvCleaner: drops 4 rotten flesh, 1 harmful splash potion, 1 stone sword and 5 sticks.
  - InvCleaner, interrupted with Escape: 3 thrown, 5 left, with no reopen.
  - InvCleaner, user cursor stack: it waits, and the 2 held flint never drop.
  - InvCleaner, item picked up mid-run: the picked-up items are cleaned in the same run.
  - InventoryManager: it equips armor and drops junk. With a full inventory, the stone sword displaced from slot 19 is kept, not lost.
  - AutoHotbar: puts the rules' items in their hotbar slots; with a user cursor stack it waits.
  - ArmorSwitch: switches there and back. When interrupted, only the head is swapped, and the rerun changes nothing.
- `tools/inventory_scenario.py` (`inv-rec-opt5z12g`):
  - Refill interrupted: 0 hotbar soups.
  - Refill complete: 3 soups on the hotbar, 2 left in main.
  - ChestSteal interrupt and reopen: 1 item taken, then 6.
  - AutoTotem equip: totem in the offhand, 1 left in main.
- `tools/inventory_parity_scenario.py` (`inv-parity-ophwsw3r`):
  - ChestSteal Best only: takes the diamond helmet and 32 cobblestone, leaves the stone sword (worse than the held diamond sword) and closes the chest.
  - AutoTotem Silent open:
    - The screen stays `playing` throughout, with W and the use key held.
    - The hold lasts 0.486 s (30 samples), and no snowballs are thrown during it.
    - The totem is equipped at 0.327 s, inside the 0.26–0.746 s window.
    - Walking (0.118) and throwing resume afterwards.
- Evidence is in docs/evidence/inventory-r2-{planners,recovery,parity}/.

Open:
- InventoryFill has no host scenario. The control socket can't hold a mouse button across frames in a container screen: `mouse` requires the playing screen and never sets `physical_buttons`, and `drag-at`/`click-at` send the press and release in one event batch. It stays integrated.
- AutoHotbar "Only best" scores items with our own categories, not the reference's per-class comparators (food nutrition×saturation, the potion per-effect amplifier compare, attack-speed and fire-aspect terms). The comparator helper rows stay below validated.
- AutoTotem's non-silent screen does not yet cancel the player's own clicks while AutoTotem has the inventory open (the reference's `onWindowClick`; decisions.md).
- AutoArmor's Delay range is out of scope for this round.

### Later in the round (final head)

Landed after the runs above:
- `d75d562`: AutoHotbar "Only best" uses the reference's per-class comparators, menu-order candidates and run-long touched slots (decisions.md). This resolves the comparator item under "Open" above.
- `3d1ea7f`: InventoryManager profile selection uses `findByName`, case-insensitive. No match selects nothing.
- `4dd6f8f` and `f4fd2cd`: AutoHeal Silent is ported, so it is no longer blocked.
  - Custom items are used at onTick with a server-only held-item change, and the client's selected slot never moves.
  - "Potions" swaps the potion into "Slot" and sends 88.99 pitch through `auto-heal-pre-motion` (HEAD of `remote_send_state`).
  - `auto-heal-post-motion` (HEAD of the empty `native_tick`, right after the motion packet) throws it. The camera never moves.
- `f4fd2cd`: Throwpot's tie order follows `ItemDataComparator` (decisions.md).
- Merged main at `5d44e13` (10bd7ad) and `109d27c` (97dbaa7).

Gates on `109d27c`:
- fmt clean
- all-target Clippy `-D warnings` clean
- tests: 246 lib + 1 integration + 4 xtask pass
- payload-test passes (246)
- host build passes, and stage/audit removes exactly 49 hooks and 29 support modules

Scenarios rerun on the clean `109d27c` build, each in survival on a fresh isolated home. Captures were inspected.
- `tools/inventory_planners_scenario.py` `inv-plan-2tmlf9o9`: 11/11, the 10 cases above plus `hotbar_comparators`.
  - The latter takes an iron pickaxe over the faster golden one, the Power II bow, Swiftness II, and golden carrots over pumpkin pie.
  - Case 7 takes the iron sword over the stone and wooden ones.
- `tools/inventory_scenario.py` `inv-rec-a3h0tumc`: 4/4.
  - The first run, `inv-rec-i0ftuvdr`, passed both Refill cases. It then failed before ChestSteal: `interact` reported the player 42.9 blocks from the chest at load average 17. The rerun passed.
- `tools/inventory_parity_scenario.py` `inv-parity-skftk8v_`: 2/2.
  - ChestSteal Best only passes.
  - AutoTotem Silent open: the screen stays `playing`. The hold lasts 0.591 s over 35 samples, the totem is equipped at 0.417 s inside the 0.266–0.857 s window, and walking resumes at 0.118.
- `tools/auto_heal_silent_scenario.py` `inv-heal-sqi9ovc6`: 2/2.
  - Custom item: an Instant Health splash potion in slot 4.
    - Nothing is used while the inventory screen is open.
    - After it closes, health goes from 10 to 14, the client's slot stays 1 and the snowballs stay 16.
    - The server's `SelectedItemSlot` is 0, and a right click throws a snowball (16 → 15).
  - Potions with Replace: the potion in inventory.13 is swapped into Slot 6 and thrown, and health goes from 10 to 14.
    - The camera stays at yaw 0, pitch -10 throughout.
    - The client's slot stays 1, the stick stays in slot 1, and nothing is left on the cursor.
  - The first run, `inv-heal-whkoefuz`, looked level, so its splash missed the player. Case 1 now looks down, because custom items keep the camera's look, as in the reference.
- Evidence is in docs/evidence/inventory-r2-final-{planners,recovery,parity}/ and docs/evidence/inventory-r2-autoheal-silent/.

Still open in this scope:
- The InventoryManager/InvCleaner rule editor UI (brief part B) is not built. The reference has about 4,200 lines of cleaner UI and 1,410 of HotbarSlotRule UI. Its rows stay not-started.
- InventoryFill: no host scenario, because the harness can't hold a mouse button in a container screen (above).
- AutoTotem's non-silent `onWindowClick` cancel.
- AutoHeal deviations:
  - no same-slot swap
  - the crafting grid isn't searched
  - vanilla stew and food are not eaten by a single use
  - Silent "Bowl mode" runs through the inventory session
  - the bowl modes (Throw, Stack, None) have no scenario
- AutoHotbar deviations: `findOverflowSlots` kinds, and moves are number-key swaps.
- Modes without a scenario:
  - the Toggle activations
  - ArmorSwitch gold, leather and chain
- ChestSteal's distance comparator: the take order isn't asserted in a host scenario.
- ItemDataComparator: the Throwpot order isn't asserted in a host scenario.
- InventoryActionGuard: no combat scenario.
- ThrowDebuff orders nearest-first; the reference doesn't sort. Left to its owner.
- Host gates: the host build above ran on `109d27c`. `5195a4f` and later change only docs and the AutoHeal scenario tool. On the coordinator's instruction, the post-merge host check and build are not run here; the coordinator runs `gates.sh --host` and these scenarios on main after the merge.

## 2026-10-31: client foundation (worker round 3)

Worker client-foundation, branch t3/7064aa90. Rows: client.bindings, client.macros, client.profiles; client.friends next.

Changes so far:
- **Profiles** (`src/core/profile_runtime.rs`, new): the reference profile model (`ProfilesManager`, `Profile`, built-ins).
  - A fresh install seeds "Modern PVP" and "Classic PVP" and selects Modern PVP. A pre-profile live state migrates into a "Default" profile, numbered if that name is taken.
  - The selected profile follows the live state. Switching captures the profile left behind.
  - `PublicProfileSettings` values stay global.
  - Module states are applied only with "Auto-load module states". HUD modules are always restored at startup.
  - A right click applies a profile's saved non-HUD states.
  - New profiles and local imports go first (`addProfile(profile, true)`).
  - The selected profile's trash button is inert ("You cannot delete your selected profile").
  - Per-profile keybinds work in the GUI row menu and are gated by open screens.
  - New client settings: "Auto-load module states" and "Frame positions per profile".
- **Profiles storage** (`src/core/profiles.rs`): profile file names are percent-encoded, so any name stays inside `profiles/`. Non-canonical stems are not listed.
- **Macros** (`src/core/macro_runtime.rs`, `src/input/macros.rs`): names may repeat, as in the reference's `LinkedHashSet`. An edit moves the macro last. GUI edits address macros by position.
- **Bindings**: GUI rebinding scenario (`tools/bindings_scenario.py`) covering chord capture, Escape/left-click clear, the shift-click hold toggle, mouse-button binds and restart.
- **Scenarios**:
  - new: `tools/macro_scenario.py` and `tools/profile_switch_scenario.py`;
  - updated for the new profile flow: `tools/gui_scenario.py` and `tools/text_editor_scenario.py`.

Tests:
- fmt clean; clippy `-D warnings` clean.
- `cargo test --workspace`: 215 + 1 + 4 + 0 passed.
- payload-test: 215 passed.

Host build and native scenarios are pending a rebuild after merging main (608c800). This entry will record those results.

### Wrap-up (round 3, 2026-10-31)

Landed after the entry above (commit 7e4cb2c, plus this wrap-up commit):

- **Gui Color is the accent** (`ThemeColors.z()`, default RGB(5,134,105)): toggles, tab underlines, module cards, macro icons and Text GUI "Match GUI color". `ThemeColors.O` stays fixed.
- **The click GUI over play.**
  - The new `gui-no-pause` hook keeps single-player ticking.
  - The vanilla HUD stays under the GUI, with no dim.
  - The host's "Game Paused" title is removed (`vape_hud::drop_paused_title`).
  - The Text GUI, Target Info and notifications draw over the GUI.
  - The HUD editor keeps its dim.
- **Text GUI "Click disable".** A hover blends the row (20% toward white). A click toggles the row's module. A module switched off this way stays listed with a grey "| Disabled" suffix until the GUI closes.
- **Audit fix.** The first build of this tree failed the audit ("unexpected transformed AST changes: debug_keys.rs"): the staged `gui_render` helper had its `.filter` closure body wrapped in a block. The helper now uses `for`/`if` (docs/hooks.md).

Gates on this branch, before merging main cdc0228:
- `cargo fmt --all --check`: exit 0.
- clippy `-D warnings`: exit 0.
- `cargo test --workspace`: 239 + 1 + 4 + 0 passed.
- payload-test: 239 passed.
- Host build (`xtask build` under the shared lock): exit 0. The audit restored the original AST after removing exactly 45 hooks and 29 support modules. Bundle `d07bec67dcd5…`.

Host scenarios, each in a fresh isolated home on that build:
- `tools/text_gui_click_scenario.py`: PASS 18/18. Evidence: `docs/evidence/client-foundation-r3-text-gui-click/`. With the GUI open:
  - `state.paused` is false and the world clock advances (day time 45 → 84 over 40 ticks);
  - the hotbar is unchanged (RMS 0.46), there are no "Game Paused" shadow pixels, and the Text GUI draws over the GUI;
  - with Click disable off, a hover and a click change nothing;
  - with it on, a hover raises the red row's green channel 14 → 58.5, a click disables AutoClicker and adds the suffix, a second click re-enables it, a third disables it, and closing the GUI drops the held row;
  - Match GUI color: hue 0.598, Gui Color 0.598.
- Regression reruns after removing the GUI dim:
  - `tools/gui_blur_rainbow_scenario.py`: PASS 6/6. Blur on fits the blurred play capture 2.88 vs sharp 14.48; blur off, sharp 0.78 vs blurred 16.10. Evidence: `docs/evidence/client-foundation-r3-blur-rainbow/`.
  - `tools/text_gui_scenario.py`: PASS 11/11. Evidence: `docs/evidence/client-foundation-r3-text-gui/`.

Not run on a current build:
- `tools/bindings_scenario.py`, `tools/macro_scenario.py` and `tools/profile_switch_scenario.py`: their last runs stopped with StaleBuild.
- `tools/friends_lan_scenario.py`: written, never run. Its docstring says so.
- `tools/gui_scenario.py`, `tools/text_editor_scenario.py` and `tools/hud_editor_scenario.py`: not rerun after the GUI changes.

The coordinator runs the host check and build after merging this branch.

Gates after merging main cdc0228 (merge a44cfab), with no host build:
- `cargo fmt --all --check`: exit 0.
- clippy `-D warnings`: exit 0.
- `cargo test --workspace`: 297 + 1 + 4 + 0 passed.
- payload-test: 297 passed.

The scenario evidence above comes from bundle `d07bec67…`, built before that merge.
