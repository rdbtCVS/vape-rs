# Vape-rs: complete Rust reimplementation for Horizon

Written 2026-10-28; updated during implementation. This remains the complete implementation plan, not a completion report. Current code, evidence, open dependencies and next steps are summarized in [docs/status.md](docs/status.md); chronological milestone results are in [docs/progress.md](docs/progress.md). Forty-three modules (several partial, experimental or compile-only) and the local client foundation are connected, while the complete feature ledger and release gates remain unfinished. The user requested implementation-first batches with testing afterward; recent compile-only evidence must not be treated as gameplay/visual validation. Keep the validation gap bounded: each batch rebuilds the executable and runs at least one fresh-home scenario per newly connected module.

## 1. Objective and user decisions

Recreate the complete useful client feature set of the adjacent Vape-v4 project as a Rust-native Horizon mod. This is a rewrite, not Java compatibility, a JVM embedding exercise, an injectable client, or a standalone Minecraft implementation.

The user explicitly wants:

- Rust for all player-facing code.
- Integration as a Horizon mod through its native source-mixin system.
- One pinned Horizon/Minecraft version at a time.
- Removal of Java, JNI/JVMTI, process injection, and unnecessary multi-version machinery.
- Near-1:1 fidelity across all retained modules, submodes, UI, visuals, settings and interactions. AI behavior is explicitly excluded by the user; all other retained modules are wanted; simpler ones may be implemented first, but prioritization is not pruning.
- A near-1:1 recreation of the existing interface, not a redesigned or deliberately simplified replacement.
- Removal of AI behavior, including SilentAura’s neural rotation mode, model selection, inference, model weights and AI-only dependencies. SilentAura’s PID and other non-AI behavior remain in scope.
- Complete removal of the online product: accounts, licensing, cloud/public profiles, reviews, parties, shared activity/inventories, remote synchronization and their backends. Local profiles and local friend/enemy management remain.
- Public distribution to general Horizon users, with continuing updates as Horizon evolves. This is not a personal Linux-only prototype frozen indefinitely on one revision.
- Rebuilding Horizon for native-mod installation/update is acceptable.
- Support for Horizon clients in both single-player and multiplayer. These are validation contexts for one mod, not alternative product scopes.

Do not interpret the staged implementation below as authorization to stop after the scaffold or prototype. Completion means the feature ledger is closed, with actual working integrations and evidence. Do not ask the user to pick a smaller module subset again. Some obsolete behavior needs a modern interpretation; document those decisions rather than silently dropping features. Unsupported old version gates are not sufficient grounds to exclude useful modules.

Do not delete the Java reference repository. The finished vape-rs distribution must have no Java dependency, but the adjacent checkout remains a read-only behavioral reference.

## 2. Workspace and reference map

| Location | Role |
| --- | --- |
| `./` (this repository) | New independent project; write implementation here |
| `../Vape-v4` | Reference implementation; do not build or execute it |
| `../horizon` | Horizon Rust source, lowercase directory name; inspect without editing during mod development |
| `../horizon/AGENTS.md`, `../horizon/CONTRIBUTING.md` | Read before working against Horizon; follow test/data isolation rules |
| `../horizon/docs/native-mods.md` | Native mod packaging, build pipeline, limitations |
| `../horizon/launcher/src/native_mods.rs` | Actual launcher source staging, tool pinning, building, installation and cache behavior |
| `../horizon/crates/horizon-mods/src/index/native.rs` | `.hnmod` validation and compatibility manifest |
| `../horizon/docs/mods.md` | Sandboxed API; some introductory status tables lag newer source |
| `../horizon/crates/horizon-mod-sdk/wit/horizon.wit` | Actual sandbox interface contract, useful for comparison |

Observed reference revisions:

- Vape-v4: `3d600a48106822755cea2d633062e5ab1f383d48`, project version `4.21.43`.
- Local Horizon at planning: `e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083`. Current pin (2026-10-29): `cc7bcbf4e508c1484dd4ae14741d78a8fbe745d1` (Horizon main; previously Nightly `9669600`); see docs/decisions.md.
- Local launcher's default horizon-mixins pin: `0500ff24aa5933fca95a877ef41125353c4b4fe3`.
- The native template's demonstrated Horizon pin is older: `028f60881678a9a0e3e198f693a82858fa475c76`. Its selectors are examples, not validated selectors for the local Horizon revision.

Record actual working-tree changes and exact revisions at implementation start. A dirty local tree is not the same as an export of its committed pin. Do not overwrite others' work, switch their branches, or assume today's HEAD remains unchanged.

