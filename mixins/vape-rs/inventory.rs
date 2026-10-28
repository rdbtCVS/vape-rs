use crate::game::vape_rs::{
    actions::HotbarSlot,
    core::lifecycle::EntityHandle,
    core::{ModuleId, with_client},
    modules::utility::{
        auto_tool::{Candidate, Snapshot, Target},
        inventory_guard::CombatSnapshot,
    },
};
use crate::game::{Game, Screen};
use horizon_client::enchant::EffectContext;
use horizon_core::random::LegacyRandom;

fn looking(yaw: f32, from: [f64; 3], to: [f64; 3]) -> bool {
    let angle = (-(to[0] - from[0])).atan2(to[2] - from[2]).to_degrees() as f32;
    ((angle - yaw + 180.0).rem_euclid(360.0) - 180.0).abs() < 60.0
}
fn friend(keys: &[String], name: &str, uuid: Option<u128>) -> bool {
    keys.iter().any(|key| {
        key.eq_ignore_ascii_case(name)
            || uuid.is_some_and(|uuid| {
                u128::from_str_radix(&key.replace('-', ""), 16).ok() == Some(uuid)
            })
    })
}
fn combat(game: &Game<'_>, keys: &[String], target: Option<Target>) -> CombatSnapshot {
    let Some(player) = game.player.as_ref() else {
        return CombatSnapshot::default();
    };
    let position = [player.position.x, player.position.y, player.position.z];
    let mut out = CombatSnapshot {
        health: player.health.health(),
        hurt_cooldown: player.health.hurt.cooldown,
        attacking_entity: game.swing.swinging() && matches!(target, Some(Target::Entity(_))),
        stationary: player.velocity.x == 0.0 && player.velocity.z == 0.0,
        ..CombatSnapshot::default()
    };
    let mut observe = |point: [f64; 3], yaw: f32| {
        let distance = (0..3)
            .map(|i| (position[i] - point[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        out.nearby_enemy |= distance <= 10.0;
        if distance < 6.0 {
            out.looking_at_enemy |= looking(game.yaw, position, point);
            out.enemy_looking_at_player |= looking(yaw, point, position);
        }
    };
    let names = game.remote_player_names();
    for mob in game
        .mobs
        .iter()
        .filter(|mob| mob.alive() && &*mob.kind.id == "minecraft:player")
    {
        let name = names
            .iter()
            .find(|(id, _)| *id == mob.id)
            .map_or("", |(_, name)| name.as_str());
        let uuid_friend = keys
            .iter()
            .filter_map(|key| u128::from_str_radix(&key.replace('-', ""), 16).ok())
            .any(|uuid| game.remote.as_ref().and_then(|r| r.player_entity(uuid)) == Some(mob.id));
        if !uuid_friend && !friend(keys, name, None) {
            observe([mob.position.x, mob.position.y, mob.position.z], mob.yaw);
        }
    }
    if let Some(host) = &game.lan {
        for guest in host
            .hub
            .guests
            .iter()
            .filter(|g| host.hub.here(g) && g.loaded && !g.dead)
        {
            if !friend(keys, &guest.name, Some(guest.uuid)) {
                observe(guest.position, guest.yaw);
            }
        }
    }
    out
}
pub(super) fn auto_tool(game: &mut Game<'_>, attack_event: bool) {
    if game.serving.is_some()
        || game.options.dedicated.is_some()
        || game.player.is_none()
        || !game.focused
        || game.screen != Screen::Playing
        || game.title.is_some()
        || game.sleeping.is_some()
        || game.dialog_open()
        || game.server_packs.prompt.is_some()
        || game.spectator()
    {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (enabled, epoch, tick, guard, keys) = with_client(&client, |runtime| {
        let guard = runtime
            .settings()
            .get("world.auto-tool.not-combat")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        (
            runtime.enabled(ModuleId::AutoTool),
            runtime.session().epoch(),
            runtime.logic_tick(),
            guard,
            if guard {
                runtime.friend_keys()
            } else {
                Vec::new()
            },
        )
    });
    if !enabled {
        return;
    }
    let Some(selected) = HotbarSlot::new(game.selected) else {
        return;
    };
    let Some(eye) = game.eye_position() else {
        return;
    };
    let block = game.pick();
    let block_distance = block.as_ref().map(|hit| {
        (0..3)
            .map(|i| (eye[i] - hit.point[i]).powi(2))
            .sum::<f64>()
            .sqrt()
    });
    let mut entity = game
        .mob_hit()
        .map(|(index, distance)| (game.mobs[index].id as i32, distance));
    if let Some(hit) = game.remote_object_hit(true)
        && entity.is_none_or(|(_, distance)| hit.distance < distance)
    {
        entity = Some((hit.id, hit.distance));
    }
    if let Some((conn, distance)) = game.guest_hit()
        && entity.is_none_or(|(_, d)| distance < d)
    {
        if let Some(guest) = game
            .lan
            .as_ref()
            .and_then(|host| host.hub.guests.iter().find(|g| g.conn == conn))
        {
            entity = Some((guest.entity_id, distance));
        }
    }
    let target = if let Some((id, distance)) = entity
        && block_distance.is_none_or(|block| distance <= block)
    {
        Some(Target::Entity(EntityHandle { epoch, id }))
    } else {
        block.as_ref().map(|hit| Target::Block(hit.pos))
    };
    let items = game.res.items();
    let block_name = block.as_ref().map(|hit| {
        &game
            .res
            .facts()
            .get(game.world.block(hit.pos[0], hit.pos[1], hit.pos[2]))
            .name
    });
    let candidates = std::array::from_fn(|slot| {
        let Some(stack) = game.inventory.get(slot).and_then(Option::as_ref) else {
            return Candidate {
                item: None,
                score: 1.0,
            };
        };
        let info = items.get(&stack.item);
        let tool = info.is_some_and(|info| info.components.iter().any(|c| c == "minecraft:tool"));
        let score = match target {
            Some(Target::Block(_)) => {
                let speed = block_name.map_or(1.0, |block| items.mining_speed(Some(stack), block));
                if tool && stack.item.starts_with("minecraft:golden_") {
                    speed / 2.0
                } else {
                    speed
                }
            }
            Some(Target::Entity(_)) if tool || stack.item.ends_with("_sword") => {
                let ctx = EffectContext {
                    attacker: Some("minecraft:player"),
                    direct_attacker: Some("minecraft:player"),
                    is_direct: true,
                    damage_type: Some("minecraft:player_attack"),
                    items: Some(items),
                    ..Default::default()
                };
                let damage = items.enchantments.modify_value(
                    Some(stack),
                    "minecraft:damage",
                    &ctx,
                    &mut LegacyRandom::new(0),
                    0.0,
                );
                info.map_or(0.0, |info| info.attack_damage)
                    + damage
                    + stack.enchantment_level("minecraft:fire_aspect") as f32 * 0.01
                    + if stack.item.ends_with("_sword") {
                        0.01
                    } else {
                        0.0
                    }
            }
            _ => 1.0,
        };
        Candidate {
            item: Some(stack.item.to_string()),
            score,
        }
    });
    let snapshot = Snapshot {
        epoch,
        tick,
        selected,
        target,
        candidates,
        attack_down: attack_event || game.left,
        sneaking: game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft),
        using_item: game.using.is_some(),
        combat_blocked: false,
    };
    let combat = if guard {
        combat(game, &keys, target)
    } else {
        CombatSnapshot::default()
    };
    let proposed = with_client(&client, |runtime| {
        runtime.auto_tool_propose(snapshot, combat)
    });
    let Some(proposed) = proposed else {
        return;
    };
    let item = game
        .inventory
        .get(proposed.change.selected.index())
        .and_then(Option::as_ref)
        .map(|stack| &*stack.item);
    if item != proposed.change.item.as_deref()
        || !with_client(&client, |runtime| {
            runtime.auto_tool_authorize(&proposed, selected)
        })
    {
        return;
    }
    // Same field updated by the host's number-key path. Remote tick input
    // sends SetCarriedItem before attack/use, through the host's own protocol.
    game.selected = proposed.change.selected.index();
    with_client(&client, |runtime| runtime.auto_tool_applied(proposed));
}
