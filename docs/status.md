# Current implementation handoff

Updated 2026-10-30 after the clicker/Reach/blatant-movement batch and the
interface-font GUI text. Before that came the classic V4 click-GUI remake;
the round before
added live entity snapshots, the reference target filter, AimAssist (Simple),
TriggerBot, SafeWalk and Velocity, each with fresh-home integrated-server
host scenarios. Single
player is supported and tested only on Horizon's integrated server thread.
**The full plan is not complete.** AI behavior and the online product are
excluded; all other retained scope remains open as listed below.

## Connected native implementation

The runtime registers 43 modules, all with normal local activation, binds,
profile persistence and GUI access. The sixteen newest (marked †) are
compile-only: no game scenario has run for them yet.

| Area | Modules |
| --- | --- |
| HUD (legit, HUD group) | Coordinates, FPS, Compass, Clock, Keystrokes/CPS, Armor Status, Potion Status |
| Game (legit, Game group) | NoHurtCam, Clear Water, NoJumpDelay, NoClickDelay, Time Changer†, Weather† |
| Render | Fullbright, StorageESP (partial: chest/furnace outlines) |
| Inventory | AutoTool, AutoArmor, AutoTotem (partial), Refill, ChestSteal |
| World / movement | FastPlace, AutoFish, Sprint (baseline), Parkour (experimental), SafeWalk, Anti-AFK† |
| Combat | AimAssist (Simple mode; Adaptive open), TriggerBot, Velocity, AutoClicker†, RightClicker†, Reach† (Normal/Advanced; Misplace open), WTap† |
| Other (exact-name search only) | HitBoxes†, Timer†, Fly†, NoFall† (Normal/AntiCheat), Step† (step height only), LongJump†, AntiFall† |
| Hidden (never listed) | NoSlowdown† (sprint re-enable open), Speed† (Bhop/AntiCheat B) |
| Utility | Panic (configurable bind, optional re-enable) |

The local foundation includes typed settings/editors, chord/held bindings,
local profiles and friends/enemies, text/clipboard/IME editing, local macro
creation/binding/delays, and HUD drag/snap/scale editing. Each Game owns its
runtime. Session generations and hotbar/use/container claims guard transient
work. Native effects use normal host input/menu/chat paths. Biome and potion
HUD names use the active Horizon language when a resource translation exists.

The click GUI follows the reference's classic V4 layout
([classic-gui-validation.json](evidence/classic-gui-validation.json)). A
"VAPE V4" navigation frame has search, settings and save icons; category
rows; a MISC section (Friends, Profiles with the current profile, Macros);
and Favorites/HUD-editor footer icons. Each category, Favorites, Profiles,
Macros, Friends and client Settings is its own 110-unit draggable,
collapsible, scrolling frame. Frames stack under the shortest column and
persist in `ui.frames`. Module rows toggle on click, and their dots expand
inline settings and a favourite star. HUD and Game legit modules are listed in the
HUD editor's frame under its Favorite/All/HUD/Game tabs. Pixel icons are authored rather than copied from the
reference images. All GUI text uses Horizon's bundled interface font (Geist)
rasterized at its on-screen size; the GUI scale setting applies when its
slider is released. Not ported: the Text GUI overlay, notifications and the
online PUBLIC profiles (replaced by a local IMPORT).

Unconnected groundwork present in source (no module uses it yet):

- World render: world-space line/box primitives and bounded projectile
  prediction with caller-supplied collision (`src/modules/render/world.rs`).
- Movement: `core::movement_input::Writer` is now the single writer of
  synthetic movement keys (Sprint and Parkour use it). Competing-module
  priorities for SafeWalk/KeepSprint/WTap and analog controller movement are
  still open.

## Evidence and limits

- Current tree: **108 runtime lib tests** pass; fmt and Clippy (all
  targets, warnings denied) pass; exact native stage/audit passes with **33
  hooks (32 HEAD + 1 MODIFY_ARG) and 18 support modules (no owned state)** on
  published horizon-mixins `07dab17`; host check and fast build pass. The
  emitted-payload and audit test suites were not rerun for the latest batch.
- **Clickers/Reach/blatant batch**
  ([clickers-reach-blatant-compile.json](evidence/clickers-reach-blatant-compile.json)):
  compile-only. **Interface-font GUI text** has one fresh-home screenshot
  check; the existing GUI scenarios were not rerun against it.
- **Packaging** ([hnmod-packaging.json](evidence/hnmod-packaging.json)):
  `tools/bundle.py` emits `dist/vape_rs-0.1.0-dev.hnmod`, and Horizon's registry
  check accepts it. `tools/launcher_build.py` builds it along the launcher's
  route (published tool via `cargo install --git`, GitHub source archive,
  release profile with `mods`, Rust 1.99.1). Smoke passed 27/27 on that
  executable. Not yet run through an actual `horizon-launcher native-build`
  install.