Primary external references inspected while planning:

- Native tool: <https://github.com/horizon-rust/horizon-mixins>
- Native starter: <https://github.com/horizon-rust/horizon-native-mod-template>
- Template instructions: <https://github.com/horizon-rust/horizon-native-mod-template/blob/main/README.md>
- Tool's boundary/module contracts: <https://github.com/horizon-rust/horizon-mixins/blob/main/docs/boundary-head-and-modules.md>
- Tool's state/effect contracts: <https://github.com/horizon-rust/horizon-mixins/blob/main/docs/effects-and-state.md>
- Registry: <https://github.com/horizon-rust/horizon-mods>
- Registry publication policy: <https://github.com/horizon-rust/horizon-mods/blob/main/MOD-POLICY.md>
- Existing native port example: <https://github.com/levzzz5154/horizon-veinminer>

Recheck exact pinned contracts instead of building against a moving README. The template and local launcher have different demonstrated revisions and platform coverage.

## 3. Architecture decision: native source mixins

Use an independent native mod bundle (`.hnmod`), not only a sandboxed Wasm `.hmod`.

Horizon has two distinct paths:

1. Sandboxed Wasm components expose bounded game snapshots, HUD/UI, keybinds, storage and selected actions. They do not provide arbitrary entity access, mutable movement, packet interception or unrestricted world rendering. They cannot support the full rewrite today.
2. Native source mixins transform a staged copy of pinned Horizon source and compile an independent game executable. This supplies the necessary integration while remaining a mod. Nothing attaches to a running process, and no external binary is injected.

The native tool implements HEAD/RETURN hooks, selected interior hooks/cancellation, argument/expression modification, wrappers, access bridges, support modules and per-instance state. Each has exact syntax and ownership restrictions. A successful plan is not proof that the transformed Rust compiles.

Important implementation constraints:

- Hooks are ordinary Rust code compiled in the target crate/module. Rust privacy, lifetimes, borrowing and dependency availability still apply.
- The inspected template permits support modules with inline Rust modules. It explicitly disallows inner file attributes/comments, external `mod` files and `include!` in module payloads. Verify the pinned tool before choosing a file layout.
- Dependencies declared by this standalone project's Cargo manifest are not automatically added to Horizon. Prefer existing host dependencies and standard Rust. Any staging dependency edits need an explicit reproducible mechanism and audit.
- Keep editable runtime sources modular for maintainability. If the tool still requires a flattened module payload, create a deterministic developer-side Rust packer with a source map and content hashes. Test the emitted code, not only the editable source. Do not assume copying a Cargo crate into the bundle makes it importable.
- `.hnmod` files use `horizon-native.json` and `mixins/<id>/horizon-mixin.toml` with Rust/TOML source files. The inspected reader rejects non-source files under the mixin directory and limits entry count/expanded size. Confirm limits at the pin. Do not put fonts, model binaries or textures into it assuming they will load.
- Pin full 40-character Horizon and mixin revisions; the manifest's compatibility fields and registry metadata must agree. No native dynamic-library ABI is needed.
- The launcher applies mods to source fetched for the pin and compiles its own build; it does not use arbitrary edits in `../horizon`.
- Native mods have the game's full OS privileges. Instance separation is not a security sandbox.

The local launcher already stages and builds native mods; this is not merely a proposed API. However, docs mark it experimental and say full game builds on Windows/Linux are unproven. Validate the actual target platform instead of promising cross-platform readiness from Rust compilation alone.

## 4. Scope: retain functionality, remove obsolete infrastructure

### Remove from the new product

- C/C++ loader, DLL extraction/loading, remote-process operations, debug privilege requests and reflective injection.
- JNI/JVMTI bridge, Java classloaders, ASM/Javassist rewriting, JVM reflection wrappers and obfuscation helpers.
- Forge/Fabric/NeoForge/Badlion mappings, version-dispatch wrappers and per-version resource mappings.
- Java/Gradle build machinery and Java runtime dependencies.
- License-enforcement/token bootstrap and dead upstream authentication infrastructure that existed to launch/inject the client. A local mod must work without a Vape account or local authentication server.
- The entire online product and its UI entry points: remote accounts/authentication, cloud/public profiles, reviews, parties, shared inventories/activity and remote synchronization. Remove obsolete online-only settings instead of leaving broken buttons or fake successful responses.
- Automatic hidden payload extraction and recursive cleanup of old client directories.
- Legacy OpenGL abstractions; render through Horizon/wgpu.
- Old-version-only compatibility patches when their purpose truly does not exist in the pinned game. Record every such case in the ledger with a reason.

