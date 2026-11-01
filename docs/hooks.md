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
| `gui-blur` | `horizon_client::game::Game<'_>::menu_blur` | Cancellable HEAD. ClientSettings "Blur background": while the click GUI or HUD editor is open, it returns the menu blur radius (or 5 when that setting is off), or no blur when the setting is off. |
| `scoreboard-sidebar` | `horizon_client::game::scoreboard::Game<'_>::draw_scoreboard` | Cancellable HEAD. Scoreboard skips the vanilla sidebar while the module is on; the HUD panel paints the same `hud_overlays::sidebar` draws at its own position. |
| `auto-tool-attack` | `horizon_client::game::Game<'_>::attack` | Instant weapon-swap dispatch before normal local attack. |
| `fullbright-night-vision` | `horizon_client::game::sculk::Game<'_>::night_vision_intensity` | Cancellable HEAD. Cancellable render intensity override; real potion state remains intact. |
| `clear-water` | `horizon_client::game::render_tick::Game<'_>::frame_fog` | Cancellable HEAD. Cancellable underwater fog helper retaining pinned native fog inputs/math. |
| `fish-hook-data` | `horizon_client::game::fishing::Game<'_>::remote_hook_data` | Observes only own-hook DATA_HOOKED_ENTITY into session-owned state; original packet handling continues. |

