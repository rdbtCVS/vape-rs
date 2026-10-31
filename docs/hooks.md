# Native hook catalogue (development)

Current audited configuration: **38 hooks (37 HEAD + 1 MODIFY_ARG), 25 support modules, no owned state**. Horizon `8f91f896a5d47d034318569940e775d8bd854bd6` (main, 2026-10-31); published, unpatched horizon-mixins main `07dab17387480ac03829584d9d1ee9912c9bb515`, the pin the `.hnmod` names.

The manifest is authoritative for exact targets and bindings. All targets resolve against the committed pristine export under target; neither adjacent checkout is modified. The audit removes exactly the declared helpers and modules, rejects any owned state, then checks that the original host AST and other source hashes are restored. Runtime compilation and actual gameplay evidence are separate checks.

## Hooks

| ID | Exact target | Behavior |
| --- | --- | --- |
| `client-tick` | `horizon_client::game::Game<'_>::tick` | Client context/storage/tick; AutoTool, FastPlace and input-delay dispatch before native tick. |
| `client-input` | `horizon_client::game::debug_keys::Game<'_>::debug_key` | Shared physical keyboard press/release/repeat dispatch and native clipboard actions. |
| `client-hud` | `horizon_client::game::extras::Game<'_>::draw_extras` | Real snapshot + shared native HUD painter; F1/screen gates retained. |
| `control-key-names` | `horizon_client::game::control::key_code` | Cancellable HEAD. Supplies missing mod/editor control-key names; continues original parser otherwise. |
| `gui-events` | `horizon_client::game::<Game<'_> as ApplicationHandler>::window_event` | Owned-window pointer, text, IME and focus events. |
| `gui-render` | `horizon_client::game::debug_keys::Game<'_>::draw_debug_overlay` | Paints the owned local GUI through the host atlas, with text in the theme's interface font (`vape_smooth_text`). |
| `game-key-capture` | `horizon_client::game::Game<'_>::mapped_key` | Cancellable HEAD (`adapters/mapped_key.rs`, binds `key`, `pressed`). Consumes handled gameplay key presses; otherwise records the canonical key event (rebound key, controller, socket) for the movement writer. |
| `control-key-capture` | `horizon_client::game::control::Game<'_>::control_key_down` | Cancellable HEAD (`adapters/control_key.rs`, binds `key`). Same isolation for socket key presses; records keys inserted outside the toggle mappings for the movement writer. |
| `mouse-bind` | `horizon_client::game::Game<'_>::play_mouse` | Cancellable HEAD. Physical mouse bindings/CPS/macros; consumes handled presses. |
| `client-frame` | `horizon_client::game::Game<'_>::hud` | Owned GUI/save lifecycle; brightness, macros, inventory and fishing advance while menus pause world ticks. |
| `session-world-open` | `horizon_client::game::Game<'_>::open_world` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-world-close` | `horizon_client::game::Game<'_>::close_world` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-respawn` | `horizon_client::game::screens::Game<'_>::respawn_player` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-connect` | `horizon_client::game::remote::Game<'_>::join_server` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-disconnect` | `horizon_client::game::remote::Game<'_>::leave_server` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `session-remote-dimension` | `horizon_client::game::remote::Game<'_>::enter_remote_dimension` | Session invalidation before the native world/connection/dimension/respawn boundary. |
| `no-hurt-camera` | `horizon_client::game::fov::Game<'_>::bob_matrix` | Cancellable HEAD. Cancellable render-only copied camera state, preserving view bob/death roll. |
| `inventory-blur` | `horizon_client::game::Game<'_>::menu_blur` | Cancellable HEAD. Inventory Blur returns the menu blur radius (or 5 when the setting is off) for the survival/container and creative screens, which 26.3 leaves unblurred. |
| `scoreboard-sidebar` | `horizon_client::game::scoreboard::Game<'_>::draw_scoreboard` | Cancellable HEAD. Scoreboard skips the vanilla sidebar while the module is on; the HUD panel paints the same `hud_overlays::sidebar` draws at its own position. |
| `auto-tool-attack` | `horizon_client::game::Game<'_>::attack` | Instant weapon-swap dispatch before normal local attack. |
| `fullbright-night-vision` | `horizon_client::game::sculk::Game<'_>::night_vision_intensity` | Cancellable HEAD. Cancellable render intensity override; real potion state remains intact. |
| `clear-water` | `horizon_client::game::render_tick::Game<'_>::frame_fog` | Cancellable HEAD. Cancellable underwater fog helper retaining pinned native fog inputs/math. |
| `fish-hook-data` | `horizon_client::game::fishing::Game<'_>::remote_hook_data` | Observes only own-hook DATA_HOOKED_ENTITY into session-owned state; original packet handling continues. |