### Preserve and reimplement

- All meaningful gameplay modules and their submodes, settings, conditions, filters and interactions.
- Click GUI, module categories/search/favorites, tooltips, keybind editing, profiles, macros, notifications and HUD customization.
- Friend/enemy/team filtering and local management.
- Render features and visual customizations using the host renderer.
- Useful debug/performance reporting through ordinary local diagnostics.

### Confirmed exclusions and retained local behavior

The user explicitly approved deleting the entire online product. Mark those ledger entries `excluded-approved` with a reference to this decision. They are not future work and do not block completion. Keep local profile management, import/export between files, local friends/enemies/team filters and all local gameplay features that formerly depended incidentally on an online manager. Extract those dependencies rather than removing their local behavior.

No service process, remote login or mod-owned network backend is required. Horizon's normal multiplayer networking is unaffected by this exclusion. Preserve near-1:1 visual behavior for retained screens; omit online-only pages and controls cleanly rather than recreating them as empty placeholders.

### Approved AI exclusion

The user explicitly requested removing AI behavior from this plan. Exclude SilentAura's AI rotation submode, its model-selection settings, neural inference, model weights and AI-only helpers/dependencies. Do not implement, package or validate those branches. Mark their ledger entries `excluded-approved`; they do not block completion.

Keep SilentAura itself, its PID rotation mode and all other non-AI targeting, aiming, combat and movement behavior. Remove AI-only controls cleanly instead of leaving unavailable options in the UI. This decision supersedes the earlier requirement to retain the AI submode. It does not authorize additional exclusions.

## 5. Reference source: where behavior lives

Root all paths in this section at `../Vape-v4/src/main/java/gg/vape/`.

| Area | Start here |
| --- | --- |
| Startup, initialization, cleanup | `Vape.java`, `lifecycle/` |
| Module inventory and version gates | `manager/ModManager.java`, `manager/ModRegistrationBuilder.java`, `module/Category.java` |
| Lifecycle, settings, submodules | `module/Mod.java`, `module/SubModule.java`, `value/`, `unmap/PropertyContainer.java` |
| Event order and cancellation | `event/`, `event/impl/` |
| Input, binds, macros, click timing | `input/`, `click/`, `module/Macro.java`, `manager/MacroManager.java` |
| Targeting and rotations | `combat/`, `rotation/`, module-specific helper directories |
| Inventory/planning/action conflicts | `inventory/`, `module/control/`, `module/utility/`, `module/blatant/` |
| Movement | `movement/`, relevant combat/blatant/utility modules |
| GUI/HUD/rendering | `ui/`, `render/`, `utils/render/`, `module/render/`, `module/none/` |
| Local configuration and profiles | `config/`, `sync/`, `manager/client/ProfilesManager.java` |
| Friends and online features | `friend/`, `manager/client/Online*`, `api/`, `protocol/`, `service/` |
| Localization/assets/model data | `../Vape-v4/src/main/resources/` |

The project has approximately 2,975 Java source files. Registration order and version gates matter. Names of classes can be misleading; trace constructors, settings creation, event subscribers, helpers and downstream actions. Do not run it to discover behavior.

## 6. Exhaustive feature ledger

Create `docs/features.csv` (or an equally machine-readable file) before extensive implementation. One row per user-visible module, submode and cross-cutting feature. A module name alone is insufficient.

Required fields:

`id`, `display_name`, `category`, `reference_paths`, `reference_methods`, `settings_and_defaults`, `visibility_conditions`, `triggers`, `inputs`, `state_machine`, `actions`, `disable_cleanup`, `dependencies`, `conflicts`, `old_version_gate`, `modern_semantics`, `horizon_hook_ids`, `tests`, `status`, `evidence`, `decision`.

Allowed statuses: `not-started`, `specified`, `implemented-core`, `integrated`, `validated`, `blocked`, `decision-needed`, `excluded-approved`, `not-applicable-reviewed`.

Only `validated` counts as ported. A visible GUI toggle, no-op implementation, fake snapshot, uncalled Rust module or unit test without host integration does not count.

Use `ModManager.init()`, `registerTextGuiSettings()` and `registerHudModules()` as the first inventory, then inspect constructor-created submodules and other managers. Deduplicate repeated registrations such as BedPlates, NoItemRelease and Timer. Distinguish helper submodules from independent entries.

### Seed inventory: registered gameplay and client modules

This is a discovered seed list, not a claim that the audit is exhaustive. Retain actual source category labels and UI names when filling the ledger.

