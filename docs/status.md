# Current implementation handoff

Updated 2026-10-31 on main `97dbaa7`, after the hud-client (round 2),
clicker-movement-debt (round 3) and hud-modules (round 3) merges and the
Panic scenario. Single player is tested on Horizon's integrated server
thread; LAN host/guest scenarios cover the packet, combat and movement paths
that differ there. **The full plan is not complete.** AI behavior and the
online product are excluded; all other retained scope is listed below as
validated, in progress or blocked.

## Ledger

docs/features.csv holds 472 rows: **101 validated**, 36 integrated,
54 implemented-core, 246 not-started, 12 blocked, 13 excluded-approved and
10 not-applicable-reviewed. 131 of the not-started rows are helper classes
whose status follows their owning module (target/helper-owners.txt), so they
move when the owners are validated. `validated` always means a passing
fresh-home host scenario with evidence in docs/evidence; `integrated` means
hooked and running in the host without a full scenario
([decisions](decisions.md) explains the levels).

Ledger gap: 37 category-level reference classes have no rows. Most are
modules the reference's ModManager never registers (LongJump, AntiFall, Step,
Strafe, Phase, Regen, OmniSprint, PotionSaver, AntiFML, DiscardedBlink,
KnockbackTest, SelfDestruct, the debug modules); the rest are control-claim
and macro support classes.
The port nevertheless ships LongJump, AntiFall and Step (compile-only, see
progress.md 2026-10-30), which the reference GUI never shows. Before release
they need a ledger decision: keep them as tracked extras or drop them for
fidelity.

Validated in a real game:
- **Combat:** AutoClicker, RightClicker, TriggerBot, JumpReset, WTap, Sprint, KeepSprint, Velocity, HitBoxes, SilentAura clicker.
- **Movement:** NoSlowdown, Speed (Bhop, Mineplex), Fly, NoFall, InvWalk, Timer, Scaffold (Legit, with the edge-sneak helper), SafeWalk, NoItemRelease, WindCharge.
- **World / utility:** Anti-AFK, AutoFish, FastPlace, Backtrack, KnockbackDelay, Panic.
- **Inventory:** Refill, Throwpot, ThrowDebuff.
- **Render:** Fullbright, Clear Water, Time Changer, Weather (both modes), AntiDebuff, ItemESP, NoHurtCam, NoClickDelay, NoJumpDelay, Inventory Blur.
- **HUD:** Armor Status, Potion Status, Coords (both modes), FPS, Compass, Clock, Keystrokes, Reach Display; local toggle notifications; the client GUI and config settings modules.

Integrated, waiting for a full scenario: AimAssist, KillAura, SilentAura,
AutoHeal (Legit), AutoArmor, AutoTotem, InventoryFill, ChestSteal, Parkour,
Blink, FakeLag, Scaffold (parent row), Text GUI, Scoreboard, MurdererFinder,
PropHunt, and the render group (ESP, Tracers, Trajectories, Search,
SpawnerFinder, Health, Arrows, Explosions, NameTags, StorageESP); the click
GUI layout/search/favorites, friends/enemies and local profiles.

Implemented core (logic and tests, host path incomplete): AutoTool,
InventoryManager, InvCleaner, AutoHotbar, ArmorSwitch; binds, macros, typed
setting editors, HUD drag/snap/scale and the shared claim owners.

## Work in flight

Nine workers are finishing their current items on branches and will be
merged one at a time with the host gates and their scenarios rerun on main:
- **combat-a:** BlockHit (Manual/Predict/Auto/Lag, committed), HitSelect, WTap submodes, HitFlick, TargetFilter, SilentAura rows.
- **combat-b:** HitSwap, AutoMace, ShieldBreaker, CrystalAura, AntiFireball, Reach re-check.
- **movement-a:** Scaffold Telly/God bridges and the rotation merge, BlockIn, Clutch, MLG, AutoLadder, AutoAnchor.
- **movement-b:** Freecam (camera hooks), Freelook on the same hooks, AutoPearl, PearlCatch, FakeLag modes, Blink, LongJump, Strafe.
- **render:** Projectiles, Indicators, BedPlates, Block Overlay, ESP modes, MurdererFinder; re-check of Chams/XRay/MotionBlur.
- **inventory:** InventoryManager editor UI and planners, AutoHeal Silent/Potions (two movement-packet hooks).
- **inventory-utility-debt:** Refill/ThrowPot/ThrowDebuff modes, AutoArmor, AutoTool guards (LAN).
- **client-foundation:** binds, macros, profiles, friends, HUD editor, arbitration, lifecycle, Text GUI settings.
- **hud-client-debt:** localization, scoreboard text replacement, Target Info, timing.