| `aim-assist-look` | `horizon_client::game::pad::Game<'_>::pad_look` | Runs after `apply_mouse_look` and before `render` each frame. AimAssist builds the live entity snapshot and turns the view through the host `mouse::Rotation::turn` (`Entity.turn`). The controller look that follows is unchanged. |
| `safe-walk-edge` | `horizon_client::motion::<MotionWorld<'_,'_,'_> as EdgeBackoffWorld>::staying_on_ground_surface` | Cancellable HEAD (library crate). Returns true for the one local player the movement writer marked for SafeWalk this tick, so the host's own `back_off_from_edge` runs without a sneak key or crouch pose. Otherwise the original `shift_key_down` answer continues. |
| `inbound-packet` | `horizon_client::game::remote::Game<'_>::handle_packet` | Cancellable HEAD (the tool keeps the macro body untouched, continue or break). Runs `vape_packets::inbound` first: KnockbackDelay holds the local player's `SetEntityMotion` and every packet behind it in a bounded queue (256) and answers Break(Ok); JumpReset only observes the packet. Held packets replay in order through `handle_packet` from the client tick, from the hook when a later packet finds them due, and on disable, disconnect, world open/close, respawn and dimension change; a thread-local guard makes the hook step aside for replayed packets. Connection-critical packets (bundles, ping/pong, keep-alive, login, respawn, disconnect, configuration, transfer, cookies, custom payloads, resource packs, chunk batches, teleports) are never held: they flush the queue and pass. Afterwards Velocity observes the packet (replayed ones too) and applies its replacement right after the host's own assignment, only while the motion is still exactly the packet's. |
| `explosion-push` | `horizon_client::game::lan_particles::Game<'_>::remote_explode` | Cancellable HEAD. Velocity re-runs the host handler on a copy with the scaled push, guarded against re-entry. |
| `world-lines-hook` | `horizon_client::game::fishing::Game<'_>::fishing_line` | Cancellable HEAD. Preserves fishing lines and adds StorageESP, ESP (3D), Tracers and Trajectories lines from the runtime's `world_lines`, marked (negative width) for the always-on-top pass. |
| `name-tags-hook` | `horizon_client::game::names::Game<'_>::name_tag_geometry` | Cancellable HEAD (support module `vape_name_tags` in `game/names.rs`, so it reuses the private `name_tags`/`name_tag_quads`). Adds NameTags and ItemESP tags to the see-through set, scaled about the camera. While NameTags renders players it replays the host body without other players' tags and cancels; otherwise it continues into the host body. |
| `outbound-packet` | `horizon_client::game::remote::Game<'_>::send` | Cancellable HEAD (support module `vape_packets` in `game/remote.rs`). Classifies the serverbound packet (movement, attack with the target's hurt time, release-use-item) and drops it (NoItemRelease, PotionSaver), holds a clone with a due time (Blink, FakeLag) or continues. Held packets are released in order from the send and from the client tick hook straight to the session, bypassing the hook; the tick also sends Regen's packets. Remote servers only; `net.rs` ClientTickEnd bypasses it. |
| `send-state-rotation` | `horizon_client::game::remote::Game<'_>::remote_send_state` | MODIFY_ARG (argument 1 of `r.position_sender.update`, require 1, ordinal 0). Replaces the rotation of the tick's motion packet with the managed rotation (SilentAura) that `vape_aura::tick` stored for this tick from the client-tick hook; otherwise passes the view's rotation through. The riding `MovePlayerRot` path is not covered. |
| `keep-sprint` | `horizon_client::game::remote::Game<'_>::remote_player_attack` | Cancellable HEAD (support module `vape_combat_remote` in `game/remote.rs`). While KeepSprint is enabled it runs the host body itself behind a re-entry guard, compares the attacker's horizontal motion before and after (the host's `ATTACKER_SLOWDOWN` cut and sprint stop), then applies KeepSprint's slowdown and sprint, and cancels. Otherwise continues. |
| `overlay-hook` | `horizon_client::game::fov::Game<'_>::draw_spyglass` | HEAD (support module `vape_overlay` in `game/fov.rs`, which reaches the private FOV, bob and spin matrices). Builds the frame projection from the gizmos camera and paints the screen-space render modules (ESP 2D, Health, Arrows, NameTags equipment row, SpawnerFinder, MurdererFinder) into the HUD batch under the host HUD. |
| `esp-outline-hook` | `horizon_client::game::glowing::Game<'_>::mob_outline` | Cancellable HEAD (support module `vape_esp_outline` in `game/glowing.rs`). In ESP Outline mode it returns the ESP colour for shown players through the host's glowing-outline pass; otherwise it continues into the host body. |
| `on-top-hook` | `horizon_client::gizmos::camera_relative` | Cancellable HEAD. Moves marked lines from the frame's depth-tested set to its always-on-top set, so StorageESP shows through walls. |
| `entity-pick` | `horizon_client::game::Game<'_>::mob_hit` | Cancellable HEAD. Reach and HitBoxes replicate the host pick with a longer ray and inflated boxes; creative, serving and dedicated keep the host pick. Uses `try_with_client` so a held runtime lock falls through. |
| `remote-attack` | `horizon_client::game::remote::Game<'_>::remote_start_attack` | Reach's post-attack bookkeeping on the remote (and single-player) path; `auto-tool-attack` covers the local path. |
| `timer-advance` | `horizon_client::clock::FixedStep::advance` | Cancellable HEAD (library crate). Re-runs the clock once on elapsed time scaled by the Timer speed the frame adapter set, guarded against re-entry. |
| `no-slowdown` | `horizon_client::motion::MotionWorld<'_,'_,'_>::controlled_simulation_tick_with_sprint_window` | HEAD (library crate). Clears the item-use slowdown for the player NoSlowdown's tick marked and, with Sprint on, `item_use_slows_sprint` so the sprint is kept. (It used to target `controlled_simulation_tick`, a wrapper only tests call; the remote tick calls the sprint-window form.) |
| `inv-walk-keys` | `horizon_client::motion::MotionWorld<'_,'_,'_>::controlled_simulation_tick_with_sprint_window` | Cancellable HEAD (library crate, `adapters/inv_walk_keys.rs`, binds `keys`, `sprint_window`). InvWalk: the movement keys physically held behind a screen (marked once per tick by `vape_world_actions::tick`) are OR-merged into the screen-released keys and the tick re-dispatched; the take-once mark makes the re-entry continue. |
| `fly-move` | `horizon_client::motion::MotionWorld<'_,'_,'_>::move_self` | Cancellable HEAD (library crate). Substitutes Fly's per-tick movement vector. |
| `send-state` | `horizon_client::game::remote::Game<'_>::remote_send_state` | NoFall/Fly pre-motion: AntiCheat's MovePlayerStatus true/false pair and the ground value for this send; Step's recognised step (Speed stage, 0.45 timer; the reference's two extra positions are not sent: 26.3 kicks a second position in one client tick). |
| `send-state-ground` | `remote_send_state`, argument 2 of `r.position_sender.update` | MODIFY_ARG (`require = 1`, `ordinal = 0`). Replaces the sent `on_ground` with the pre-motion value when set. |
| `auto-totem-attack` | `horizon_client::game::remote::Game<'_>::remote_start_attack` | Cancellable HEAD (`adapters/auto_totem_attack.rs`, binds `game`), ahead of `remote-attack`. While AutoTotem's Silent open holds input (`suppressInput`: the session plus one "Silent move delay"), the attack click returns false (no swing, no attack); `remote-attack` steps aside for the same hold. |
| `auto-totem-use` | `horizon_client::game::remote::Game<'_>::remote_use` | Cancellable HEAD (`adapters/auto_totem_use.rs`). The same hold cancels the use click (no item use, block place or entity interaction). |
| `auto-totem-break` | `horizon_client::game::remote::Game<'_>::remote_continue_attack` | Cancellable HEAD (`adapters/auto_totem_break.rs`). The same hold cancels held-attack block breaking. Movement keys (forward, back, left, right, jump, sprint) are held through the movement writer (owner `AutoTotem`, priority 5). |

## Support modules

| Name | Host insertion file | Editable source |
| --- | --- | --- |
| `vape_rs` | `crates/horizon-client/src/game.rs` | `src/lib.rs` (expanded to `runtime.rs`) |
| `vape_hud` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/hud_painter.rs` |
| `vape_camera` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/camera.rs` |
| `vape_inventory` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/inventory.rs` |
| `vape_transactions` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape_rs/transactions.rs` |
| `vape_macros` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape_rs/macros.rs` |
| `vape_fast_place` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/fast_place.rs` |
| `vape_auto_fish` | `crates/horizon-client/src/game/fishing.rs` | `mixins/vape_rs/auto_fish.rs` |
| `vape_world_lines` | `crates/horizon-client/src/game/fishing.rs` | `mixins/vape_rs/world_lines.rs` |
| `vape_on_top` | `crates/horizon-client/src/gizmos.rs` | `mixins/vape_rs/on_top.rs` |
| `vape_input_delays` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/input_delays.rs` |
| `vape_parkour` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/parkour.rs` |
| `vape_combat` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/combat.rs` |
| `vape_safe_walk` | `crates/horizon-client/src/motion.rs` | `mixins/vape_rs/safe_walk.rs` |
| `vape_client_owner` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/client_owner.rs` |
| `vape_timer` | `crates/horizon-client/src/clock.rs` | `mixins/vape_rs/timer.rs` |
| `vape_motion_flags` | `crates/horizon-client/src/motion.rs` | `mixins/vape_rs/motion_flags.rs` |
| `vape_movement` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape_rs/movement.rs` |
| `vape_world_motion` | `crates/horizon-client/src/motion.rs` | `mixins/vape_rs/world_motion.rs` |
| `vape_world_actions` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape_rs/world_actions.rs` |
| `vape_smooth_text` | `crates/horizon-client/src/ui_theme.rs` | `mixins/vape_rs/smooth_text.rs` |
| `vape_aura` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/aura.rs` |
| `vape_combat_remote` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape_rs/combat_remote.rs` |
| `vape_overlay` | `crates/horizon-client/src/game/fov.rs` | `mixins/vape_rs/overlay.rs` |
| `vape_remote_tab` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape_rs/remote_tab.rs` |
| `vape_esp_outline` | `crates/horizon-client/src/game/glowing.rs` | `mixins/vape_rs/esp_outline.rs` |

Inventory worker (2026-10-31), no new hooks:

- `vape_transactions::dispatch`, in `client-frame`, runs the use-sequence executor (`Runtime::throw_frame`) for Throwpot, ThrowDebuff and AutoHeal Legit. Its effects use the host's own paths: `Game.selected`, `vape_macros::use_once`, and `remote_drop`/`drop_held`.
- The same dispatch runs InventoryFill on `Screen::Inventory` screens. It reads the physical Shift keys and the left button, finds the hovered slot with `Game::slot_at`, and shift-clicks it with `menu_ops`.

The runtime source is deterministically expanded from src/lib.rs. The published tool can't add a field to `Game<'_>`, so `vape_client_owner` keeps one Arc<Mutex<Runtime>> per thread behind the same `__horizon_mixins_vape_rs_client` accessor. Only the window thread's client Game uses it; hooks return before using it on a server or dedicated Game, and that thread drops its unused runtime when it ends. Adapters release runtime borrows before calling host actions. Session generations invalidate stale entity, slot, container and hook observations. Transient state never enters local profiles.

Native input/menu effects use Horizon’s normal action paths. Macro, fishing, tool and container ownership is coordinated per client. Server/dedicated simulation is guarded out. The GUI owns a Paused screen and completes closing on a later frame so a close click cannot attack.

BlockOverlay drafts under docs/drafts are unregistered and excluded from the runtime payload. Their outline/fill integration requirements are recorded in docs/specs/block-overlay.md. No full BlockOverlay implementation is claimed.

Current evidence and remaining validation are in [status.md](status.md) and [progress.md](progress.md). `tools/bundle.py` packages the `.hnmod`; `tools/launcher_build.py` builds it along the launcher's route (README).

Live entity snapshots come from `vape_combat::snapshot`: alive `Game::mobs` (remote players included, named through `remote_player_names`) and, when hosting LAN, the guests. Positions are interpolated at `clock.fraction()`, ids are paired with the session epoch, and line-of-sight raycasts (`MobWorld::line_of_sight`) run only when a filter needs them. The pure filter/sort/rotation code is in `src/targeting`. AimAssist and TriggerBot consume it; Velocity reads the nearby-player facing from the same mobs. TriggerBot attacks through `vape_macros::attack_once`: the remote interaction queue, or the integrated tick's attack clicks.

Movement keys have one writer: `vape_input_delays::movement`, called from `client-tick` before local and remote movement, and from `client-frame` on screen/pause/unfocused frames. It reads the host's canonical key set and toggle mappings, resolves the per-tick requests from Sprint and Parkour (`vape_parkour::edge_ahead` answers the host collision query) through `core::movement_input::Writer`, and writes only the axes that differ from the user's state, or that it owned and must hand back. Host events reach the writer through the two key-capture adapters above. Host scenarios: docs/evidence/postmerge-validation.json.

KillAura and SilentAura run from `client-tick` through `vape_aura::tick`, after TriggerBot: snapshots from `vape_combat::snapshot`, attacks by entity id through `vape_combat_remote::attack` (on a server: `Attack`, `remote_player_attack`, attack-strength reset, swing and `Punch`, in the host's order and respecting `miss_time`; locally `attack_mob` and the swing), misses through `vape_combat_remote::miss`. The shared rotation claim and managed rotation live in the runtime (`core/combat_runtime.rs`, `modules/combat/rotation.rs`).