- Framework/settings: ClientSettings, TextGuiSettings, Panic.
- Clicking and combat: LeftClicker, RightClicker, Velocity, JumpReset, KnockbackDelay, Reach, WTap, AimAssist, KillAura, BowAimbot, HitSelect, BlockHit, SilentAura, its SilentAuraClicker child, HitFlick, AntiBot, Triggerbot, HitSwap, AutoMace, ShieldBreaker, CrystalAura and its placement/target submodules.
- Inventory, equipment and consumables: Throwpot, Refill, ChestSteal, AutoArmor, InvCleaner, ThrowDebuff, AutoTool, AutoPearl, ArmorSwitch, AutoHotbar, AutoHeal, InventoryManager, InventoryFill, AutoTotem, AutoFish, NoItemRelease.
- Movement, placement and rescue: KeepSprint, FastPlace, Scaffold, SafeWalk, Fly, MLG, Parkour, Sprint, BlockIn and its planning/render helpers, InvWalk, BedBreaker, AutoLadder, Clutch, NoFall, NoSlowdown, Speed, Timer, AutoAnchor, WindCharge, PearlCatch.
- Network/time behavior: Blink, Backtrack, FakeLag and their queue/delay modes; cross-reference combat/movement features with packet effects.
- World/context: Search, SpawnerFinder, AntiAFK, PropHunt, MurderFinder, XRay.
- Visuals: Tracers, NameTags, ESP (2D, 3D, outline and skeleton modes), StorageESP, Fullbright, Trajectories, AntiDebuff, Arrows, ItemESP, Indicators, Health, BedPlates, Explosions, Chams, Freecam, Projectiles, HitBoxes.

### Seed inventory: HUD and visual behavior

FreeLookHudModule, NoClickDelayHudModule, MouseDelayFix, KeystrokesHudModule, ClockHudModule, PotionEffectsHudModule, BlockhitAnimationHudModule, NoHurtDelayHudModule, ArmorStatusHudModule, CompassHudModule, WeatherChangerHudModule, NoHurtCameraHudModule, TimeChangerHudModule, CoordinatesHudModule, FpsDisplayHudModule, ReachDisplayHudModule, NoFogHudModule, BlockOverlayHudModule, BlockRenderColorOverrideHudModule, ScoreboardHudModule, InventoryBlurHudModule and MotionBlur.

Some classes named `HudModule` alter input/camera/gameplay; classify by actual behavior. NoClickDelay/MouseDelayFix and BlockhitAnimation have old-version-specific reasons. Decide modern equivalents individually. Removing multi-version support does not automatically remove a feature previously gated off on modern versions.

### Cross-cutting inventory

- Module list overlay, ordering, color schemes, detail suffixes and visibility.
- GUI windows/pages, search, favorites, navigation, scaling, animations and all value editors.
- Toggle/hold/release binding modes, multiple binds, mouse buttons, keybind editing and conflict feedback.
- Macros: triggers, sequencing, delay/cancellation and profile behavior; inspect actual supported actions.
- Profile save/load/rename/delete/import/export, built-in profiles, defaults, dirty state and notifications.
- Local friends/enemies, aliases, team detection, bot filtering and entity target selectors.
- HUD anchors, drag layout, snapping, scale, colors, item/text rendering and screen-safe placement.
- Localization, text entry, font fallback and resource loading.
- Target indicators/target info, inventory display, rotation state, trajectory/path previews.
- Randomized timing, adaptive rotation, recorded/model data and deterministic test seams.
- Inter-module arbitration for rotations, attack/use, hotbar, inventory, rescue and render passes.
- Online-only behavior audit, recorded as approved exclusions; verify local features no longer depend on it.

## 7. Rust architecture

Keep the feature logic testable without compiling or launching Horizon. Do not reproduce the Java class hierarchy or stringly typed reflection bus.

Suggested conceptual layout (adapt payload layout to the pinned tool):

```text
Cargo.toml / Cargo.lock         standalone Rust workspace, pinned dependencies
compatibility.json             full source/tool revisions, target game, source hashes
src/
  core/                        registry, lifecycle, events, settings, config, profiles
  input/                       binds, macros, tick/frame timing
  world/                       snapshots, entity handles, targeting and math
  actions/                     arbitration and bounded action requests
  modules/{combat,movement,utility,world,render,hud}/
  ui/                          model, layout, widgets and rendering commands
  host/                        small owned data contracts; no fake implementation in release
mixins/vape-rs/
  horizon-mixin.toml            actual validated source selectors and bindings
  adapters/*.rs                thin host-specific hooks, subject to packaging rules
  runtime.rs                   deterministic emitted payload if flattening is necessary
tools/                         Rust developer tools for packing, staging, auditing and tests
docs/features.csv              exhaustive parity ledger
docs/hooks.md                  source selectors, ordering, threads, hazards
docs/decisions.md               explicit product/modernization choices
tests/                         state-machine, integration and scenario coverage
```

