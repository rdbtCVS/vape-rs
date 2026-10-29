// Live entity snapshots for combat modules and the AimAssist frame adapter.
// Snapshots are owned values for one frame: positions interpolated at the
// render partial tick, ids paired with the session epoch.
use crate::game::vape_rs::core::{ModuleId, lifecycle::EntityHandle, with_client};
use crate::game::vape_rs::inventory::item_limit::HeldItem;
use crate::game::vape_rs::modules::combat::aim_assist::Frame;
use crate::game::vape_rs::targeting::entities::{Bounds, EntityClass, EntityView, PlayerView};
use crate::game::{Game, Screen};

fn lerp(previous: [f64; 3], current: [f64; 3], partial: f64) -> [f64; 3] {
    [0, 1, 2].map(|i| previous[i] + (current[i] - previous[i]) * partial)
}

fn item_score(game: &Game<'_>, item: Option<&str>, armor: bool) -> f64 {
    let Some(info) = item.and_then(|id| game.res.items().get(id)) else {
        return 0.0;
    };
    if !armor {
        return f64::from(info.attack_damage);
    }
    // Vanilla armor modifiers name the piece's slot group; see vape_transactions::score.
    info.modifiers
        .iter()
        .filter(|(_, _, _, _, group)| {
            group == "armor" || group == "any" || info.equip_slot.as_deref() == Some(group.as_str())
        })
        .map(|(attribute, _, amount, _, _)| match attribute.as_str() {
            "minecraft:armor" | "armor" => *amount,
            "minecraft:armor_toughness" | "armor_toughness" => amount * 0.25,
            _ => 0.0,
        })
        .sum()
}

fn eye(game: &Game<'_>) -> Option<horizon_core::geometry::Vec3> {
    let p = game.player.as_ref()?;
    Some(horizon_core::geometry::Vec3::new(
        p.position.x,
        p.position.y + f64::from(p.dimensions.eye_height),
        p.position.z,
    ))
}

/// Living entities other than the local player: mobs (including remote
/// players) and, when hosting LAN, the guests. `sight` limits the
/// line-of-sight raycasts to entities within that distance.
pub(super) fn snapshot(game: &Game<'_>, epoch: crate::game::vape_rs::core::lifecycle::Epoch, sight: Option<f64>) -> Vec<EntityView> {
    let partial = game.clock.fraction();
    let names = game.remote_player_names();
    let eye = eye(game);
    let world = sight.map(|_| game.mob_world_base(&[]));
    let visible = |at: [f64; 3], eye_height: f64| -> bool {
        let (Some(world), Some(from), Some(range)) = (world.as_ref(), eye, sight) else {
            return true;
        };
        let to = horizon_core::geometry::Vec3::new(at[0], at[1] + eye_height, at[2]);
        let d = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2) + (to.z - from.z).powi(2)).sqrt();
        d > range || world.line_of_sight(from, to)
    };
    let mut out = Vec::new();
    for mob in game.mobs.iter().filter(|m| m.alive()) {
        let class = EntityClass::classify(&mob.kind.id, &mob.kind.category);
        let current = [mob.position.x, mob.position.y, mob.position.z];
        let position = lerp([mob.previous.x, mob.previous.y, mob.previous.z], current, partial);
        let b = mob.bounds();
        let (lo, hi) = (b.min_position(), b.max_position());
        let e = &mob.extra;
        let worn = [e.head.as_deref(), e.chest.as_deref(), e.legs.as_deref(), e.feet.as_deref()];
        let name = if class == EntityClass::Player {
            names.iter().find(|(id, _)| *id == mob.id).map(|(_, n)| n.clone()).unwrap_or_default()
        } else {
            mob.custom_name.clone().unwrap_or_default()
        };
        out.push(EntityView {
            handle: EntityHandle { epoch, id: mob.id as i32 },
            class,
            name,
            position,
            tick_position: current,
            bounds: Bounds { min: [lo.x, lo.y, lo.z], max: [hi.x, hi.y, hi.z] },
            health: mob.health,
            invisible: mob.invisible,
            holding: e.main_hand.is_some(),
            armored: worn.iter().any(Option::is_some),
            armor: worn.iter().map(|w| item_score(game, *w, true)).sum(),
            threat: item_score(game, e.main_hand.as_deref(), false),
            visible: visible(current, mob.kind.eye_height),
            vehicle: Some(mob.id) == game.riding,
            blocking: e.blocking,
        });
    }
    if let Some(host) = &game.lan {
        for guest in host.hub.guests.iter().filter(|g| host.hub.here(g) && g.loaded && !g.dead) {
            let worn: Vec<_> = guest.armor.iter().map(|a| a.as_ref().map(|s| &*s.item)).collect();
            let held = guest.inventory.get(guest.selected).and_then(|s| s.as_ref());
            let min = [guest.position[0] - 0.3, guest.position[1], guest.position[2] - 0.3];
            out.push(EntityView {
                handle: EntityHandle { epoch, id: guest.entity_id },
                class: EntityClass::Player,
                name: guest.name.clone(),
                position: lerp(guest.previous_position, guest.position, partial),
                tick_position: guest.position,
                bounds: Bounds { min, max: [min[0] + 0.6, min[1] + 1.8, min[2] + 0.6] },
                health: guest.health,
                invisible: false,
                holding: held.is_some(),
                armored: worn.iter().any(Option::is_some),
                armor: worn.iter().map(|w| item_score(game, *w, true)).sum(),
                threat: item_score(game, held.map(|s| &*s.item), false),
                visible: visible(guest.position, 1.62),
                vehicle: false,
                blocking: false,
            });
        }
    }
    out
}