- **Combat/movement round** ([combat-movement-round.json](evidence/combat-movement-round.json),
  [aim-assist.json](evidence/aim-assist.json)), fresh isolated homes:
  AimAssist 14/14, TriggerBot 12/12, SafeWalk 13/13 and Velocity 10/10, each
  on the integrated server; smoke regression 27/27.
  The scenarios found and fixed two bugs: TriggerBot reading the server's attack ticker, and a shared reset that let a disabled AimAssist clear Velocity every frame. They also showed that this server sends the TNT push as a motion packet, so the explosion-packet hook is still unexercised.
- **Post-merge scenarios** ([postmerge-validation.json](evidence/postmerge-validation.json)):
  integrated 27/27, controlled LAN 16/16, covering Sprint,
  Parkour, AutoArmor, AutoTotem, Panic, death/respawn and reconnect.
- Not covered: the new combat modules against real players, LAN guests or
  remote servers; window focus loss; real keyboard rebinding events;
  switching between two worlds; third-party servers.
- **Re-pin to main** ([repin-main.json](evidence/repin-main.json)): every
  scenario was rerun. The GUI, HUD editor, HUD detail and text editor
  scenarios failed on stale click coordinates. They were rewritten for the
  classic GUI and pass ([classic-gui-validation.json](evidence/classic-gui-validation.json)).
- StorageESP, AutoTool, profile, lifecycle and NoHurtCam pass on the new pin. Refill, ChestSteal, AutoFish, FastPlace, Fullbright,
  Clear Water, the delay removals and localization still have compile/unit
  evidence only.
- Ledger: 16 integrated, 48 implemented-core, 13 excluded-approved, 2
  not-applicable-reviewed, 393 not-started, 0 validated
  ([decisions](decisions.md) explains `integrated` vs `validated`).
- [hooks.md](hooks.md) lists every current target and module.

Concrete implementation milestones:

| Commit | Change |
| --- | --- |
| `4d7e729` | Approved AI exclusion, retaining SilentAura PID |
| `2f0b433` | AutoTool with real native mining/selection evidence |
| `a4a9b6f` | Typed dropdown/slider/range/color/pointer editors |
| `3509cca` | Four HUD widgets, local macros and native inventory automation |
| `90a9f78` | FastPlace, Fullbright and inventory metadata safeguards |
| `17371d2` | AutoFish, Clear Water, shared use/container claims and input delays |
| `3f0462a` | Safe macro restoration and documentation handoff |
| `c30f977` | Pure targeting/rotation primitives |
| `932511a` | Sprint connected to native movement input |
| `844d72e` | World-render line/box/projectile primitives |
| `1a43a3f` | Partial StorageESP with host scenarios |
| `d951a3e` | Refill allow-list/scatter/cursor fixes |
| `a013a23` | Partial AutoTotem |
| `82a0c68` | Configurable Panic restoration |
| `fdb0ff6` | Localized biome/potion HUD names |
| `340eb44` | MouseDelayFix / scoreboard predicate triage |
| `60c6775` | Clicker Normal/Extra timing primitives |
| `bcd854f` | Experimental Parkour and movement-request resolver |
| (round) | Re-pin to Nightly 9669600, bounded inventory gate, movement writer, build identity, post-merge + LAN scenarios, ledger kinds, registry/GUI split |
| `d146567` | Live entity snapshots, reference target filter, item-limit groups, AimAssist (Simple) |
| `ce3dca4` | SafeWalk, TriggerBot, Velocity (server-thread path); per-module combat resets |
| `725d322` | Handoff refresh (its extra Velocity hooks were later removed with the single-player mode decision) |
| `530b106` | Classic V4 click GUI |
| `f73ca64` | AutoClicker, RightClicker, Reach, HitBoxes, Timer, Fly, NoFall, NoSlowdown (compile-only); MODIFY_ARG audit support |
| `951a729` | GUI text in the interface font; GUI scale applied on release |

## Open dependencies and next work

Per area, in dependency order. None of these are blocked externally except
where stated; they are unfinished implementation.

**Round status.** Worker A's foundation (entity snapshots, target filter,
host `Entity.turn`) is done, with AimAssist and TriggerBot as consumers.
Worker B has an observe-only inbound boundary: Velocity works, but Blink,
FakeLag and Backtrack need a cancellable inbound or outbound point.
`handle_packet` has macros, so look for a macro-free dispatch or a mixin-tool
change. Worker C has SafeWalk on the movement writer; KeepSprint, WTap,
NoSlowdown and AutoLadder still need writer priorities (WTap and
Anti-AFK now request through the writer). Worker D (camera,
projection, world text) hasn't started.

**Validation debt first.** The compile-only (†) modules above each need a
fresh-home integrated-server scenario (clicker CPS and hold/release, Reach
range, HitBoxes expansion, Timer rate, Fly motion, NoFall status packets,
NoSlowdown item-use speed, Speed/LongJump motion, AntiFall lagback, WTap,
Anti-AFK, Step, Time Changer, Weather), and the GUI scenarios should be rerun against
the interface-font text.