Use typed events and requests. Examples are tick pre/post, physical input, requested movement/rotation, attack/use, inventory transaction, world change, network ingress/egress and render stages. Define their actual host timing before coding modules against them.

### State and ownership

- Prefer a per-client runtime attached through supported mixin state. Never use one mutable global runtime for the renderer, integrated server and multiple `Game` instances.
- Build bounded owned snapshots of relevant entities/world/inventory; identify entities by ID plus world/session generation. Do not retain borrowed host references across ticks.
- Modules produce typed action requests; a central coordinator arbitrates priority, phase, cancellation and resource ownership. Respect physical versus synthetic input.
- Prevent recursive synthetic actions re-entering their own hooks. Preserve original function return values and ensure original code executes exactly once unless cancellation is intentional.
- Separate client-visible state from server-authoritative state. Client-only installation must not depend on modifying a remote server.
- On disable, panic, disconnect, death, world switch or profile swap, release owned input and rotation/hotbar claims, clear queued actions and restore rendering/camera overrides safely.
- Use tick-based deadlines for gameplay and a monotonic clock for UI/subtick timing. Inject clocks and seeded randomness in tests; do not tie behavior to FPS accidentally.

### Configuration and resources

- Versioned readable configuration, stable module/setting IDs, typed validation, atomic writes and deterministic migrations.
- Write only within this mod's instance-owned directory; no broad recursive deletion and no hidden payload cache.
- Preserve unknown settings where reasonable; clamp/reject malformed values with a clear diagnostic. Avoid silently enabling modules on import.
- Provide explicit, data-only legacy JSON import if desired. Never deserialize or execute Java objects. This is an optional migration aid, not a Java runtime dependency.
- Classify assets for reuse and provenance. Prefer host fonts/icons where appropriate. New shaders must be WGSL/host-compatible.
- Resolve `.hnmod` source-only asset restrictions early. Consider code-encoded approved small assets or an explicitly supported companion resource pack; measure size and build cost. Do not make installation depend on files outside the distributed mod without documenting them.

## 8. Horizon integration research and hook catalogue

Start with these paths; function names/selectors must be verified against the selected commit, not guessed from filenames:

| Needed integration | Horizon starting point |
| --- | --- |
| Client lifecycle, input event dispatch, tick/frame ordering | `crates/horizon-client/src/game.rs` |
| Existing mod tick/frame snapshot integration | `crates/horizon-client/src/game/mods.rs` |
| Input mapping and mod input | `crates/horizon-client-input/src/`, `game/mod_input.rs` |
| HUD retained shapes and rendering | `game/mod_render.rs`, `hud.rs`, `hud_layout.rs`, `hud_overlays.rs` |
| World rendering, depth/occlusion, batching | `gfx.rs`, `gfx/`, `camera.rs`, `game/render_control.rs` |
| Client movement/physics | `motion.rs`, `motion/`, gameplay dispatch in `game.rs` |
| Screens and inventory actions | `game/screens.rs`, inventory/container and interaction call sites found via `rg` |
| Remote client/network lifecycle | `game/remote.rs`, `game/remote_*`, `crates/horizon-protocol/src/` |
| Integrated server and LAN authority | `game/integrated.rs`, `game/lan_actions.rs`, `game/levels.rs` |

For example, `game/combat.rs` is substantially a damage/death tracker; its filename alone does not identify the attack entry point. Trace callers to the actual action and packet paths.

For every hook, record: stable internal ID, exact source path/symbol, injection kind, source selector/expected match count, bindings, thread/phase, priority, cancellation/return semantics, lifetime assumptions, reentrancy guard, pristine source hash and proving test.

Prefer a small number of shared hooks dispatching into the runtime. Broad OVERWRITE operations conflict with other mods and make source updates fragile; use them only where narrower supported hooks cannot express the behavior and document why.

Validate target functions against mixin limits: closures/`?` preservation, async/generic/unsafe/const restrictions, ambiguous calls, macro expansion and temporary-drop semantics depend on hook kind/tool version. An unsupported required hook is a concrete upstream tooling issue to isolate, not a reason to bypass the mod architecture.

## 9. Implementation phases and gates

