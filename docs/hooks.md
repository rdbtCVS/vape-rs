# Native hook catalogue (development)

Horizon `e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083`; mixins `29a8f8f84b253ea89422c0049ff5e8bfda36f1fb`. Full pristine hashes are in compatibility.json. The generated plan records exact original function fingerprints/node paths; selectors below must resolve once. Priority 100. Gameplay hooks do not cancel or replace their targets; original methods run normally after helpers. The control-key parser hook supplies only missing mod/editor key names (F8/F9/F10, Right Shift, Home/End/Delete).

| ID | Source / exact target | Bindings and phase | Lifetime/thread contract |
| --- | --- | --- | --- |
| client-tick | `game.rs`, `horizon_client::game::Game<'_>::tick` | HEAD, `&mut *self` | Runs before remote-client early return. Reject serving/dedicated before accessing thread-local runtime. First tick emits local diagnostic once. No recursive host calls. |
| client-input | `game/debug_keys.rs`, `horizon_client::game::debug_keys::Game<'_>::debug_key` | HEAD, `&mut *self`, physical key/action by value | Shared physical-key path for real keyboard events and horizon-ctl key taps. Focused Playing screen and local player required to toggle; release clears state. Fixed F8/F9/F10 are temporary integration controls pending full bind editor. |
| client-hud | `game/extras.rs`, `horizon_client::game::extras::Game<'_>::draw_extras` | HEAD, game and batch mutable borrows | Called in visible world HUD layer. Copies real PlayerMotion position and Game FPS. Host atlas paints text; no retained GPU handles. Rejects serving/dedicated. Borrow ends before original body. |
| control-key-names | `game/control.rs`, `horizon_client::game::control::key_code` | Cancellable HEAD, borrowed `name` | Returns F8/F9/F10 for those names (case-insensitive), continues the original parser for every other name. The original parser body contains no opaque macros. This lets the actual host control path send the shipped integration keys; it does not toggle modules itself. |

The runtime payload is inserted as `game::vape_rs` into game.rs. Generic lifetime Game does not satisfy this tool's nongeneric per-instance state contract. The original adapter used thread-local state as scaffolding. The checked-in development tool patch now supports lifetime-only state owners and preserved constructor bodies. One `Arc<Mutex<Runtime>>` is initialized on each Game; adapters clone a temporary handle before host effects. No singleton remains. Fixture tests demonstrate independent instances and moves/drops; actual profile scenarios pass on both host paths. Session invalidation before stateful gameplay remains the next dependency. The launcher distribution gate requires a published tool pin containing the patch.

Proof: `xtask stage` plans/applies against a committed export. `xtask audit` removes exact planned helper statements only at method HEAD and the exact emitted module; the remaining canonical AST must match. Other plan inputs must retain their hashes. Unit tests reject changed helper bodies and reordered calls. Full binary checking and resource-backed scenarios remain separate evidence.

## GUI foundation additions

- `client-frame`: HEAD on `game::Game<'_>::hud`; handles client-owned GUI lifecycle and pending preference writes even when world ticking is paused.
- `gui-events`: noncancellable HEAD on `game::<Game<'_> as ApplicationHandler>::window_event`; validates the owned window, then dispatches native pointer/text/IME/focus events. Original body is preserved.
- `gui-render`: HEAD on `game::debug_keys::Game<'_>::draw_debug_overlay`; custom GUI draw commands use the host atlas/batch. Only while owning the GUI does it replace underlying menu vertices/ranges; glyph upload patches remain available.
- `game-key-capture` and `control-key-capture`: cancellable HEAD on `Game::mapped_key` and `control::Game::control_key_down`; consume handled presses before game mapping/action, while releases retain cleanup.
- `mouse-bind`: cancellable HEAD on `Game::play_mouse`; named physical mouse bindings share runtime activation logic and consume handled presses before attack/use. Original release behavior is retained.

The GUI uses an owned `Screen::Paused` with the normal pause-menu controls disabled; it does not impersonate a sandboxed mod screen or stock settings widgets. Closing is completed on a subsequent frame so the close pointer event cannot become a world attack. Compatibility errors remain fail-closed; the precise contracts are in the current native manifest and generated plan.
