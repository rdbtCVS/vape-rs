# Current implementation handoff

Updated 2026-10-29 after the combat/movement round: live entity snapshots,
the reference target filter, AimAssist (Simple), TriggerBot, SafeWalk and
Velocity, each with fresh-home integrated and in-frame host scenarios.
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
| Combat | AimAssist (Simple mode; Adaptive open), TriggerBot, Velocity |
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

- Current tree: **102 runtime tests + 4 audit tests** and 102 emitted-payload
  tests pass; fmt and Clippy (all targets, warnings denied) pass; exact
  native stage/audit passes with **29 HEAD hooks, one owned state and 13
  support modules**; host check and fast build pass.
- **Combat/movement round** ([combat-movement-round.json](evidence/combat-movement-round.json),
  [aim-assist.json](evidence/aim-assist.json)), fresh isolated homes:
  AimAssist 14/14, TriggerBot 12/12, SafeWalk 13/13 and Velocity 10/10, each
  on both the integrated-server and in-frame paths; smoke regression 27/27.
  The scenarios found and fixed three bugs: TriggerBot reading the server's
  attack ticker, a shared reset that let a disabled AimAssist clear
  Velocity every frame, and in-frame knockback bypassing packets.
- **Post-merge scenarios** ([postmerge-validation.json](evidence/postmerge-validation.json)):
  integrated and in-frame 27/27, controlled LAN 16/16, covering Sprint,
  Parkour, AutoArmor, AutoTotem, Panic, death/respawn and reconnect.
- Not covered: the new combat modules against real players, LAN guests or
  remote servers; window focus loss; real keyboard rebinding events;
  switching between two worlds; third-party servers.
- StorageESP, AutoTool, GUI/profile/HUD editing, lifecycle and NoHurtCam keep
  their earlier scenarios. Refill, ChestSteal, AutoFish, FastPlace, Fullbright,
  Clear Water, the delay removals and localization still have compile/unit
  evidence only.
- Ledger: 13 integrated, 35 implemented-core, 13 excluded-approved, 2
  not-applicable-reviewed, 409 not-started, 0 validated
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
| `725d322` | Velocity in-frame knockback/explosion hooks; handoff refresh |

## Open dependencies and next work

Per area, in dependency order. None of these are blocked externally except
where stated; they are unfinished implementation.

**Round status.** Worker A's foundation (entity snapshots, target filter,
host `Entity.turn`) is done, with AimAssist and TriggerBot as consumers.
Worker B has an observe-only inbound boundary: Velocity works, but Blink,
FakeLag and Backtrack need a cancellable inbound or outbound point.
`handle_packet` has macros, so look for a macro-free dispatch or a mixin-tool
change. Worker C has SafeWalk on the movement writer; KeepSprint, WTap,
NoSlowdown and AutoLadder still need writer priorities. Worker D (camera,
projection, world text) hasn't started.

**Drafted, not registered:** `docs/drafts/clicker.rs` holds a port of
the reference click engine: Normal/Extra/Extra+ timing, the hold/release
cycle, jitter and break blocks, with unit tests written but never compiled.
Next steps: move it to `src/modules/combat/`, register it, add a physical attack-held flag in `Runtime::input`,
add an adapter (press = `attack_once` + `game.left = true`, release =
`game.left = false`), and write a host scenario for AutoClicker and then
RightClicker.

1. **Combat / targeting:** AimAssist Adaptive mode; AutoClicker and
   RightClicker (engine drafted); HitSelect, BlockHit, Reach, Hitboxes,
   KillAura (non-AI), SilentAura PID, AntiBot (teammate/bot checks the filter
   already calls), HitSwap/AutoMace (TriggerBot's cooldown bypass) and the
   remaining combat rows. A shared rotation owner is needed once more than
   AimAssist turns the view.
2. **Movement / packets:** movement-writer priorities, then KeepSprint, WTap,
   NoSlowdown, AutoLadder; a cancellable inbound/outbound packet boundary for
   JumpReset, KnockbackDelay, Blink, FakeLag, Backtrack; then NoFall, Speed, Fly, Timer, Scaffold modes, Clutch, MLG,
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
