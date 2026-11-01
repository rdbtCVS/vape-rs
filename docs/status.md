# Current implementation handoff

Updated 2026-11-01 on main, after the worst-case load runs, the BackTrack
release-time and TargetInfo preview fixes (bundle `692a0f79`) and the wind-down
merge of the round 5/6 worker branches (progress.md 2026-11-01).
The round 2–4 workers (combat-a, combat-b, inventory, render,
inventory-utility-debt, movement-a, movement-b, client-foundation,
hud-client-debt) are merged with their ledger updates. The round 5
(combat-targeting, placement) and round 6 (invutil) worker edits are merged
unvalidated: they pass the gates but no host scenario was run, and the
ledger rows stay as they were. Round 6 Motion Blur stays on branch
`t3/9b567e4d` because its injection does not resolve against the pin.
Single player is tested on Horizon's integrated server thread; LAN host/guest
scenarios cover the packet, combat and movement paths that differ there.
**The full plan is not complete.** AI behavior and the online product are
excluded; all other retained scope is listed below as validated, in progress
or blocked.

## Ledger

docs/features.csv holds 489 rows: **219 validated**, 42 integrated,
40 implemented-core, 129 not-started, 9 blocked, 13 excluded-approved and
37 not-applicable-reviewed. Helper classes take their status from their
owning module ([decisions](decisions.md)); a review of all 97 helper rows
moved 19 of them on 2026-10-31, and most of the remaining not-started rows
are the 47 classes of the inventory-filter GUI editor, which is not ported. `validated` always means a
passing fresh-home host scenario with evidence in docs/evidence;
`integrated` means hooked and running in the host without a full scenario.

Scope (user decision, 2026-11-01): whether the reference's ModManager registers a
module is not a scope criterion, and the interface is a design rebuilt from the
source, not a clone of one image. The 17 module classes it never registers now
have rows:
- LongJump, AntiFall and Step are implemented-core (they exist, with no scenario yet).
- Strafe, Regen, PotionSaver, Knockback Test, Phase, the older DiscardedBlink, Block-In's "Simulation" renderer, Benchmark, OutgoingPackets and the Render Test Module are not-started.
- OmniSprint, CPSMod and SelfDestruct are not-applicable-reviewed: their reference bodies are empty, and SelfDestruct's unload is removed injection infrastructure.
- Anti-FML is not-applicable-reviewed: Horizon sends no Forge handshake to cancel.

Modules validated in a real game (83 module/client rows):
- **Combat:** AutoClicker, RightClicker, TriggerBot, JumpReset, WTap, Sprint, KeepSprint, Velocity, HitBoxes, BlockHit, HitSelect, HitSwap, ShieldBreaker, TargetFilter, SilentAura clicker.
- **Movement:** NoSlowdown, Speed, Fly, NoFall, InvWalk, Timer, SafeWalk, NoItemRelease, WindCharge, Freecam.
- **World / utility:** Anti-AFK, AutoFish, FastPlace, BackTrack, KnockbackDelay, Panic, AutoPearl, PearlCatch, AutoTool.
- **Inventory:** Refill, Throwpot, ThrowDebuff, AutoHeal, AutoTotem, ChestSteal, InventoryManager, InvCleaner, AutoHotbar, ArmorSwitch.
- **Render:** Fullbright, Clear Water, Time Changer, Weather, AntiDebuff, ItemESP, NoHurtCam, NoClickDelay, NoJumpDelay, Inventory Blur, ESP, Tracers, NameTags, Search, SpawnerFinder, StorageESP, Trajectories, Projectiles, Arrows, Health, Explosions, Indicators, BedPlates, Block Overlay, PropHunt, MurdererFinder.
- **HUD / client:** Armor Status, Potion Status, Coords, FPS, Compass, Clock, Keystrokes, Reach Display, Target Info, local toggle notifications, the GUI and config settings modules.

Integrated, waiting for a full scenario on main: AimAssist, KillAura,
SilentAura, AutoArmor, AutoMace (Manual/Auto/Density/Breach modes
validated), InventoryFill, Parkour, Blink, FakeLag, Scaffold (Legit
validated; GodBridge 23/24 and TellyBridge 14/16 on the worker branch),
Freelook, Scoreboard (text replacement), Text GUI, the click GUI
layout/search/favorites, friends/enemies, local profiles, macros, binds
and the deterministic timing.

Implemented core (logic and tests, host path incomplete): AntiFireball,
HitFlick, the shared claim owners, session lifecycle, HUD drag/snap/scale and
the typed setting editors.

Not started: CrystalAura and its submodules, AutoAnchor, AutoLadder,
BedBreaker, Block-In, Clutch, MLG, MotionBlur (it has hook targets now, see
decisions.md), the InventoryManager filter-rule editor panel, and the
translations/fonts row (the HUD still uses the host bitmap font; the GUI uses
Geist).

## Blocked on the host

Each names the capability missing at pin 8f91f89 (details in the ledger's
decision column and decisions.md):
- **Reach** and its entity-position helper: the integrated/LAN server re-aims a guest attack and re-picks it at 3 blocks.
- **Throwpot Soup/Both:** no per-stack consumable component synced to the client and honoured by item use.
- **ESP Skeleton** (mode and helper): no player-model bone poses at render time.
- **Chams:** no per-draw entity pipeline (polygon offset) and no depth-tested pass after entities.
- **XRay:** terrain alpha carries emission, so Opacity can't be drawn, and the layer split is inline.
- **AutoMace rotation controller** (Aim, Silent aim, Elytra swaps): no side-effect-free fall simulation of the local player.