### Phase 0 — Specification and pin selection

1. Read local agent rules, inspect dirty state, record both reference commits.
2. Create the complete feature ledger and behavioral specs from static inspection, including defaults and excluded version branches.
3. Choose one Horizon commit. Prefer the user's local target if the mixin tool supports it; do not silently downgrade to the template's older demonstration pin.
4. Pin the mixin tool and Rust toolchain; document the selected game protocol/version.
5. Record near-1:1 UI/feature scope, approved online exclusions, modern interpretations of obsolete patches and the public update policy. Do not substitute UI modernization or a limited module subset.

Gate: all discovered modules have rows, every removal has a reason, and first hook targets resolve on the actual chosen source.

### Phase 1 — Reproducible native integration

1. Establish the standalone testable Rust workspace and source packaging approach.
2. Implement reproducible export → source-hash audit → plan → apply → compile. Keep all staged files/build outputs in this project's target directories.
3. Add one lifecycle/tick hook with visible local diagnostic output and a clean disable path.
4. Add one keybind/toggle and one simple real HUD overlay, using actual host state.
5. Produce a source-only `.hnmod` whose IDs/versions/pins agree with its inner manifests.
6. Test that originals are unchanged and repeated staging is deterministic.

Gate: actual pinned Horizon binary compiles with the bundle; the feature is reachable through installed hooks. Standalone compilation is insufficient. Keep the first runtime proof minimal and then expand; do not stop here.

### Phase 2 — Shared runtime and complete settings/UI foundation

Implement module lifecycle, typed settings, event dispatch, action arbitration, binds/macros, local profiles, local friend/enemy filters, notifications, GUI widgets and HUD editing.

Port the complete settings behavior, including conditional visibility and defaults, progressively with each feature. Use metadata-driven editors where practical. Recreate the interface near-1:1: layout, proportions, typography, spacing, colors, icons, animations, window/page behavior, tooltips, focus, scrolling, binds and HUD editor. Reimplement the rendering in Rust; do not redesign the product around Horizon's stock settings widgets.

Gate: configure/toggle/save/reload real modules in-game; profile changes and panic clean up active state; text entry and GUI focus do not leak gameplay input. Maintain reference UI specifications and repeatable screenshot comparisons across scales. Since the Java reference remains untrusted, derive initial references from source/static assets and user-supplied captures; do not execute it to capture screenshots without a separately suitable isolated environment. Call out visual aspects that remain unverified rather than declaring pixel parity from code inspection.

### Phase 3 — HUD and rendering

Start with coordinates/FPS/clock/keystrokes, then armor/potions/compass/reach display/scoreboard. Add entity overlays, world-space ESP/tracers/nametags, storage/search/trajectories and path previews. Finish camera/fullbright/fog/weather/time/blur/chams/XRay features.

Explicitly implement depth testing, through-wall behavior where intended, camera-relative precision, clipping, GUI scaling, host render ordering and resource lifetime. Do not carry over OpenGL state assumptions.

Gate: visual checks at multiple window sizes, GUI scales, camera distances and screen states; features survive resize/world changes and restore host rendering when disabled. Capture and inspect real rendered artifacts, not just mathematical draw lists.

### Phase 4 — Inventory, consumables, utilities and rescue

Implement inventory snapshot/transaction adapter first, then AutoTool and simpler item selection, followed by armor, refill/steal/clean/fill/sort/consumable modules, fishing/pearl/totem behavior and placement/rescue planners.

Use actual container IDs, state revisions/acknowledgements, cooldowns and remote-client action paths. Coalesce/serialize conflicting modules. Account for user inventory input while actions are pending.

Gate: controlled scenarios demonstrate intended effects and interruption recovery without dupes, lost items, stale slot selection or repeated actions.

### Phase 5 — Targeting, combat, movement and advanced state machines

Build shared target filters, visibility/raycast tests, rotation coordinator and click timing. Port simple modules first, then multi-stage behavior and modern-item modules. Implement all combat/movement seeds and their submodes, including retained non-AI adaptive paths where specified. SilentAura uses its retained PID rotation behavior; AI branches are excluded.

Modern combat cooldowns, shields, attack/use order and movement correction must match the pinned host. Old-version mechanics must have an explicit modern interpretation or a reviewed not-applicable entry. Do not simulate server acceptance by modifying only local display values.

Gate: deterministic algorithm tests plus actual host scenarios for targeting, action order, conflicts, disable cleanup and remote corrections. Test the combinations the original module-control layer coordinates.

### Phase 6 — Packet queues and timing-dependent behavior