fn held(game: &Game<'_>) -> HeldItem {
    let stack = game.inventory.get(game.selected).and_then(|s| s.as_ref());
    let info = stack.and_then(|s| game.res.items().get(&s.item));
    HeldItem {
        id: stack.map(|s| s.item.trim_start_matches("minecraft:").to_owned()),
        display_name: stack
            .map(|s| horizon_client::stations::hover_name(&|id| game.pack.names.display_item(id), s))
            .unwrap_or_default(),
        block: info.is_some_and(|i| i.block.is_some()),
        food: info.is_some_and(|i| i.food.is_some()),
        potion: stack.is_some_and(|s| s.item.ends_with("potion")),
        slot: game.selected,
    }
}

fn playing(game: &Game<'_>) -> bool {
    game.focused
        && game.screen == Screen::Playing
        && game.title.is_none()
        && game.sleeping.is_none()
        && !game.dialog_open()
        && game.server_packs.prompt.is_none()
        && !game.spectator()
        && game.player.as_ref().is_some_and(|p| p.health.health() > 0.0)
}

/// After the host's own mouse look and before rendering: AimAssist turns the
/// view through the same `Entity.turn` the mouse uses.
pub(super) fn aim_assist(game: &mut Game<'_>) {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (active, epoch, walls, range) = with_client(&client, |runtime| {
        let walls = runtime.settings().get("combat.aim-assist.targets.ignore-walls").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let range = runtime.settings().get("combat.aim-assist.distance").and_then(serde_json::Value::as_f64).unwrap_or(8.0);
        (runtime.aim_assist_active(), runtime.session().epoch(), walls, range)
    });
    let Some(player) = game.player.as_ref() else { return; };
    if !active || game.title.is_some() {
        return;
    }
    let partial = game.clock.fraction();
    let current = [player.position.x, player.position.y, player.position.z];
    let previous = [game.previous_position.x, game.previous_position.y, game.previous_position.z];
    let strafe = f32::from(u8::from(game.keys.contains(&winit::keyboard::KeyCode::KeyA)))
        - f32::from(u8::from(game.keys.contains(&winit::keyboard::KeyCode::KeyD)));
    // The crosshair is on a block when the block hit is nearer than any mob.
    let aiming_block = game.pick().is_some_and(|hit| {
        let eye = game.eye_position().unwrap_or_default();
        let d = (0..3).map(|i| (hit.point[i] - eye[i]).powi(2)).sum::<f64>().sqrt();
        game.mob_hit().is_none_or(|(_, m)| d < m)
    });
    let frame = Frame {
        player: PlayerView {
            position: lerp(previous, current, partial),
            tick_position: current,
            yaw: game.yaw,
            pitch: game.pitch,
            strafe,
        },
        entities: snapshot(game, epoch, walls.then_some(range + 1.0)),
        held: held(game),
        attack_down: game.left,
        aiming_block,
        playing: playing(game),
        movement_locked: false,
        sensitivity: game.settings.sensitivity,
        elapsed_ms: 0,
    };
    let (x, y) = with_client(&client, |runtime| {
        if runtime.enabled(ModuleId::AimAssist) { runtime.aim_assist_frame(frame) } else { (0.0, 0.0) }
    });
    if x == 0.0 && y == 0.0 {
        return;
    }
    let mut rotation = horizon_client::mouse::Rotation { x_rot: game.pitch, y_rot: game.yaw, x_rot_o: game.pitch, y_rot_o: game.yaw };
    rotation.turn(x, y);
    game.yaw = rotation.y_rot;
    game.pitch = rotation.x_rot;
}