1. **Combat / targeting:** AimAssist Adaptive mode; Reach Misplace,
   HitBoxes interact-at clamp, RightClicker use claim/offhand whitelist and
   Reach/HitBoxes for LAN guests; HitSelect, BlockHit,
   KillAura (non-AI), SilentAura PID, AntiBot (teammate/bot checks the filter
   already calls), HitSwap/AutoMace (TriggerBot's cooldown bypass) and the
   remaining combat rows. A shared rotation owner is needed once more than
   AimAssist turns the view.
2. **Movement / packets:** KeepSprint (no client-side slowdown on Horizon's
   remote attack path to cancel), NoSlowdown's sprint re-enable, AutoLadder,
   Strafe (needs AimAssist's current target exposed), Step's position packets
   and timer slowdown, LongJump's vine/ice factors; a cancellable inbound/outbound packet boundary for
   JumpReset, KnockbackDelay, Blink, FakeLag, Backtrack; then NoFall's jump-key detail, Scaffold modes, Clutch, MLG,
   Freecam and the remaining world/utility rows. Parkour has an integrated-server scenario; a LAN case is still open.
3. **Render:** connect camera/projection, world text/quads, entity snapshots
   and collision queries; finish StorageESP (fill, through-wall, all container
   types); then ESP (3D/2D/outline/skeleton), Tracers, Nametags, ItemESP,
   Search, SpawnerFinder, Chams, Xray, Trajectories, Projectiles, Arrows,
   Indicators, Bedplates, Explosions. **BlockOverlay** transparent fill still
   needs an opaque-body-safe geometry insertion capability in the mixin tool or
   an explicit host geometry hook ([block-overlay.md](specs/block-overlay.md)).
4. **Inventory / consumables:** InventoryManager rule model and editor frames,
   InvCleaner, AutoHotbar, InventoryFill, custom item-name/potion metadata,
   ThrowPot, ThrowDebuff, AntiDebuff, AutoHeal, ArmorSwitch, Refill pots/soup
   modes and double-click, InvWalk, NoItemRelease. AutoTotem still lacks silent
   opening, random slot, extra timing variance, count display and
   inventory-free operation. Refill/ChestSteal/AutoFish need host scenarios
   for the bounded-gate recovery.
5. **HUD / UI:** ClientSettings (floating/integrated/none), Text GUI and module
   list, ModuleDisplayInfo; the HUD-module family (Freelook, Reach Display, Scoreboard, Inventory Blur, Motion Blur, Block Hit
   Animation, Block Render Color Override, Health); triage of the remaining
   "inherited/HUD; review" ledger rows; typography, icons, client translations
   and Windows date conventions. Panic default visibility needs verification.
6. **Other fidelity:** remote AutoFish ground-state observation, creative
   inventory protocol support, precise inventory scoring parity and
   shader-pack brightness inputs.
7. **Validation after implementation:** fresh isolated integrated-server scenarios for every compile-only module, then remote/LAN cases,
   visual comparisons, interruption/cleanup checks and performance.
8. **Distribution:** the `.hnmod` builds on the published tool, with a
   per-thread runtime owner (docs/decisions.md, 2026-10-29). Still open: a run
   through an actual installed `horizon-launcher native-build`, Windows/macOS
   builds, a tagged Horizon pin for a public release, provenance, the release
   gates and registry submission.

## Reproduction and resources

Horizon is pinned to `cc7bcbf4e508c1484dd4ae14741d78a8fbe745d1` (main at 2026-10-29 22:01, untagged); Minecraft 26.3 / protocol 777. horizon-mixins is
published main `07dab17387480ac03829584d9d1ee9912c9bb515`, unpatched. See compatibility.json and README.md for commands.

Prepared resource directory located for this workspace:

```text
<launcher-dir>/versions/nightly-2026-10-29-a305d38/.local/horizon-client/26.3
```

Use those resources read-only and tools/native_session.py with fresh homes and
working directories for every game execution. No assets, player worlds,
credentials or backend data are committed. Neither reference is built/executed
as Java. Horizon exports/builds stay under target; unrelated changes now present
in the adjacent Horizon checkout are not used or overwritten.

## Trying the development build

`xtask build` records the build identity beside the executable
(`target/host-cargo/fast/vape-rs-build.json`). `play.py` and the scenario tools
refuse to launch an executable that wasn't built from the current sources and
pins (`--allow-stale` overrides with a warning). The last recorded build is the
one in [postmerge-validation.json](evidence/postmerge-validation.json). This
development build disables audio/controllers; the virtual control-socket pad
still works.

Run `python3 tools/play.py` from the project root; rebuild after source changes with
`cargo run -p xtask --locked --offline -- build`. It opens the visible native
client in a fresh disposable survival instance under target/scenarios/manual-*.
The default prepared resources are the read-only path above; a positional path
overrides them. `--creative` starts
creative (inventory automation requires survival). Quit the window normally;
the test instance and game.log are retained. No normal player worlds/settings
are used. Right Shift opens the GUI, F8/F9 toggle coordinates/FPS and F10 is panic.

GUI/profile/HUD editing, AutoTool, the partial StorageESP, Sprint, Parkour,
AutoArmor, AutoTotem and Panic have real host evidence. Treat Refill,
ChestSteal, macros and fishing as experimental until their scenarios run. BlockOverlay and the rest of the unfinished
plan are not available by enabling hidden options.