Implement Blink/Backtrack/FakeLag and relevant movement/combat queue modes against the actual current packet model. Keep protocol codecs in Horizon; intercept typed boundaries where possible.

Bound memory and delay; define ordering, ownership, flush/drop rules, correction handling, disconnect behavior and world/session invalidation. Preserve connection-critical traffic. Avoid delaying shared server-side traffic accidentally.

Gate: local controlled client/server integration verifies ordering and corrections, queue cleanup, reconnects and behavior under loss/disconnection. Unit-testing a vector of packets is not enough.

### Phase 7 — Remaining parity and removal audit

Audit every ledger row, not only the module seeds. Complete specialized world modes, subsettings, macros, translations, performance views and retained non-AI behavior. Finish the near-1:1 visual and interaction pass; simpler interim UI must not become the final implementation by accident.

Verify that no account/login/token bootstrap, online pages, cloud/public profile calls, reviews, parties, shared activity or inventory service, remote synchronization or hidden localhost service remains. Preserve local profiles and friend/enemy filters. Do not confuse the removed online product with packet features operating through Horizon's existing game connection.

Gate: every retained row is validated or has an explicit accepted modernization decision. Online-only and AI-only rows are `excluded-approved`, not pending decisions or unfinished optional functionality. No additional module exclusions are inferred from implementation difficulty.

### Phase 8 — Release and maintenance

Package clean source bundles, provide build/install/update instructions, publish compatibility pins and test reports, audit artifacts for Java/C/C++ payloads and unintended data/network behavior. Validate installation through the actual native-mod path in a fresh instance.

Provide a reproducible upgrade process: select new pin, compare hook target source, regenerate plan, fail loudly on drift, compile, rerun relevant tests, update compatibility metadata. Supporting successive versions does not require shipping concurrent multi-version compatibility code. Each published version pins a tested revision pair; newer mod releases advance those pins.

Public distribution and Horizon compatibility tracking are requirements from the first integration milestone, not polish to postpone until all modules exist. Establish CI for standalone tests, emitted-payload compilation and source drift detection early. Maintain a machine-readable compatibility matrix mapping mod releases to Horizon/game/mixin revisions, tested platforms and artifact hashes. Test on the host platforms supported by the intended Horizon release; report unvalidated platforms honestly and resolve packaging differences before claiming support.

Set up an upstream-update check that detects new target Horizon revisions, attempts staging/compilation and runs regression scenarios, then produces a reviewable update result. Never auto-publish an untested build, silently move an existing release's pin or mutate users' installations. The exact tracking channel/cadence (stable releases, Nightlies or every main commit) remains to be selected; recommended initial policy is stable release compatibility plus continuous main-branch drift checks. This is a release-policy detail, not permission to freeze development permanently.

Offer clear compatibility errors, reproducible rebuild instructions, upgrade/rollback guidance and preservation of user settings. Source/native builds remain the supported delivery path because the user accepts rebuilding. Do not spend the initial phase inventing binary injection or a dynamic plugin ABI to avoid compilation.

## 10. Testing and evidence

Use layers that prove different things:

1. Pure Rust tests: settings validation, deterministic timing, target selection, movement math, planners, queue ordering and state-machine cleanup.
2. Hook fixtures: original body executes once, returns stay intact, cancellation is honored, locals/arguments have expected values, module payload compiles and multiple mods have deterministic ordering.
3. Pinned-source audits: validate exact selectors/source hashes, transformed AST scope, pristine checkout preservation and repeatable staging.
4. Real host builds: compile the actual client binary with emitted runtime/module payload and the exact lockfile. A plan/check of an isolated toy fixture cannot substitute for this.
5. Scenario tests: fresh worlds and local controlled servers; default integrated server and LAN/remote client. Single player is supported and tested only on Horizon's default integrated server thread (user decision, docs/decisions.md). Client modules must run once, on the client, even if the host has another `Game` instance for its server thread. Single-player and multiplayer are both required: remote authority, acknowledgements/corrections and integrated-server threading differ technically even though the user-facing mod is the same. Test the relevant host paths without treating either mode as an optional scope choice.
6. Visual checks: real HUD, GUI and world-render captures; verify enabled and restored states. Use available display/capture tooling according to the session's tool rules.
7. Performance checks: mod disabled, runtime enabled with all features off, representative overlays, dense entity scenes, many enabled modules and worst-case queue/planner loads. Bound work and allocations per tick/frame, and report measured differences.

