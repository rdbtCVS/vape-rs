# AutoFish

Static reference: `module/utility/AutoFish.java` in the pinned Vape-v4 tree.
It repeatedly uses a held fishing rod, reels when a bite occurs, waits one
second before casting again, optionally reels a caught entity, and optionally
reels a grounded hook after three seconds. `Refill rods` selects a hotbar rod
or opens the normal inventory and quick-moves a main-inventory rod; its click
delay is randomized in the retained 50–200 ms range (default 75–125 ms).

The pure state machine is `modules::utility::auto_fish::AutoFish`. It accepts
only a bounded host context (`Rod`, `Hook`, inventory-open state and monotonic
time) and emits one typed effect at a time. It has no packet, input or menu
dependency. Cancellation clears all cast/ground/refill pending state.

The native adapter belongs in `game::fishing::vape_auto_fish`; nesting under
`game::fishing` gives it exact private `Hook` state. It reads `Hook::nibble`
for real local and remote `DATA_BITING` detection and `Hook::state`/`on_ground`
for caught/ground conditions, rather than using velocity or particle guesses.
It emits normal Horizon use input through the shared macro use helper and
uses the shared transaction snapshot/request/submit path for the real
quick-move, including its cursor/revision checks. It waits for a later host
snapshot before selecting the moved rod.

`DATA_HOOKED_ENTITY` is observed by a separate `HEAD` adapter on
`Game::remote_hook_data`. Its protocol `Value::Int` is encoded as target ID
plus one (zero means no target), so the adapter stores only a decoded positive
target in the runtime's mod-owned fish-hook state. This supplements, rather
than mutates, Horizon's remote Hook model. `macro_rod_ready` returns true for
an on-ground bobber, a local hook attached to a player target, or such a
remote target observation.

Required runtime integration: add `ModuleId::AutoFish`, an `auto_fish:
AutoFish` field, and setting definitions:

- `utility.auto-fish.recast-ground`: boolean, default `false`.
- `utility.auto-fish.recast-caught`: boolean, default `false`.
- `utility.auto-fish.refill-rods`: boolean, default `true`.
- `utility.auto-fish.click-delay`: number range 50–200 ms, step 5, defaults
  75–125 ms.

The adapter must be called once per focused gameplay frame/tick through a
support module injected into `crates/horizon-client/src/game/fishing.rs`. It
guards dead/unfocused players, menus, title/dialog/server-pack screens,
sleep/spectator and active macro item use. A disabled/non-playable call must
call `AutoFish::cancel`; it does not synthesize packets or mutate inventory.

Runtime bridge methods required by the adapter are
`auto_fish_step(Context, Options) -> Option<Effect>`, `cancel_auto_fish()`,
`auto_fish_refilling() -> bool`, and `inventory_busy() -> bool`. The last
prevents AutoFish from beginning its refill while an inventory planner owns a
transaction; the runtime must likewise keep its planners out while AutoFish
reports a refill in progress. The shared macro bridge must expose
`game::remote::vape_macros::use_once(&mut Game)` for the adapter.

The implemented metadata HEAD observer retains this client's hook target in
mod-owned session state, so caught-entity recasting is connected in remote
play as well. Real `DATA_BITING` supplies remote bite detection. The pinned
host does not currently update the remote hook's on-ground flag; remote
Recast-ground parity therefore remains an explicit validation/integration gap.
Rod-refill requests verify live slot/cursor fingerprints and wait for changed
container revision before selecting the replacement. Shared hotbar/use/container
claims prevent concurrent macro, tool and menu actions from stealing ownership.