/// The entity under the crosshair before any block: a mob (remote players
/// included) or, when hosting LAN, a guest. `miss` is true when the view hits
/// neither an entity nor a block.
fn crosshair(game: &Game<'_>, entities: &[EntityView]) -> (Option<EntityView>, bool) {
    let mob = game.mob_hit().filter(|(_, d)| game.before_block(*d)).map(|(i, d)| (game.mobs[i].id as i32, d));
    let guest = game.guest_hit().filter(|(_, d)| game.before_block(*d)).and_then(|(conn, d)| {
        let host = game.lan.as_ref()?;
        host.hub.guests.iter().find(|g| g.conn == conn).map(|g| (g.entity_id, d))
    });
    let best = match (mob, guest) {
        (Some(m), Some(g)) => Some(if g.1 < m.1 { g } else { m }),
        (m, g) => m.or(g),
    };
    let entity = best.and_then(|(id, _)| entities.iter().find(|e| e.handle.id == id).cloned());
    let miss = entity.is_none() && game.pick().is_none();
    (entity, miss)
}

pub(super) fn trigger_tick(game: &Game<'_>, now_ms: u64, epoch: crate::game::vape_rs::core::lifecycle::Epoch, walls: bool) -> crate::game::vape_rs::modules::combat::trigger_bot::Tick {
    use crate::game::vape_rs::modules::combat::trigger_bot::{Cooldown, Tick};
    let entities = snapshot(game, epoch, walls.then_some(6.0));
    let (crosshair, crosshair_miss) = crosshair(game, &entities);
    let player = game.player.as_ref();
    Tick {
        now_ms,
        hurt_time: player.map_or(0, |p| p.health.hurt_animation_time),
        attack_down: game.left,
        playing: playing(game),
        held: held(game),
        on_ground: player.is_some_and(|p| p.ground.on_ground),
        vertical_motion: player.map_or(0.0, |p| p.position.y - game.previous_position.y),
        crosshair,
        crosshair_miss,
        // The client's LivingEntity.attackStrengthTicker, which vanilla's
        // getCooledAttackStrength reads (the integrated server keeps its own).
        cooldown: Cooldown {
            ticks: game.hand_strength.ticker as f32,
            delay: horizon_client::attack_strength::delay(game.main_hand_attack_speed()),
        },
        // canBypassCooldown needs HitSwap/AutoMace or the axe-without-HitSwap
        // case (RotationUtil.u), which is not ported yet: never bypass.
        cooldown_bypass: false,
    }
}

/// Client tick HEAD: one TriggerBot step and its synthetic attack click.
pub(super) fn trigger_bot(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (active, epoch, now, walls) = with_client(&client, |runtime| {
        let walls = runtime.settings().get("combat.trigger-bot.targets.ignore-walls").and_then(serde_json::Value::as_bool).unwrap_or(false);
        (runtime.trigger_bot_active(), runtime.session().epoch(), runtime.now_ms(), walls)
    });
    if !active || game.player.is_none() || game.title.is_some() {
        return;
    }
    let tick = trigger_tick(game, now, epoch, walls);
    let effects = with_client(&client, |runtime| runtime.trigger_bot_tick(&tick));
    if effects.attack && tick.playing {
        crate::game::remote::vape_macros::attack_once(game);
    }
}

/// The physical attack button changed. True swallows the activation click.
pub(super) fn trigger_bot_input(game: &Game<'_>, pressed: bool) -> bool {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (enabled, epoch, now) = with_client(&client, |runtime| {
        (runtime.enabled(ModuleId::TriggerBot), runtime.session().epoch(), runtime.now_ms())
    });
    if !enabled || game.player.is_none() {
        return false;
    }
    let tick = trigger_tick(game, now, epoch, false);
    with_client(&client, |runtime| runtime.trigger_bot_input(&tick, pressed))
}