Standard standalone commands after the workspace exists:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
```

Populate approved dependencies before using offline builds. New native tooling commands must be documented accurately; do not copy the template's Python commands if implementing Rust tooling instead. Keep builds/tests scoped to the current change, expanding when failures or new risks justify it.

For host staging, use the pinned mixin tool's `plan` and `apply`, then compile/test the staged source. Its `check` and the launcher native-build path carry explicit native-code trust requirements: read and honor current instructions before executing them. The planning task did not authorize running unreviewed third-party mod helpers.

## 11. Security, provenance and operating constraints

The Java checkout was initially suspected of being an infostealer. Static review found no confirmed credential-stealing path, but found process injection, hidden extraction, unverified adjacent DLL selection, permissive TLS checks, configurable activity sharing and recursive application-directory cleanup. Do not transplant those behaviors into the Rust design. No Java project code, build scripts or installers were executed during this analysis.

- Continue treating the reference as untrusted data. Read source; do not run its Gradle wrapper, Java code, native loader or repository tools.
- Newly authored Rust tests are appropriate; inspect any borrowed build scripts before running them. Native mod code is unrestricted code.
- Use fresh scratch game/launcher homes for every executable test. Follow `../horizon/AGENTS.md`: isolate `HORIZON_HOME`, `HORIZON_LAUNCHER_HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`; set `HORIZON_LAUNCHER_NO_SELF_UPDATE=1` and `HORIZON_NO_AUDIO=1`. If launching the launcher, also isolate its trash, clipboard and desktop paths.
- Never test against real player worlds, accounts or live third-party servers. Network feature tests belong on controlled local test servers.
- Do not read browser profiles, credential stores or unrelated home-directory files. No telemetry or automatic external requests in the core mod.
- Keep diagnostics local, redact tokens and avoid logging private server information unnecessarily.
- Do not bundle Mojang game files, decompiled sources or prepared resources. Players supply compatible resources.

Horizon core contribution rules prohibit translating other mods into its own codebase. Keep this rewrite in its independent repository; do not submit copied/translated Vape feature code as Horizon core contributions. If a generic host/tooling change is required, isolate and implement it according to upstream rules.

The reference root has a CC0 document, but that does not by itself establish the provenance of reconstructed code, branding, fonts or textures. Track sources/attribution, inspect third-party notices and resolve redistribution rights before publication. Registry policy specifically requires permission/licensing for ports. This is a release requirement, not a reason to leave technical exploration unfinished.

## 12. Agent execution protocol and definition of done

At the start of each work session:

1. Read this plan and any local `AGENTS.md`; inspect status without discarding edits.
2. Read `docs/features.csv` and progress notes. Continue the earliest unfinished dependency rather than restarting the scaffold.
3. Select a concrete vertical slice with specification, pure logic, host adapter, tests and an observable result.
4. Verify hooks and dependencies against the recorded pins. Keep reference source reads bounded and use `rg` first.
5. Implement, validate and update the ledger with actual commands/results and remaining limits.

Record decisions in `docs/decisions.md` and concise handoffs in `docs/progress.md`: changed files, completed feature IDs, tests, staged-source pin, failures/blockers and next step. Do not mark blocked work as complete or hide unavailable resources behind a simulated host.

Avoid a large generated list of dummy module implementations. Implement shared mechanisms and working slices, then expand systematically until all rows close. Preserve failures/unsupported cases as explicit diagnostics rather than pretending settings have effects.

Final completion criteria:

- All retained modules, submodes and cross-cutting features are validated in the ledger; exclusions are explicit and justified. UI/visual fidelity is near-1:1, not merely equivalent functionality in a redesigned interface.
- No Java/JVM/Gradle, injector, C/C++ loader, native dynamic-library injection, runtime mapping database or concurrent-version dispatch remains in the distribution.
- A pinned source-only Horizon native bundle installs/builds through its supported route and works without a Vape account/service for local features.
- Actual host compilation, gameplay scenarios, rendering checks and selected interaction tests pass with recorded evidence.
- Disabled features restore host behavior; state is isolated to the correct client/world/session; disconnects and corrections cannot leak queued actions into the next session.
- Runtime performance is measured and bounded; release artifacts contain only intended mod sources/assets and required notices.
- No online product/backend dependency remains. Known limitations, platform coverage and compatibility pins are documented honestly, with an operational process for keeping public releases compatible with Horizon updates.

The next implementation action is Phase 0: create the exhaustive ledger and verify a first hook against the chosen Horizon pin. The first engineering milestone is Phase 1's real tick/keybind/HUD slice, followed by the complete rewrite—not a declaration of completion after a prototype.
