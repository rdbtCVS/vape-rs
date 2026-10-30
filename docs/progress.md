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
