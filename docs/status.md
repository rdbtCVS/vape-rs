# Current implementation handoff

Updated 2026-10-29 after the post-merge validation round: Horizon re-pin,
bounded inventory transactions, a single movement-input writer, build
identities and the first resource-backed scenarios for the merged modules.
**The full plan is not complete.** AI behavior and the online product are
excluded; all other retained scope remains open as listed below.

## Connected native implementation

The runtime registers 27 modules, all with normal local activation, binds,
profile persistence and GUI access:

| Area | Modules |
| --- | --- |
| HUD | Coordinates, FPS, Compass, Clock, Keystrokes/CPS, Armor Status, Potion Status |
| Render | NoHurtCam, Fullbright, Clear Water, StorageESP (partial: chest/furnace outlines) |
| Inventory | AutoTool, AutoArmor, AutoTotem (partial), Refill, ChestSteal |
| World / movement | FastPlace, AutoFish, NoJumpDelay, NoClickDelay, Sprint (baseline), Parkour (experimental), SafeWalk |
| Combat | AimAssist (Simple mode; Adaptive open), TriggerBot, Velocity (in-frame path open) |
| Utility | Panic (configurable bind, optional re-enable) |

The local foundation includes typed settings/editors, chord/held bindings,
local profiles and friends/enemies, text/clipboard/IME editing, local macro
creation/binding/delays, and HUD drag/snap/scale editing. Each Game owns its
runtime. Session generations and hotbar/use/container claims guard transient
work. Native effects use normal host input/menu/chat paths. Biome and potion
HUD names use the active Horizon language when a resource translation exists.

Unconnected groundwork present in source (no module uses it yet):

- Targeting is now connected: `vape_combat::snapshot` provides live entity
  views and `targeting::filter` the reference target filter; AimAssist is
  the first consumer ([aim-assist.json](evidence/aim-assist.json)).
- Clicker timing: Normal/Extra click-interval primitives in
  `src/modules/combat/click_timing.rs` (unit-tested only).
- World render: world-space line/box primitives and bounded projectile
  prediction with caller-supplied collision (`src/modules/render/world.rs`).
- Movement: `core::movement_input::Writer` is now the single writer of
  synthetic movement keys (Sprint and Parkour use it). Competing-module
  priorities for SafeWalk/KeepSprint/WTap and analog controller movement are
  still open.

## Evidence and limits

- Current tree: **85 runtime tests + 4 audit tests** and 85 emitted-payload
  tests pass; formatting and Clippy (all targets, warnings denied) pass; exact
  native stage/audit passes with **23 HEAD hooks, one owned state and 11
  support modules** on the new pin; the optimized client was rebuilt and its
  build identity recorded.
- **Post-merge scenarios** on that exact build
  ([postmerge-validation.json](evidence/postmerge-validation.json)), fresh
  isolated homes each: integrated server 27/27, in-frame 27/27, controlled LAN
  (two owned processes on localhost) 16/16. Covered: Sprint, a controller-held
  sprint surviving Sprint disable, pause-menu release, Parkour edge jump,
  AutoArmor equip and full-inventory no-op recovery with upgrade once room
  appears, AutoTotem acting after that no-op, Panic disable/restore,
  death/respawn, and disconnect/reconnect with the server keeping the swapped
  items. The scenarios found and fixed an AutoArmor scoring bug (vanilla
  slot-group armor modifiers were ignored).
