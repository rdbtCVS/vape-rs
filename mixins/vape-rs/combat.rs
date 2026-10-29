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