/// `RotationUtil.H/F` for Velocity: other non-friend players within 6 blocks
/// looking at us, and us looking at one of them (integer yaw under 60).
fn facing(game: &Game<'_>, friends: &[String]) -> crate::game::vape_rs::modules::combat::velocity::Facing {
    use crate::game::vape_rs::targeting::entities::{distance, int_yaw_difference};
    let mut out = crate::game::vape_rs::modules::combat::velocity::Facing::default();
    let Some(player) = game.player.as_ref() else { return out; };
    let me = [player.position.x, player.position.y, player.position.z];
    let me_view = PlayerView { position: me, tick_position: me, yaw: game.yaw, pitch: game.pitch, strafe: 0.0 };
    let names = game.remote_player_names();
    for mob in game.mobs.iter().filter(|m| m.alive() && &*m.kind.id == "minecraft:player") {
        let name = names.iter().find(|(id, _)| *id == mob.id).map_or("", |(_, n)| n.as_str());
        if friends.iter().any(|f| f.eq_ignore_ascii_case(name)) {
            continue;
        }
        let at = [mob.position.x, mob.position.y, mob.position.z];
        if distance(me, at) >= 6.0 {
            continue;
        }
        let them = PlayerView { position: at, tick_position: at, yaw: mob.yaw, pitch: 0.0, strafe: 0.0 };
        out.player_facing_target |= int_yaw_difference(&them, me) < 60;
        out.target_facing_player |= int_yaw_difference(&me_view, at) < 60;
    }
    out
}

/// A Velocity replacement for the local player's last motion packet. The
/// inbound hook only observes (the host's packet handler holds macros), so
/// the replacement lands right after the host's own assignment: at the next
/// packet or tick, and only if the motion is still exactly what the packet set.
fn apply_motion_override(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let Some((replacement, received)) = with_client(&client, |runtime| runtime.take_motion_override()) else { return; };
    if let Some(player) = game.player.as_mut()
        && [player.velocity.x, player.velocity.y, player.velocity.z] == received
    {
        player.velocity = horizon_core::geometry::Vec3::new(replacement[0], replacement[1], replacement[2]);
    }
}

/// Inbound packet boundary (`handle_packet` HEAD, observe-only). Velocity
/// decides on the local player's motion packets here.
pub(super) fn inbound(game: &mut Game<'_>, packet: &horizon_protocol::packets::play::Clientbound) {
    use crate::game::vape_rs::modules::combat::velocity::Decision;
    use horizon_protocol::packets::play::Clientbound as C;
    apply_motion_override(game);
    let C::SetEntityMotion(p) = packet else { return; };
    if !game.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (active, friends) = with_client(&client, |runtime| (runtime.velocity_active(), runtime.friend_keys()));
    if !active || game.player.is_none() {
        return;
    }
    let facing = facing(game, &friends);
    let water = game.player.as_ref().is_some_and(|pl| pl.fluids.was_touching_water);
    with_client(&client, |runtime| {
        if let Decision::Replace(m) = runtime.velocity_motion(facing, water, p.velocity) {
            runtime.set_motion_override(m, p.velocity);
        }
    });
}

/// `remote_explode` HEAD (cancellable): Velocity scales the explosion push by
/// running the host's handler on a copy with the replaced push.
pub(super) fn explosion(game: &mut Game<'_>, p: &horizon_protocol::packets::play::Explode) -> core::ops::ControlFlow<()> {
    use crate::game::vape_rs::modules::combat::velocity::Decision;
    use core::ops::ControlFlow;
    let Some(push) = p.knockback else { return ControlFlow::Continue(()); };
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (active, reentry, friends) = with_client(&client, |runtime| {
        (runtime.velocity_active(), runtime.take_explosion_reentry(), runtime.friend_keys())
    });
    if reentry || !active || game.player.is_none() {
        return ControlFlow::Continue(());
    }
    let facing = facing(game, &friends);
    let water = game.player.as_ref().is_some_and(|pl| pl.fluids.was_touching_water);
    let decision = with_client(&client, |runtime| runtime.velocity_explosion(facing, water, push));
    let Decision::Replace(m) = decision else { return ControlFlow::Continue(()); };
    let mut changed = p.clone();
    changed.knockback = Some(m);
    with_client(&client, |runtime| runtime.set_explosion_reentry());
    game.remote_explode(&changed);
    ControlFlow::Break(())
}

/// Client tick HEAD: Velocity's delayed scaling of the current motion.
pub(super) fn velocity_tick(game: &mut Game<'_>) {
    apply_motion_override(game);
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let Some(player) = game.player.as_ref() else { return; };
    let water = player.fluids.was_touching_water;
    let v = [player.velocity.x, player.velocity.y, player.velocity.z];
    if let Some(next) = with_client(&client, |runtime| runtime.velocity_tick(water, v))
        && let Some(player) = game.player.as_mut()
    {
        player.velocity = horizon_core::geometry::Vec3::new(next[0], next[1], next[2]);
    }
}