Unowned after this round: the HUD font (HUD text still uses the host bitmap
font; the GUI already uses Geist), the HUD editor's transparency and the
armor bar's glow.

## Blocked on the host

Each names the capability missing at the pin:
- **Reach:** the integrated/LAN server re-picks guest attacks at 3 blocks (being re-checked).
- **AutoHeal Silent:** server clicks are only sent with the inventory screen open; no held-item use API (being re-checked).
- **Chams, XRay:** no mesher or entity-render hooks (being re-checked against the crates mixins can target).
- **MotionBlur:** no frame-accumulation hook.
- **Freelook (7 rows):** recorded as needing a camera-rotation hook. movement-b's Freecam hooks on `player_camera_entity` and `turn_player` provide it, so these rows are being implemented rather than staying blocked.

## Evidence on main 97dbaa7

- `gates.sh --host`: fmt, Clippy `-D warnings` (all targets), **431 tests passed / 0 failed**, payload-test, stage/audit with **44 hooks and 29 support modules** on horizon-mixins `0e9229d`, host check and fast build (68 warnings, none fatal).
- Scenarios rerun on merged main (fresh isolated homes): hud_detail 8/8, weather_modes 13/13, hud_editor 9/9, hud_modes 5/5, hud_readouts 7/7, no_hurt_camera 7/7; panic 12/12; the clicker-movement-debt set (clicker modes, TriggerBot and Sprint/SafeWalk over LAN, Velocity). Results and the evidence folders are listed per merge in [progress.md](progress.md).
- Online removal (Phase 7): the pack-time capability audit rejects `unsafe`, sockets, processes, FFI, `include_*!`, the host HTTP client, telemetry/friends crates and URL literals in shipped code; no Java/C/C++ or binaries are tracked; the nine online-product and four AI rows are `excluded-approved`.
- Settings parity: `tools/settings_parity.py` compares every reference setting definition with the port's settings dump; it is rerun after the merges.
- Performance: `tools/perf_scenario.py` measures per-module frame cost. The last runs were on a machine loaded by the workers and are not attributable; performance stays unvalidated until the idle-machine rerun after this round.
- Not covered: remote/third-party servers, window focus loss, switching between two worlds.
- [hooks.md](hooks.md) lists every target and module.

## Release and maintenance (Phase 8)

- `xtask drift <horizon> [revision] [--compile]` checks an upstream candidate without touching the pin, and `.github/workflows/drift.yml` runs it daily on Horizon main (read-only, never publishes).
- compatibility-matrix.json maps each mod version to its Horizon, mixin, game and protocol pins and platforms; `tools/bundle.py` writes a per-artifact `.compatibility.json` with its SHA-256 and refuses a stable build whose pins differ from its entry.
- docs/development.md covers build, install, update, rollback and settings preservation.
- Re-pin candidate: Horizon main `5062f225` (Minecraft 26.3 / protocol 777 unchanged; all hooks plan, apply and audit; seven changed hook targets reviewed). Its new anticheat runs only on a dedicated server's hub, so integrated and LAN scenarios are unaffected. The re-pin follows the merges and a full scenario rerun.
- Linux x86-64 is the only platform exercised; Windows and macOS are unvalidated. The installed `horizon-launcher native-build` run is the player's to make; `tools/launcher_build.py` reproduces that route for agents.

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

Modules with real host evidence are listed under "Ledger" above; treat the
rest (render modules, inventory planners, macros) as experimental until their
scenarios run. BlockOverlay and the rest of the unfinished plan are not
available by enabling hidden options.
