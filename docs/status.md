# Current implementation handoff

Updated 2026-10-28 at the user's request to finish the current batch and bring
the documentation up to date. **The full plan is not complete.** AI behavior
and the online product are excluded; all other retained scope remains.

## Connected native implementation

The runtime registers 18 modules, all with normal local activation, binds,
profile persistence and GUI access:

| Area | Modules |
| --- | --- |
| HUD | Coordinates, FPS, Compass, Clock, Keystrokes/CPS, Armor Status, Potion Status |
| Render | NoHurtCam, Fullbright, Clear Water |
| Inventory / world / utility | AutoTool, AutoArmor, Refill, ChestSteal, FastPlace, AutoFish, NoJumpDelay, NoClickDelay |

The local foundation includes typed settings/editors, chord/held bindings,
local profiles and friends/enemies, text/clipboard/IME editing, local macro
creation/binding/delays, and HUD drag/snap/scale editing. Each Game owns its
runtime. Session generations and hotbar/use/container claims guard transient
work. Native effects use normal host input/menu/chat paths.

The latest macro cleanup restores its original slot on GUI/focus/panic/profile
cleanup only while it still owns the claim and user selection is unchanged.
Manual selection, preemption, death and world replacement win. New tests cover
menu cleanup, manual override and panic versus dimension replacement.

## Evidence and limits

- The close-out workspace pass has **48 runtime tests + 4 audit tests passing**;
  formatting and Clippy with warnings denied pass. Final native staging/check
  results are recorded in [progress.md](progress.md) and
  [macro-cleanup-closeout.json](evidence/macro-cleanup-closeout.json).
- The native configuration has **22 HEAD hooks, one owned state and nine
  support modules**. [hooks.md](hooks.md) lists all current targets and modules.
- Earlier resource-backed foundation/GUI/lifecycle/HUD/NoHurtCam/AutoTool
  scenarios passed on integrated-server and in-frame paths. Evidence is under
  docs/evidence; those are specific slices, not full reference parity.
- Recent widgets, macros, inventory, brightness/fog, fishing and input-delay
  batches have compilation evidence. **No new gameplay/visual suite was run**
  for them, following the user's implementation-first preference. No ledger
  row was promoted to `validated` from compilation alone.

Concrete implementation milestones:

| Commit | Change |
| --- | --- |
| `4d7e729` | Approved AI exclusion, retaining SilentAura PID |
| `2f0b433` | AutoTool with real native mining/selection evidence |
| `a4a9b6f` | Typed dropdown/slider/range/color/pointer editors |
| `3509cca` | Four HUD widgets, local macros and native inventory automation |
| `90a9f78` | FastPlace, Fullbright and inventory metadata safeguards |
| `17371d2` | AutoFish, Clear Water, shared use/container claims and input delays |

The final close-out commit adds safe macro restoration, focused tests, lint
cleanup and these documentation updates; Git history records its exact SHA.

## Open dependencies and next work

1. **BlockOverlay:** preserved design drafts are under docs/drafts and are
   excluded from the payload and registry. Its outline boundary is known;
   transparent fill requires an opaque-body-safe geometry insertion capability
   in the mixin tool or an explicit host geometry hook. The exact proposed
   layer, vertex format and hook are in [block-overlay.md](specs/block-overlay.md).
   No BlockOverlay implementation or host validation is claimed.
2. **Recent module fidelity:** remote AutoFish ground-state observation, custom
   potion effects/item names, creative inventory protocol support, precise
   inventory scoring/selection parity, shader-pack brightness inputs, HUD
   typography/icons/localization and Windows date conventions remain open.
3. **Full retained port:** InventoryManager/rule editors, remaining HUD/UI/world
   render features, and the targeting/rotation/movement/packet foundations and
   combat/utility modules remain unfinished. Continue their dependencies from
   [plan.md](../plan.md) and [features.csv](features.csv); do not reduce scope or
   call the current implementation the full port.
4. **Validation after implementation:** run the new batch in fresh isolated
   worlds on integrated-server and in-frame paths, then independent remote/LAN
   cases, visual comparisons, interruption/cleanup checks and performance.
   The later manual-testing preparation rebuilds the native executable; a
   successful build still does not constitute new gameplay/visual evidence.
5. **Distribution:** launcher/public packaging is gated until a published mixin
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

GUI/profile/HUD editing and AutoTool have earlier real host evidence. Treat the
newer inventory, macro, fishing and render features as experimental until their
resource-backed validation pass. BlockOverlay and the rest of the unfinished
plan are not available by enabling hidden options.

## Partial targeting groundwork

Pure candidate filtering, five sort modes and bounded rotation math have been added; see [targeting-foundation.json](evidence/targeting-foundation.json). They are not connected to live entities or player rotation, and no new combat or movement module is registered. Native snapshots, scoring, visibility, input/action dispatch and host scenarios remain the next dependency work.

## Sprint input baseline

Sprint is registered with its Cancel Invis setting and feeds the host's sprint key at the existing client tick hook. It is an experimental input baseline: scaffold and shared movement arbitration, exact reference conditions, controlled-input interference and real host scenarios remain open. See [sprint-core.json](evidence/sprint-core.json). The targeting foundation remains unconnected.
