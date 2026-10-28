# Native hook catalogue (development)

Horizon `e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083`; mixins `29a8f8f84b253ea89422c0049ff5e8bfda36f1fb`. Full pristine hashes are in compatibility.json. The generated plan records exact original function fingerprints/node paths; selectors below must resolve once. Priority 100. Gameplay hooks do not cancel or replace their targets; original methods run normally after helpers. The control-key parser hook supplies only the three missing mod-key names.

| ID | Source / exact target | Bindings and phase | Lifetime/thread contract |
| --- | --- | --- | --- |
| client-tick | `game.rs`, `horizon_client::game::Game<'_>::tick` | HEAD, `&mut *self` | Runs before remote-client early return. Reject serving/dedicated before accessing thread-local runtime. First tick emits local diagnostic once. No recursive host calls. |
| client-input | `game/debug_keys.rs`, `horizon_client::game::debug_keys::Game<'_>::debug_key` | HEAD, `&mut *self`, physical key/action by value | Shared physical-key path for real keyboard events and horizon-ctl key taps. Focused Playing screen and local player required to toggle; release clears state. Fixed F8/F9/F10 are temporary integration controls pending full bind editor. |
| client-hud | `game/extras.rs`, `horizon_client::game::extras::Game<'_>::draw_extras` | HEAD, game and batch mutable borrows | Called in visible world HUD layer. Copies real PlayerMotion position and Game FPS. Host atlas paints text; no retained GPU handles. Rejects serving/dedicated. Borrow ends before original body. |
| control-key-names | `game/control.rs`, `horizon_client::game::control::key_code` | Cancellable HEAD, borrowed `name` | Returns F8/F9/F10 for those names (case-insensitive), continues the original parser for every other name. The original parser body contains no opaque macros. This lets the actual host control path send the shipped integration keys; it does not toggle modules itself. |

The runtime payload is inserted as `game::vape_rs` into game.rs. Generic lifetime Game does not satisfy this tool's nongeneric per-instance state contract. The initial adapter uses thread-local state only after a client guard; it owns no world/entity handles or queued actions. A full lifecycle/session owner is required before stateful gameplay modules. Multiple sequential client Game instances on one thread are not yet validated.

Proof: `xtask stage` plans/applies against a committed export. `xtask audit` removes exact planned helper statements only at method HEAD and the exact emitted module; the remaining canonical AST must match. Other plan inputs must retain their hashes. Unit tests reject changed helper bodies and reordered calls. Full binary checking and resource-backed scenarios remain separate evidence.
