# Current implementation handoff

Updated 2026-10-29 after the parallel implementation round (combat/movement,
render, inventory and HUD/UI workers) was merged into main. **The full plan is
not complete.** AI behavior and the online product are excluded; all other
retained scope remains open as listed below.

## Connected native implementation

The runtime registers 23 modules, all with normal local activation, binds,
profile persistence and GUI access:

| Area | Modules |
| --- | --- |
| HUD | Coordinates, FPS, Compass, Clock, Keystrokes/CPS, Armor Status, Potion Status |
| Render | NoHurtCam, Fullbright, Clear Water, StorageESP (partial: chest/furnace outlines) |
| Inventory | AutoTool, AutoArmor, AutoTotem (partial), Refill, ChestSteal |
| World / movement | FastPlace, AutoFish, NoJumpDelay, NoClickDelay, Sprint (baseline), Parkour (experimental) |
| Utility | Panic (configurable bind, optional re-enable) |

The local foundation includes typed settings/editors, chord/held bindings,
local profiles and friends/enemies, text/clipboard/IME editing, local macro
creation/binding/delays, and HUD drag/snap/scale editing. Each Game owns its
runtime. Session generations and hotbar/use/container claims guard transient
work. Native effects use normal host input/menu/chat paths. Biome and potion
HUD names use the active Horizon language when a resource translation exists.

Unconnected groundwork present in source (no module uses it yet):

- Targeting: candidate filtering (friend/enemy/team/visibility), distance/yaw/
  armor/threat/health sorting and bounded rotation stepping
  ([targeting-foundation.json](evidence/targeting-foundation.json)).
- Clicker timing: Normal/Extra click-interval primitives in
  `src/modules/combat/click_timing.rs` (unit-tested only).
- World render: world-space line/box primitives and bounded projectile
  prediction with caller-supplied collision (`src/modules/render/world.rs`).
- Movement: an initial movement-request resolver used by Sprint; not yet a
  general multi-input arbiter.

## Evidence and limits

- Integrated main (after all merges): **67 runtime tests + 4 audit tests** and
  67 emitted-payload tests pass; formatting and Clippy (all targets, warnings
  denied) pass; exact native stage/audit passes with **23 HEAD hooks, one owned
  state and 11 support modules**; the pinned client cargo check passes. See
  [parallel-round-integration.json](evidence/parallel-round-integration.json).
- **StorageESP** is the only module from this round with resource-backed host
  scenarios: fresh isolated integrated-server and in-frame runs showed a chest
  outline, and the integrated-server capture confirmed it clears when disabled.
- Sprint, Parkour, AutoTotem, Panic, the Refill fixes and localization have
  compile/unit evidence only. No ledger row is `validated` from compilation.
- Earlier resource-backed foundation/GUI/lifecycle/HUD/NoHurtCam/AutoTool
  scenarios remain the other host evidence; they are specific slices, not full
  reference parity. [hooks.md](hooks.md) lists every current target and module.
- Ledger: 34 implemented-core, 13 excluded-approved, 2 not-applicable-reviewed
  (MouseDelayFix, scoreboard predicate; see [hud-triage.md](specs/hud-triage.md)),
  423 not-started. 0 validated.

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

## Open dependencies and next work

Per area, in dependency order. None of these are blocked externally except
where stated; they are unfinished implementation.

1. **Combat / targeting:** live entity snapshots (position, box, health,
   armor, team, line of sight), reference score calculation, a single-writer
   rotation owner through the host look path, and attack/use scheduling on top
   of the existing click timing. Then AimAssist, TriggerBot, Left/RightClicker,
   HitSelect, BlockHit, Reach, Hitboxes, KillAura (non-AI), SilentAura PID,
   AntiBot and the remaining combat rows.
2. **Movement / packets:** complete shared movement-input arbitration, then
   SafeWalk, KeepSprint, WTap, NoSlowdown, AutoLadder; an inbound/outbound
   packet hook for Velocity, JumpReset, KnockbackDelay, Blink, FakeLag,
   Backtrack; then NoFall, Speed, Fly, Timer, Scaffold modes, Clutch, MLG,
   Freecam and the remaining world/utility rows. Parkour needs host scenarios.
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
   inventory-free operation, and needs a survival scenario.
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
8. **Distribution:** launcher/public packaging is gated until a published mixin
   pin includes the required lifetime-state patch. Cross-platform builds,
   installation/provenance and release gates remain open.

## Reproduction and resources

Horizon stays pinned to `e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083`;
Minecraft 26.3 / protocol 777. Mixin PR #2 base is
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

The current client was rebuilt successfully for manual testing; evidence:
[manual-playtest-build.json](evidence/manual-playtest-build.json). No game was
launched as part of that build. This development build disables audio/controllers.

Run `python3 tools/play.py` from the project root; rebuild after source changes with
`cargo run -p xtask --locked --offline -- build`. It opens the visible native
client in a fresh disposable survival instance under target/scenarios/manual-*.
The default prepared resources are the read-only path above; a positional path
overrides them. `--in-frame` selects the second host path; `--creative` starts
creative (inventory automation requires survival). Quit the window normally;
the test instance and game.log are retained. No normal player worlds/settings
are used. Right Shift opens the GUI, F8/F9 toggle coordinates/FPS and F10 is panic.

GUI/profile/HUD editing, AutoTool and the partial StorageESP have real host
evidence. Treat the newer inventory, macro, fishing, Sprint, Parkour, AutoTotem
and Panic features as experimental until their resource-backed validation pass. BlockOverlay and the rest of the unfinished
plan are not available by enabling hidden options.