| `aim-assist-look` | `horizon_client::game::pad::Game<'_>::pad_look` | Runs after `apply_mouse_look` and before `render` each frame. AimAssist builds the live entity snapshot and turns the view through the host `mouse::Rotation::turn` (`Entity.turn`). The controller look that follows is unchanged. |
| `safe-walk-edge` | `horizon_client::motion::<MotionWorld<'_,'_,'_> as EdgeBackoffWorld>::staying_on_ground_surface` | Cancellable HEAD (library crate). Returns true for the one local player the movement writer marked for SafeWalk this tick, so the host's own `back_off_from_edge` runs without a sneak key or crouch pose. Otherwise the original `shift_key_down` answer continues. |
| `inbound-packet` | `horizon_client::game::remote::Game<'_>::handle_packet` | Cancellable HEAD (the tool keeps the macro body untouched, continue or break). Runs `vape_packets::inbound` first: KnockbackDelay holds the local player's `SetEntityMotion` and every packet behind it in a bounded queue (256) and answers Break(Ok); JumpReset only observes the packet. Held packets replay in order through `handle_packet` from the client tick, from the hook when a later packet finds them due, and on disable, disconnect, world open/close, respawn and dimension change; a thread-local guard makes the hook step aside for replayed packets. Connection-critical packets (bundles, ping/pong, keep-alive, login, respawn, disconnect, configuration, transfer, cookies, custom payloads, resource packs, chunk batches, teleports) are never held: they flush the queue and pass. Afterwards Velocity observes the packet (replayed ones too) and applies its replacement right after the host's own assignment, only while the motion is still exactly the packet's. |
| `explosion-push` | `horizon_client::game::lan_particles::Game<'_>::remote_explode` | Cancellable HEAD. Velocity re-runs the host handler on a copy with the scaled push, guarded against re-entry. |
| `world-lines-hook` | `horizon_client::game::fishing::Game<'_>::fishing_line` | Cancellable HEAD. Preserves fishing lines and adds StorageESP, ESP (3D), Tracers and Trajectories lines from the runtime's `world_lines`, Search (outlines and per-block tracers), PropHunt and Projectiles lines from `scene_lines`/`projectile_lines` (paths simulated once a tick), all marked (negative width) for the always-on-top pass, and Explosions' blast rings and Block Overlay's outline from `depth_lines`, left depth-tested. It also hands ESP Outline its colours. The entity snapshot is built only while ESP, Tracers or Trajectories is on (StorageESP reads only the rendered block entities). |
| `name-tags-hook` | `horizon_client::game::names::Game<'_>::name_tag_geometry` | Cancellable HEAD (support module `vape_name_tags` in `game/names.rs`, so it reuses the private `name_tags`/`name_tag_quads`). Adds NameTags and ItemESP tags to the see-through set, scaled about the camera. While NameTags renders players it replays the host body without other players' tags and cancels; otherwise it continues into the host body. The same hook adds StorageESP's and PropHunt's see-through filled quads (`world_quads`) and Explosions' sphere and Block Overlay's fill (`depth_quads`, depth-tested). NameTags' tags for a frame come from one runtime call (`name_tags`). |
| `outbound-packet` | `horizon_client::game::remote::Game<'_>::send` | Cancellable HEAD (support module `vape_packets` in `game/remote.rs`). Classifies the serverbound packet (movement, attack with the target's hurt time, release-use-item) and drops it (NoItemRelease, PotionSaver), holds a clone with a due time (Blink, FakeLag) or continues. Held packets are released in order from the send and from the client tick hook straight to the session, bypassing the hook; the tick also sends Regen's packets. Remote servers only; `net.rs` ClientTickEnd bypasses it. |
| `send-state-rotation` | `horizon_client::game::remote::Game<'_>::remote_send_state` | MODIFY_ARG (argument 1 of `r.position_sender.update`, require 1, ordinal 0). Replaces the rotation of the tick's motion packet with the managed rotation (SilentAura) that `vape_aura::tick` stored for this tick from the client-tick hook; otherwise passes the view's rotation through. The riding `MovePlayerRot` path is not covered. |
| `keep-sprint` | `horizon_client::game::remote::Game<'_>::remote_player_attack` | Cancellable HEAD (support module `vape_combat_remote` in `game/remote.rs`). While KeepSprint is enabled it runs the host body itself behind a re-entry guard, compares the attacker's horizontal motion before and after (the host's `ATTACKER_SLOWDOWN` cut and sprint stop), then applies KeepSprint's slowdown and sprint, and cancels. Otherwise continues. |
| `overlay-hook` | `horizon_client::game::fov::Game<'_>::draw_spyglass` | HEAD (support module `vape_overlay` in `game/fov.rs`, which reaches the private FOV, bob and spin matrices). Builds the frame projection from the gizmos camera and paints the screen-space render modules (ESP 2D, Health, Arrows, NameTags equipment row with the host foil, SpawnerFinder, MurdererFinder, BedPlates, Indicators) into the HUD batch under the host HUD. The entity snapshot is built only when ESP is in 2D mode, NameTags shows equipment, or Arrows or MurdererFinder is on; Indicators' projectiles and BedPlates' beds are gathered only while those are on. It also sends MurdererFinder's next Callout, if one is due, through the host's own chat send: `Game::send_chat` on a server, `Game::handle_chat_input` otherwise. |
| `esp-outline-hook` | `horizon_client::game::glowing::Game<'_>::mob_outline` | Cancellable HEAD (support module `vape_esp_outline` in `game/glowing.rs`). In ESP Outline mode it returns the ESP colour for shown players through the host's glowing-outline pass; otherwise it continues into the host body. |
| `on-top-hook` | `horizon_client::gizmos::camera_relative` | Cancellable HEAD. Moves marked lines from the frame's depth-tested set to its always-on-top set, so StorageESP, ESP, Tracers, Search, PropHunt and Projectiles show through walls; a line starting at `VIEW_CENTRE` (NaN) starts at the real camera plus its look vector. |
| `entity-pick` | `horizon_client::game::Game<'_>::mob_hit` | Cancellable HEAD. Reach and HitBoxes replicate the host pick with a longer ray and inflated boxes; creative, serving and dedicated keep the host pick. Uses `try_with_client` so a held runtime lock falls through. |
| `remote-attack` | `horizon_client::game::remote::Game<'_>::remote_start_attack` | Reach's post-attack bookkeeping on the remote (and single-player) path; `auto-tool-attack` covers the local path. |
| `timer-advance` | `horizon_client::clock::FixedStep::advance` | Cancellable HEAD (library crate). Re-runs the clock once on elapsed time scaled by the Timer speed the frame adapter set, guarded against re-entry. |
| `no-slowdown` | `horizon_client::motion::MotionWorld<'_,'_,'_>::controlled_simulation_tick_with_sprint_window` | HEAD (library crate). Clears the item-use slowdown for the player NoSlowdown's tick marked and, with Sprint on, `item_use_slows_sprint` so the sprint is kept. (It used to target `controlled_simulation_tick`, a wrapper only tests call; the remote tick calls the sprint-window form.) |
| `inv-walk-keys` | `horizon_client::motion::MotionWorld<'_,'_,'_>::controlled_simulation_tick_with_sprint_window` | Cancellable HEAD (library crate, `adapters/inv_walk_keys.rs`, binds `keys`, `sprint_window`). InvWalk: the movement keys physically held behind a screen (marked once per tick by `vape_world_actions::tick`) are OR-merged into the screen-released keys and the tick re-dispatched; the take-once mark makes the re-entry continue. |
| `fly-move` | `horizon_client::motion::MotionWorld<'_,'_,'_>::move_self` | Cancellable HEAD (library crate). Substitutes Fly's per-tick movement vector. |
| `send-state` | `horizon_client::game::remote::Game<'_>::remote_send_state` | NoFall/Fly pre-motion: AntiCheat's MovePlayerStatus true/false pair and the ground value for this send; Step's recognised step (Speed stage, 0.45 timer; the reference's two extra positions are not sent: 26.3 kicks a second position in one client tick). |
| `send-state-ground` | `remote_send_state`, argument 2 of `r.position_sender.update` | MODIFY_ARG (`require = 1`, `ordinal = 0`). Replaces the sent `on_ground` with the pre-motion value when set. |
| `hit-select-attack` | `horizon_client::game::remote::Game<'_>::remote_start_attack` | Cancellable HEAD, priority 110 (before `remote-attack`). HitSelect's `EventClickMouse`: a cancelled attack click does nothing (returns false: no instant block break); a passed one runs `ClientSettings.applyAttackEffects`. |
| `hit-select-use` | `horizon_client::game::remote::Game<'_>::remote_use` | Cancellable HEAD. HitSelect Normal holds back use and right clicks until the next tick (`EventRightClickMouse`/`EventPlayerUseItem`). |
| `hit-select-continue-attack` | `horizon_client::game::remote::Game<'_>::remote_continue_attack` | Cancellable HEAD. HitSelect Normal holds back block hitting (`EventSendClickBlockToController`). |
| `block-hit-mouse` | `horizon_client::game::Game<'_>::play_mouse` | Cancellable HEAD, priority 90 (after `mouse-bind`). BlockHit's `EventMouseButton`: Manual's block at an attack press; Auto/Predict swallow a use press while attacking with a sword. |
| `block-hit-send` | `horizon_client::game::remote::Game<'_>::send` | Cancellable HEAD, priority 90 (after `outbound-packet`, the reference's LOWEST). BlockHit Lag holds the release-use packet and what follows it until its delay ends or the target is lost; the held packets go out through the session, past the hooks. |

| `auto-totem-attack` | `horizon_client::game::remote::Game<'_>::remote_start_attack` | Cancellable HEAD (`adapters/auto_totem_attack.rs`, binds `game`), ahead of `remote-attack`. While AutoTotem's Silent open holds input (`suppressInput`: the session plus one "Silent move delay"), the attack click returns false (no swing, no attack); `remote-attack` steps aside for the same hold. |
| `auto-totem-use` | `horizon_client::game::remote::Game<'_>::remote_use` | Cancellable HEAD (`adapters/auto_totem_use.rs`). The same hold cancels the use click (no item use, block place or entity interaction). |
| `auto-totem-break` | `horizon_client::game::remote::Game<'_>::remote_continue_attack` | Cancellable HEAD (`adapters/auto_totem_break.rs`). The same hold cancels held-attack block breaking. Movement keys (forward, back, left, right, jump, sprint) are held through the movement writer (owner `AutoTotem`, priority 5). |
| `auto-heal-pre-motion` | `horizon_client::game::remote::Game<'_>::remote_send_state` | HEAD (`adapters/auto_heal_pre_motion.rs`, binds `game`), ahead of `send-state`. AutoHeal Silent with Potions (`EventPreMotion`): Replace's swap into "Slot" through `remote_menu_ops` (`Input::Swap`, no screen), and, when a potion is needed on the ground and the Delay has passed, this packet's pitch 88.99 through the managed rotation (`vape_combat_remote::set_rotation`; the camera is unchanged). |
| `auto-heal-post-motion` | `horizon_client::game::native_hooks::Game<'_>::native_tick` | HEAD (`adapters/auto_heal_post_motion.rs`, binds `game`). `tick_remote` calls the empty `native_tick` right after `tick_remote_level`, which returns as soon as `remote_send_state` has sent the motion packet, so this is the first point after it (the audit takes no RETURN kind). Skips the server-side `Game`. AutoHeal PostMotion: puts back the managed rotation, then uses the pending potion silently (SetCarriedItem to the slot, `remote_use_item` with the sent look, SetCarriedItem back; `game.selected` and the camera are restored). |
| `target-info-pot-use` | `horizon_client::game::potions::Game<'_>::use_potion_item` | HEAD (`adapters/target_info_pot_use.rs`). Target Info's pots strip: a heal potion the local player drinks counts against it (`vape_hud::target_heal_used`). |
| `target-info-pot-remote-use` | `horizon_client::game::remote::Game<'_>::remote_use_item` | HEAD (`adapters/target_info_pot_remote_use.rs`). The same on a server, from the held stack of the used hand. |
| `target-info-pot-launch` | `horizon_client::game::potions::Game<'_>::launch_thrown` | HEAD (`adapters/target_info_pot_launch.rs`). A heal splash potion the local player throws counts against it (`vape_hud::target_heal_joined`). |
| `target-info-pot-remote-data` | `horizon_client::game::remote_projectiles::Game<'_>::remote_projectile_data` | HEAD (`adapters/target_info_pot_remote_data.rs`). A remote thrown potion's item (entity data index 8) counts for the target when it is a heal pot (`vape_hud::target_remote_potion`). |

| `pearl-use-item` | `horizon_client::game::remote::Game<'_>::remote_use_item` | HEAD (`adapters/pearl_use_item.rs`, binds `hand`; support module `vape_pearls` in `game/remote.rs`). Tells the pearl runtime that this client used an ender pearl from that hand, so AutoPearl and PearlCatch track their own throw. Serving and dedicated are guarded out. |
| `pearl-throw-item` | `horizon_client::game::creatures::Game<'_>::throw_item` | HEAD (`adapters/pearl_throw_item.rs`, binds `stack`). The same notice on the single-player throw path; a LAN guest's turn (`lan_acting`) is not this client's throw. |
| `freecam-camera` | `horizon_client::game::perspective::Game<'_>::player_camera_entity` | Cancellable HEAD (`adapters/freecam_camera.rs`, binds `&mut Game`). One dispatcher: Freecam's camera entity when the camera is free; otherwise, while Freelook is engaged, the host's own camera entity (asked for again under a re-entry guard) turned to the freelook rotation, with the camera type set to third person before `camera_view` reads it. |
| `freecam-turn` | `horizon_client::game::perspective::Game<'_>::turn_player` | Cancellable HEAD (`adapters/freecam_turn.rs`, binds `xo`, `yo`). One dispatcher: the turn goes to Freecam's camera, else to Freelook (converted to cursor counts by `degrees_per_count`), and the player does not turn; otherwise the host turns the player. |
| `freecam-body` | `horizon_client::game::perspective::Game<'_>::own_body_in_view` | Cancellable HEAD (`adapters/freecam_body.rs`). Spawn Fake: the player's real body is drawn in first person while the camera is free. |

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
| `vape_teams` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape_rs/teams.rs` |
| `vape_hit_select` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/hit_select.rs` |
| `vape_block_hit` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/block_hit.rs` |

| `vape_modern_combat` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/modern_combat.rs` |

Combat-b worker (2026-10-31, round 2), no new hooks, one new support module `vape_modern_combat`:

- `mouse-bind` lets HitSwap, AutoMace and ShieldBreaker see a physical left press (swallowed or not) through `vape_modern_combat::press`; a cancelled press goes no further.
- `client-tick` runs `vape_modern_combat::tick` (slot steps, synthetic attacks, swing releases) after TriggerBot. `vape_modern_combat::trigger_assist` gives TriggerBot its cooldown and Shield check bypasses, and `request(Source)` routes TriggerBot's and the AutoClicker's synthetic attacks through the dispatch.
- `auto-tool-attack` and `remote-attack` call `vape_modern_combat::post_attack` after the host's own attack (AutoMace's two-tick swap back, Stun slam's follow-up).
- AntiFireball (`src/core/anti_fireball_runtime.rs`) has no host hook yet.

| `vape_pearls` | `crates/horizon-client/src/game/remote.rs` | `mixins/vape_rs/pearls.rs` |
| `vape_freecam` | `crates/horizon-client/src/game.rs` | `mixins/vape_rs/freecam.rs` |

Inventory worker (2026-10-31), no new hooks:

- `vape_transactions::dispatch`, in `client-frame`, runs the use-sequence executor (`Runtime::throw_frame`) for Throwpot, ThrowDebuff and AutoHeal Legit. Its effects use the host's own paths: `Game.selected`, `vape_macros::use_once`, and `remote_drop`/`drop_held`.
- The same dispatch runs InventoryFill on `Screen::Inventory` screens. It reads the physical Shift keys and the left button, finds the hovered slot with `Game::slot_at`, and shift-clicks it with `menu_ops`.

The runtime source is deterministically expanded from src/lib.rs. The published tool can't add a field to `Game<'_>`, so `vape_client_owner` keeps one Arc<Mutex<Runtime>> per thread behind the same `__horizon_mixins_vape_rs_client` accessor. Only the window thread's client Game uses it; hooks return before using it on a server or dedicated Game, and that thread drops its unused runtime when it ends. Adapters release runtime borrows before calling host actions. Session generations invalidate stale entity, slot, container and hook observations. Transient state never enters local profiles.

Native input/menu effects use Horizon’s normal action paths. Macro, fishing, tool and container ownership is coordinated per client. Server/dedicated simulation is guarded out. The GUI owns a Paused screen and completes closing on a later frame so a close click cannot attack.

BlockOverlay drafts under docs/drafts are unregistered and excluded from the runtime payload. Their outline/fill integration requirements are recorded in docs/specs/block-overlay.md. No full BlockOverlay implementation is claimed. The registered Block Overlay (`src/modules/render/block_overlay.rs`, render round 2) draws through `world-lines-hook` (outline) and `name-tags-hook` (fill); the drafts stay unused.

Current evidence and remaining validation are in [status.md](status.md) and [progress.md](progress.md). `tools/bundle.py` packages the `.hnmod`; `tools/launcher_build.py` builds it along the launcher's route (README).

Live entity snapshots come from `vape_combat::snapshot`: alive `Game::mobs` (remote players included, named through `remote_player_names`) and, when hosting LAN, the guests. Positions are interpolated at `clock.fraction()`, ids are paired with the session epoch, and line-of-sight raycasts (`MobWorld::line_of_sight`) run only when a filter needs them. The pure filter/sort/rotation code is in `src/targeting`. AimAssist and TriggerBot consume it; Velocity reads the nearby-player facing from the same mobs. TriggerBot attacks through `vape_macros::attack_once`: the remote interaction queue, or the integrated tick's attack clicks.

Movement keys have one writer: `vape_input_delays::movement`, called from `client-tick` before local and remote movement, and from `client-frame` on screen/pause/unfocused frames. It reads the host's canonical key set and toggle mappings, resolves the per-tick requests from Sprint and Parkour (`vape_parkour::edge_ahead` answers the host collision query) through `core::movement_input::Writer`, and writes only the axes that differ from the user's state, or that it owned and must hand back. Host events reach the writer through the two key-capture adapters above. Host scenarios: docs/evidence/postmerge-validation.json.

KillAura and SilentAura run from `client-tick` through `vape_aura::tick`, after TriggerBot: snapshots from `vape_combat::snapshot`, attacks by entity id through `vape_combat_remote::attack` (on a server: `Attack`, `remote_player_attack`, attack-strength reset, swing and `Punch`, in the host's order and respecting `miss_time`; locally `attack_mob` and the swing), misses through `vape_combat_remote::miss`. The shared rotation claim and managed rotation live in the runtime (`core/combat_runtime.rs`, `modules/combat/rotation.rs`).

### 2026-10-31: combat part A (worker round 2) counts

With the combat part A branch (`t3/1e32141e`) on main `97dbaa7`, stage and audit count **49 hooks (47 HEAD + 2 MODIFY_ARG) and 32 support modules**. The branch adds five hooks (`hit-select-attack`, `hit-select-use`, `hit-select-continue-attack`, `block-hit-mouse`, `block-hit-send`) and three support modules (`vape_teams`, `vape_hit_select`, `vape_block_hit`), all listed in the tables above. The header count at the top of this file predates both.

Movement worker B (2026-10-31, worker round 2): 5 hooks (`pearl-use-item`, `pearl-throw-item`, `freecam-camera`, `freecam-turn`, `freecam-body`) and 2 support modules (`vape_pearls`, `vape_freecam`). `client-tick` calls `vape_pearls::tick` and `vape_freecam::tick`.

- `vape_pearls::tick` feeds the pearl runtime (the pearls in the world, players, the hotbar, a collision-only block ray for the landing prediction). It answers with slot switches through `Game.selected` and throws through the host's use-item path. Rotations go through the shared rotation claim (`Rotations/Shared`).
- `vape_freecam::tick` runs Freelook's post tick (the queued Toggle, Hold's bind state, the perspective put back) for every client tick. While Freecam is wanted, it also gathers the physical movement keys and the arrow keys and holds the player's rotation. The movement writer releases the seven movement keys and walks the player with the arrows under Move Fake.
- The movement writer drops a socket-held key that is no longer physically held (`socket_held`/`socket_released`), so a control-socket key-up reaches the writer.
- Freelook has no injection of its own; it shares `client-tick`, `freecam-camera` and `freecam-turn` with Freecam (Freecam is asked first).

Movement-a worker (2026-10-31), no new hooks or support modules (still 44 hooks and 29 support modules):

- `vape_world_actions::tick` (client-tick HEAD) now runs the shared rotation manager (`Runtime::rotation_tick_at`, the RotationManager port in `src/core/rotation_runtime.rs`) for every visible owner: WindCharge, Scaffold Legit, GodBridge and TellyBridge. The old movement-only rotation is gone.
- Before the rotation, `scaffold_bridge_tick` runs GodBridge/TellyBridge (`Runtime::scaffold_bridge_tick`). It reads the inventory, the crosshair block (`crosshair_block`, the 26.3 `normalReachRayTrace`), the right button, the accumulated mouse and the user's movement keys. Its effects go through host paths: `Game.selected`, and `vape_macros::use_once` for `useItemKey.onTick(1)`.
- The bridge modes' movement keys (the movement-task port, `src/modules/movement/movement_task.rs`) and their movement-input and right-click locks go through the one movement writer (`vape_input_delays::movement`). Sprint reads the movement-input lock there.

Client foundation worker (2026-10-31), round 3: one new hook (45 injections and 29 support modules in the manifest after merging main 97dbaa7).

| ID | Exact target | Behavior |
| --- | --- | --- |
| `gui-no-pause` | `horizon_client::game::Game<'_>::pauses_game` | HEAD, cancellable. Returns `Break(false)` while the click GUI or the HUD editor owns the host's `Paused` screen (`Runtime::ui_screen_owned`) and the vanilla pause menu is not open. Otherwise it continues to the host's rule. |

Changes to existing hooks and support modules:

- `gui-render` (`adapters/gui_render.rs`), outside the HUD editor:
  - it no longer clears the frame or draws a dim;
  - it calls `vape_hud::drop_paused_title`, then clears the Text GUI row boxes for this frame;
  - it draws the `vape_hud::GUI_OVERLAYS` panels (`hud.text-gui`, `hud.target-info`) over the GUI, and the notifications last.
  - The HUD editor path keeps its clear, its 0x33000000 dim and its preview panels.
- `client-hud` (`adapters/hud.rs`) takes a snapshot while playing and also while the GUI shows the HUD (`Runtime::gui_shows_hud`). In that state it skips the `GUI_OVERLAYS` panels and the notifications, which `gui-render` draws on top.
- `vape_hud` (`hud_painter.rs`):
  - new `GUI_OVERLAYS`;
  - new `drop_paused_title`: removes the host's "Game Paused" title only when its vertices match a probe rendering byte for byte;
  - `text_gui` returns its row boxes for Click disable.

Mixin helper pitfall found during this round: in the staged `debug_keys.rs`, the helper's `.filter(|p| crate::game::vape_hud::GUI_OVERLAYS.contains(&p.module))` closure body had been wrapped in a block. The staged statement then no longer matched the statement the audit builds from the helper source, and the build failed with "unexpected transformed AST changes". The helper now uses a plain `for`/`if`.

Placement worker (2026-10-31, worker round 5), no new hooks or support modules (64 hooks and 35 support modules, as on main 4c8d6e6):

- `vape_world_actions::tick` (client-tick HEAD) now runs `scaffold_bridge_tick` **after** the shared rotation tick (`Runtime::rotation_tick_at`), with the body and the crosshair block (`crosshair_block`) read after the view turned. This replaces "Before the rotation" in the movement-a notes above. The reference bus dispatches LOWEST (`RotationManager.onTick`) before NORMAL (the bridge modes).
- The rotation driver advances `rotation::updates_per_tick(sensitivity)` controller updates per tick (the reference's sensitivity time scale) instead of a fixed 50.
- `client-input` (`adapters/input.rs`) releases a socket-held movement axis on the key's Release event. The movement writer no longer infers a socket release from `physical_keys`; this replaces the `socket_held`/`socket_released` note in the movement B section.
- `vape_fast_place::fast_place` (`fast_place.rs`) gates on what `FastPlace.onTick` gates on: the local player and the right-click use claim (plus the port's own server-thread and dedicated guards). The earlier window-focus and open-screen gates are gone; the host reads `use_delay` only when it handles the use key.
