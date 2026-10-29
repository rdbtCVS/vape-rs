# Native hook catalogue (development)

Current audited configuration: **22 HEAD hooks, one Game-owned state, 9 support modules**. Horizon `e1b2b0b2fdf5962a30fb5de31b0c1bd6829d9083`; mixin base `29a8f8f84b253ea89422c0049ff5e8bfda36f1fb` plus the hashed lifetime-state patch in compatibility.json.

The manifest is authoritative for exact targets and bindings. All targets resolve against the committed pristine export under target; neither adjacent checkout is modified. The audit removes exactly the declared helpers, modules and owned state, then checks that the original host AST and other source hashes are restored. Runtime compilation and actual gameplay evidence are separate checks.

## Hooks

| ID | Exact target | Behavior |
| --- | --- | --- |
| `client-tick` | `horizon_client::game::Game<'_>::tick` | Client context/storage/tick; AutoTool, FastPlace and input-delay dispatch before native tick. |
| `client-input` | `horizon_client::game::debug_keys::Game<'_>::debug_key` | Shared physical keyboard press/release/repeat dispatch and native clipboard actions. |
| `client-hud` | `horizon_client::game::extras::Game<'_>::draw_extras` | Real snapshot + shared native HUD painter; F1/screen gates retained. |
| `control-key-names` | `horizon_client::game::control::key_code` | Cancellable HEAD. Supplies missing mod/editor control-key names; continues original parser otherwise. |
| `gui-events` | `horizon_client::game::<Game<'_> as ApplicationHandler>::window_event` | Owned-window pointer, text, IME and focus events. |
| `gui-render` | `horizon_client::game::debug_keys::Game<'_>::draw_debug_overlay` | Paints the owned local GUI through the host atlas. |
| `game-key-capture` | `horizon_client::game::Game<'_>::mapped_key` | Cancellable HEAD. Consumes handled gameplay key presses; releases retain cleanup. |
| `control-key-capture` | `horizon_client::game::control::Game<'_>::control_key_down` | Cancellable HEAD. Same isolation for controlled key events. |
| `mouse-bind` | `horizon_client::game::Game<'_>::play_mouse` | Cancellable HEAD. Physical mouse bindings/CPS/macros; consumes handled presses. |
| `client-frame` | `horizon_client::game::Game<'_>::hud` | Owned GUI/save lifecycle; brightness, macros, inventory and fishing advance while menus pause world ticks. |
| `session-world-open` | `horizon_client::game::Game<'_>::open_world` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-world-close` | `horizon_client::game::Game<'_>::close_world` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-dimension` | `horizon_client::game::Game<'_>::enter_dimension` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-respawn` | `horizon_client::game::screens::Game<'_>::respawn_player` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-connect` | `horizon_client::game::remote::Game<'_>::join_server` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-disconnect` | `horizon_client::game::remote::Game<'_>::leave_server` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-remote-dimension` | `horizon_client::game::remote::Game<'_>::enter_remote_dimension` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `no-hurt-camera` | `horizon_client::game::fov::Game<'_>::bob_matrix` | Cancellable HEAD. Cancellable render-only copied camera state, preserving view bob/death roll. |
| `auto-tool-attack` | `horizon_client::game::Game<'_>::attack` | Instant weapon-swap dispatch before normal local attack. |
| `fullbright-night-vision` | `horizon_client::game::sculk::Game<'_>::night_vision_intensity` | Cancellable HEAD. Cancellable render intensity override; real potion state remains intact. |
| `clear-water` | `horizon_client::game::render_tick::Game<'_>::frame_fog` | Cancellable HEAD. Cancellable underwater fog helper retaining pinned native fog inputs/math. |
| `fish-hook-data` | `horizon_client::game::fishing::Game<'_>::remote_hook_data` | Observes only own-hook DATA_HOOKED_ENTITY into session-owned state; original packet handling continues. |

## Support modules

| Name | Host insertion file | Editable source |
| --- | --- | --- |
| `vape_rs` | `crates/horizon-client/src/game.rs` | `src/lib.rs` (expanded to `runtime.rs`) |
| `vape_hud` | `crates/horizon-client/src/game.rs` | `mixins/vape-rs/hud_painter.rs` |
| `vape_camera` | `crates/horizon-client/src/game.rs` | `mixins/vape-rs/camera.rs` |
| `vape_inventory` | `crates/horizon-client/src/game.rs` | `mixins/vape-rs/inventory.rs` |
| `vape_transactions` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape-rs/transactions.rs` |
| `vape_macros` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape-rs/macros.rs` |
| `vape_fast_place` | `crates/horizon-client/src/game.rs` | `mixins/vape-rs/fast_place.rs` |
| `vape_auto_fish` | `crates/horizon-client/src/game/fishing.rs` | `mixins/vape-rs/auto_fish.rs` |
| `vape_input_delays` | `crates/horizon-client/src/game.rs` | `mixins/vape-rs/input_delays.rs` |

The runtime source is deterministically expanded from src/lib.rs. Each Game owns one Arc<Mutex<Runtime>>; there is no process/thread singleton. Adapters release runtime borrows before calling host actions. Session generations invalidate stale entity, slot, container and hook observations. Transient state never enters local profiles.

Native input/menu effects use Horizon’s normal action paths. Macro, fishing, tool and container ownership is coordinated per client. Server/dedicated simulation is guarded out. The GUI owns a Paused screen and completes closing on a later frame so a close click cannot attack.

BlockOverlay drafts under docs/drafts are unregistered and excluded from the runtime payload. Their outline/fill integration requirements are recorded in docs/specs/block-overlay.md. No full BlockOverlay implementation is claimed.

Current evidence and remaining validation are in [status.md](status.md) and [progress.md](progress.md). Public packaging remains gated until a published mixin-tool pin contains the required state patch.

The targeting/rotation primitives under `src/targeting` are pure and currently have no host hook or live snapshot adapter. They do not alter the hook counts above.

Sprint uses the existing `client-tick` hook and `vape_input_delays` support module to supply the host's normal sprint key state; its per-client flag tracks the synthetic key for cleanup. This is a baseline, not the planned general movement-input owner.

Parkour currently runs from the existing client-tick hook, querying host collision shapes and briefly injecting the jump key. This adapter is experimental and has not had host gameplay scenarios.