- Not covered: window focus loss (the socket can't unfocus the window), real
  keyboard rebinding events (controller and socket paths were covered),
  switching between two worlds, and third-party servers.
- StorageESP, AutoTool, GUI/profile/HUD editing, lifecycle and NoHurtCam keep
  their earlier scenarios. Refill, ChestSteal, AutoFish, FastPlace, Fullbright,
  Clear Water, the delay removals and localization still have compile/unit
  evidence only.
- Ledger: 5 integrated (Sprint, Parkour, AutoTotem, AutoArmor, Panic), 29
  implemented-core, 13 excluded-approved, 2 not-applicable-reviewed, 0
  validated. The new `kind` column splits the 423 not-started rows into 88
  modules, 159 submodes, 27 submodules and 5 client rows, plus 144 helper rows
  that are tracked under their owners ([decisions](decisions.md)).
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

## Open dependencies and next work

Per area, in dependency order. None of these are blocked externally except
where stated; they are unfinished implementation.

**Next parallel round: one foundation plus its first consumer per worker.**
Each worker delivers the shared piece, one observable module that uses it,
interruption cleanup and a fresh-home scenario. They add only a line to
`src/core/registry.rs` and their own files.

| Worker | Foundation | First consumer | Then unlocks |
| --- | --- | --- | --- |
| A | Live entity snapshots (id+session, box, health, armor, team, line of sight) shared with rendering, plus one rotation owner through the host look path | AimAssist | TriggerBot, clickers, KillAura, SilentAura PID, ESP/Tracers inputs |
| B | Inbound/outbound packet boundary hook (typed, bounded, session-invalidated) | Velocity | JumpReset, KnockbackDelay, Blink, FakeLag, Backtrack |
| C | Movement-writer priorities and remaining axes (sneak/strafe suppression, analog stick) | SafeWalk | KeepSprint, WTap, NoSlowdown, AutoLadder, Scaffold |
| D | Camera/projection plus world text/quads through the host renderer | Nametags | ESP 2D/3D, Tracers, StorageESP fill/through-wall, Trajectories |

Before that round: retarget `client-tick`/`client-frame`/`client-hud` onto the
stable hook points where order allows. Resolve the packaging owner decision
(user) in docs/decisions.md.

1. **Combat / targeting:** live entity snapshots (position, box, health,
   armor, team, line of sight), reference score calculation, a single-writer
   rotation owner through the host look path, and attack/use scheduling on top
   of the existing click timing. Then AimAssist, TriggerBot, Left/RightClicker,
   HitSelect, BlockHit, Reach, Hitboxes, KillAura (non-AI), SilentAura PID,
   AntiBot and the remaining combat rows. (Worker A above.)
2. **Movement / packets:** complete shared movement-input arbitration, then
   SafeWalk, KeepSprint, WTap, NoSlowdown, AutoLadder; an inbound/outbound
   packet hook for Velocity, JumpReset, KnockbackDelay, Blink, FakeLag,
   Backtrack; then NoFall, Speed, Fly, Timer, Scaffold modes, Clutch, MLG,
   Freecam and the remaining world/utility rows. Parkour has an integrated
   and in-frame scenario; a LAN case is still open.
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
   list, ModuleDisplayInfo; the HUD-module family (Freelook, Time/Weather
   Changer, Reach Display, Scoreboard, Inventory Blur, Motion Blur, Block Hit
   Animation, Block Render Color Override, Health); triage of the remaining
   "inherited/HUD; review" ledger rows; typography, icons, client translations
   and Windows date conventions. Panic default visibility needs verification.
6. **Other fidelity:** remote AutoFish ground-state observation, creative
   inventory protocol support, precise inventory scoring parity and
   shader-pack brightness inputs.
7. **Validation after implementation:** fresh isolated integrated-server and
   in-frame scenarios for every compile-only module, then remote/LAN cases,
   visual comparisons, interruption/cleanup checks and performance.
8. **Distribution:** published horizon-mixins rejects the Game-owned state
   and four macro-target hooks. A stock-tool variant type-checks without them
   ([repin-packaging-eval.json](evidence/repin-packaging-eval.json)). Choose the
   owner (user decision), retarget the four hooks, then prove an installed
   launcher build. Cross-platform builds, provenance and release gates remain
   open.

## Reproduction and resources

Horizon is pinned to `9669600c389ad6d205db4ec0bb061935f2a38d0c`
(`nightly-source/2026-10-28-9669600`); Minecraft 26.3 / protocol 777. Mixin PR #2 base is
`29a8f8f84b253ea89422c0049ff5e8bfda36f1fb` plus the hashed checked-in
lifetime-state patch. See compatibility.json and README.md for commands.

Prepared resource directory located for this workspace:

```text
<launcher-dir>/versions/nightly-2026-10-28-9b772b2/.local/horizon-client/26.3
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
overrides them. `--in-frame` selects the second host path; `--creative` starts
creative (inventory automation requires survival). Quit the window normally;
the test instance and game.log are retained. No normal player worlds/settings
are used. Right Shift opens the GUI, F8/F9 toggle coordinates/FPS and F10 is panic.

GUI/profile/HUD editing, AutoTool, the partial StorageESP, Sprint, Parkour,
AutoArmor, AutoTotem and Panic have real host evidence. Treat Refill,
ChestSteal, macros and fishing as experimental until their scenarios run. BlockOverlay and the rest of the unfinished
plan are not available by enabling hidden options.
