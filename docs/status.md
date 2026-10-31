# Current implementation handoff

Updated 2026-10-31 after the re-pin to Horizon main `8f91f89` and a
six-worker round (inventory, validation, combat, movement, render, HUD/UI),
all merged into main. Single player is supported and tested on Horizon's
integrated server thread; LAN host/guest scenarios cover the packet and
KeepSprint paths. **The full plan is not complete.** AI behavior and the
online product are excluded; all other retained scope remains open as listed
below.

## Connected native implementation

The runtime registers 83 modules, all with local activation, binds, profile
persistence and GUI access. † marks modules with no game scenario yet
(compile, unit tests and host check only).

| GUI category | Modules |
| --- | --- |
| Combat | AimAssist (Simple; Adaptive open), AutoClicker, RightClicker, TriggerBot, JumpReset, WTap, SilentAura (PID), Sprint, Reach (blocked on the host server's re-pick) |
| Other | KillAura, KeepSprint, InvWalk, HitBoxes, Timer, Fly, NoFall, Step, LongJump†(vine/ice factors), AntiFall (partly observable), NoItemRelease, PotionSaver, Regen (effect not observable on this host) |
| Hidden | AutoHeal (Legit only), NoSlowdown, Speed (Bhop/AntiCheat B) |
| Utility | Panic, Velocity, KnockbackDelay, Blink, Backtrack, FakeLag (Repel tested), Scaffold (Legit only), WindCharge |
| World | AutoTool, AutoFish, ChestSteal, FastPlace, SafeWalk, Parkour (experimental), Anti-AFK, MurdererFinder† (callout blocked), PropHunt† |
| Inventory | AutoArmor, AutoTotem (no silent open), Refill, Throwpot, ThrowDebuff, InventoryFill†, InventoryManager†, InvCleaner†, AutoHotbar†, ArmorSwitch† |
| Render | Fullbright, Clear Water, Time Changer, Weather, AntiDebuff, ItemESP, NoHurtCam, NoClickDelay, NoJumpDelay, Inventory Blur, NameTags (animals tested; equipment row†), StorageESP (chests tested; fill/ender/open†), ESP†, Tracers†, Trajectories†, Search†, SpawnerFinder†, Health†, Arrows†, Explosions† |
| HUD | Coords, FPS, Compass, Clock, Keystrokes, Armor Status, Potion Effects, Reach Display, Scoreboard, Target Info, Text GUI (module list) |

Client: classic V4 click GUI ([classic-gui-validation.json](evidence/classic-gui-validation.json)),
ClientSettings search styles (floating/integrated/none, Ctrl+F), toggle
toasts and the GUI-bind indicator, local profiles, friends/enemies, macros,
HUD drag/snap/scale editing. Shared owners: `core::movement_input::Writer`
for synthetic movement keys, the combat rotation claim
(`modules/combat/rotation.rs`, rotation sent in the movement packet only),
the inventory click session and the bounded inventory gate. Movement keeps a
separate small rotation controller for WindCharge/Scaffold
(`modules/movement/rotation.rs`); unifying it with the combat claim is open.

## Evidence and limits

- Current tree (main `4390bc3`): fmt and Clippy (`-D warnings`) pass,
  **194 lib + 3 integration tests** and the payload test pass, stage/audit
  passes with **43 hooks and 29 support modules** on horizon-mixins
  `0e9229d`, host check passes (68 warnings, none fatal) and the fast build
  passes.
- Ledger: 64 validated, 32 integrated, 66 implemented-core, 6 blocked,
  13 excluded-approved, 4 not-applicable-reviewed, 287 not-started
  ([decisions](decisions.md) explains `integrated` vs `validated`).
- Scenario evidence this round (fresh isolated homes):
  - Inventory: [inventory-throw.json](evidence/inventory-throw.json), [inventory-recovery.json](evidence/inventory-recovery.json).
  - Validation debt: validation-movement/combat/pvp (LAN)/legit-render/hold (LAN)/utility/delays-perf `.json` in docs/evidence.
  - Combat: combat-aura.json 16/16, combat-keep-sprint.json (LAN) 13/13.
  - Movement: movement-world.json 28/28.
  - HUD: hud-target-info, hud-readouts, hud-inventory-blur, hud-scoreboard, hud-modes, hud-text-gui, hud-client-settings. These ran on the pre-merge HUD build `ca264f4`; rerun on the merged tree is pending.
  - Render: **none** — `tools/render_scenario.py` is committed but not run.
- Bugs found by scenarios and fixed: NoSlowdown hook target, NoItemRelease
  action id, JumpReset tick order, Time Changer restore, AntiDebuff speed
  modifier, Refill reopen after close, Step's extra position packets
  (kicked; dropped).
- Performance: 12 render/HUD modules raised average CPU/frame 1.75 → 4.11 ms
  and 1 % lows 5.5 → 40.5 ms (validation-delays-perf.json). Not profiled.
- Not covered: remote/third-party servers, window focus loss, real keyboard
  rebinding events, switching between two worlds.
- [hooks.md](hooks.md) lists every target and module.

## Open dependencies and next work

1. **Validation first:** run `tools/render_scenario.py`; rerun the HUD
   scenarios on the merged tree; scenarios for the inventory planners,
   InventoryFill, LongJump factors, NoFall jump release, FakeLag
   Latency/Dynamic, NameTags players/mobs, Parkour on LAN.
2. **Combat:** HitSelect (needs a cancellable hook on `remote_start_attack`;
   decoded constants recorded in progress.md), BlockHit, HitFlick, AntiBot,
   HitSwap/AutoMace, ShieldBreaker, CrystalAura, BowAimbot, AimAssist
   Adaptive, Reach Misplace; SilentAura Ring/Box render; rotation for the
   riding packet and third-person view.
3. **Movement / world:** Scaffold GodBridge/TellyBridge on the combat
   rotation claim; merge the two rotation controllers; Strafe, Freecam,
   AutoPearl, PearlCatch, MLG, Clutch, AutoLadder, BlockIn, AutoAnchor,
   BedBreaker, FakeLag Dynamic, Blink outgoing-only/fake player; InvWalk key
   byte and text-field check.
4. **Render:** Projectiles, Indicators, BedPlates, BlockOverlay
   ([block-overlay.md](specs/block-overlay.md)); Search per-block tracers,
   crossbow multishot, enchant glint, Explosions depth; Health's U+2764
   heart (the interface font lacks the glyph; the HUD worker's vanilla-font
   fix was dropped at merge).
5. **Inventory:** InventoryManager editor frames; custom item-name/potion
   metadata; AutoTotem count display on the Text GUI.
6. **HUD / UI:** HUD panels hidden while the click GUI is open; Target Info
   head preview, pot count, enchant-aware comparator; client translations,
   Windows date conventions; Panic default visibility.
7. **Blocked on Horizon:** Reach on the integrated/LAN server (server re-pick
   at 3 blocks), AutoTotem silent/inventory-free, AutoHeal Silent/Potions,
   ESP skeleton (no bone poses), Xray/Chams (mesher/entity-render hooks),
   Freelook (camera rotation), MotionBlur (frame accumulation),
   MurdererFinder callout (chat send).
8. **Distribution:** an actual installed `horizon-launcher native-build` run,
   Windows/macOS, a tagged Horizon pin, provenance, release gates, registry.

## Reproduction and resources

Horizon is pinned to `8f91f896a5d47d034318569940e775d8bd854bd6` (main at 2026-10-31 04:49, untagged); Minecraft 26.3 / protocol 777. horizon-mixins is
published main `0e9229d88d7b6ab18add048de680d873f04cd9b5` (0.6.0), unpatched. See compatibility.json and README.md for commands.

Prepared resource directory located for this workspace:

```text
<launcher-dir>/versions/<nightly>/.local/horizon-client/26.3
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

Modules with real host evidence are listed under "Evidence" above; treat the
rest (render modules, inventory planners, macros) as experimental until their
scenarios run. BlockOverlay and the rest of the unfinished plan are not
available by enabling hidden options.