## Evidence on merged main

- `gates.sh --host` on `d90e860` (all nine workers merged): fmt, Clippy `-D warnings` (all targets), **895 tests passed / 0 failed**, payload-test, stage/audit with **64 hooks and 35 support modules** on horizon-mixins `0e9229d`, host check and host build.
- `gates.sh --host` with the BackTrack and TargetInfo fixes: fmt, Clippy, **899 tests passed / 0 failed**, payload-test, stage/audit and host build.
- Scenarios: each worker's scenarios passed on its branch in fresh homes (progress.md lists the instance names). On merged main (bundle `692a0f79`), these pass: bindings, macros, profile switch, the GUI, text editor and HUD editor (after a scenario-side triage for the moved layout), the inbound hold, both Target Info scenarios and the BackTrack/Blink/planner/disabled load parts. Also passing, on earlier main builds: pearl, render (single player and LAN), legit render and the AutoTool LAN guard. TellyBridge is 14/16 and is with worker placement-r5.
- Settings parity (`tools/settings_parity.py`): no default or range differences remain. It now honours the reference's version-gated registrations (`U(value, ForgeVersion…)`, target 26.3). The remaining "missing setting" entries are all on integrated or blocked rows: Blink "Spawn fake", KillAura "Show target" and its colours, Reach "Misplace"/"Disadvantage", SilentAura's Break blocks, render and colour settings, and SilentAura's AI settings (excluded).
- AntiBot's 16 "Your team color" options are not-applicable-reviewed: the reference registers that setting only below 1.21.11 and uses the player-list colour on 26.3, which the port detects.
- Online removal (Phase 7): the pack-time capability audit rejects `unsafe`, sockets, processes, FFI, `include_*!`, the host HTTP client, telemetry/friends crates and URL literals in shipped code; no Java/C/C++ or binaries are tracked.
- Performance (plan §10 item 7, all on main in fresh homes):
  - Per-module frame cost, `tools/perf_scenario.py`, near-idle machine (load ~6 of 12 cores): 67/67. Each render/HUD module alone adds under 0.5 ms of CPU per frame, all twelve together 0.296 ms, at a steady 119 fps, and nothing is left once off (docs/evidence/perf-idle-main, progress.md 2026-10-31).
  - Worst-case loads, `tools/perf_load_scenario.py` against the pinned host built without vape-rs (`tools/plain_build.sh`), progress.md 2026-11-01:
    - With every module off, vape-rs adds +0.032 ms of CPU per frame in a 24-cow scene and −0.262 ms in a 300-cow scene (within the noise). ESP, Tracers and NameTags on 300 cows add 2.349 ms.
    - Three inventory planners on a full inventory add 0.026 ms to the frame tick.
    - Blink holds 45 s of moves (unbounded, as in the reference; keep-alives bypass it, so the guest stays connected) and sends them with a 3.066 ms slow-frame tick.
    - BackTrack holds 300 cows' traffic up to the 1024-packet inbound limit and replays a full queue inside a 0.234 ms slow-frame tick.
  - These figures were taken while other sessions compiled (load 2–15), so they bound the cost rather than measure it precisely.
- Not covered: remote/third-party servers, window focus loss, switching between two worlds.
- [hooks.md](hooks.md) lists every target and module.

## Release and maintenance (Phase 8)

- `xtask drift <horizon> [revision] [--compile]` checks an upstream candidate without touching the pin, and `.github/workflows/drift.yml` runs it daily on Horizon main (read-only, never publishes). `verify.yml` runs the gates; `release.yml` builds bundles and never publishes on its own.
- compatibility-matrix.json maps each mod version to its Horizon, mixin, game and protocol pins and platforms; `tools/bundle.py` writes a per-artifact `.compatibility.json` with its SHA-256 and refuses a stable build whose pins differ from the matrix.
- docs/development.md covers build, install, update, rollback and settings preservation.
- Re-pin: the earlier candidate `5062f225` is stale; Horizon main has moved about 1,450 commits further. The pin stays at 8f91f89 for this cycle. A re-pin needs `xtask drift --compile` on the new head, a hook review and the full scenario rerun. Some of the blocks above may be lifted by it, so each is re-checked then.
- Linux x86-64 is the only platform exercised; Windows and macOS are listed as unvalidated in the matrix. The installed `horizon-launcher native-build` run is the player's to make; `tools/launcher_build.py` reproduces that route for agents.

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
pins (`--allow-stale` overrides with a warning). This development build
disables audio/controllers; the virtual control-socket pad still works.

Run `python3 tools/play.py` from the project root; rebuild after source changes with
`cargo run -p xtask --locked --offline -- build`. It opens the visible native
client in a fresh disposable survival instance under target/scenarios/manual-*.
The default prepared resources are the read-only path above; a positional path
overrides them. `--creative` starts
creative (inventory automation requires survival). Quit the window normally;
the test instance and game.log are retained. No normal player worlds/settings
are used. Right Shift opens the GUI, F8/F9 toggle coordinates/FPS and F10 is panic.

Modules with real host evidence are listed under "Ledger" above; treat the
rest as experimental until their scenarios run. Unfinished features are not
available by enabling hidden options.
