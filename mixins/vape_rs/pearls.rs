// AutoPearl and PearlCatch on the host: the pearls, players and hotbar the
// pearl runtime reads each tick, the block ray its landing prediction casts
// (collision shapes only, so liquids pass, as `rayTraceBlocks` without
// stopOnLiquid), and the slot switches and throws it answers with.
use super::{S, play};
use crate::game::vape_rs::core::pearl_runtime::{PearlAction, PearlInput};
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::modules::utility::auto_pearl::{Hit, Pearl, Person};
use crate::game::vape_rs::targeting::entities::EntityClass;
use crate::game::Game;
use horizon_client::projectiles::{ProjectileKind, ProjectileWorld};
use horizon_core::geometry::Vec3;
use std::time::Instant;

fn hotbar(game: &Game<'_>, item: &str) -> Option<u8> {
    game.inventory
        .iter()
        .take(9)
        .position(|s| s.as_ref().is_some_and(|s| &*s.item == item))
        .map(|slot| slot as u8)
}

/// The players a pearl may belong to: remote players, LAN guests and the
/// local player.
fn people(game: &Game<'_>, position: [f64; 3]) -> Vec<Person> {
    let mut out: Vec<Person> = game
        .mobs
        .iter()
        .filter(|m| m.alive() && EntityClass::classify(&m.kind.id, &m.kind.category) == EntityClass::Player)
        .map(|m| Person { id: m.id as i32, position: [m.position.x, m.position.y, m.position.z], local: false })
        .collect();
    if let Some(host) = &game.lan {
        out.extend(
            host.hub
                .guests
                .iter()
                .filter(|g| host.hub.here(g) && g.loaded && !g.dead)
                .map(|g| Person { id: g.entity_id, position: g.position, local: false }),
        );
    }
    let local = game
        .remote
        .as_ref()
        .map(|r| r.entity_id)
        .or(game.lan.as_ref().map(|h| h.host_entity))
        .unwrap_or(0);
    out.push(Person { id: local, position, local: true });
    out
}

/// A throw at the managed rotation: the UseItem packet (or the in-process
/// throw) takes the rotation for this use only, and the camera stays.
fn silent_use(game: &mut Game<'_>, rotation: (f32, f32)) {
    let view = (game.yaw, game.pitch);
    (game.yaw, game.pitch) = rotation;
    if game.remote.is_some() {
        // The tick's HEAD runs before MultiPlayerGameMode.tick syncs the
        // carried slot, so a slot chosen this tick goes out first.
        let selected = game.selected;
        if let Some(r) = game.remote.as_mut()
            && r.carried != Some(selected)
        {
            r.carried = Some(selected);
            game.send(S::SetCarriedItem(play::SetCarriedItem { slot: selected as i16 }));
        }
        game.remote_use();
    } else if game.using.is_none() {
        if let Err(error) = game.use_with_hand() {
            with_client(&game.__horizon_mixins_vape_rs_client(), |r| r.notify(format!("Pearl throw failed: {error}")));
        }
        game.use_delay = 4;
    }
    (game.yaw, game.pitch) = view;
}

pub fn tick(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if !with_client(&client, |r| r.pearls_wanted()) {
        return;
    }
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let position = [player.position.x, player.position.y, player.position.z];
    let alive = player.health.health() > 0.0;
    let pearls: Vec<Pearl> = game
        .creatures
        .projectiles
        .iter()
        .filter(|p| matches!(p.kind, ProjectileKind::EnderPearl))
        .map(|p| Pearl {
            id: p.id,
            position: [p.position.x, p.position.y, p.position.z],
            velocity: [p.velocity.x, p.velocity.y, p.velocity.z],
            in_water: p.touching_water,
            stuck: p.stuck,
        })
        .collect();
    let people = people(game, position);
    let held = crate::game::vape_combat::held(game);
    let input = PearlInput {
        at: Instant::now(),
        position,
        eye_height: f64::from(player.dimensions.eye_height),
        motion: [player.velocity.x, player.velocity.y, player.velocity.z],
        on_ground: player.ground.on_ground,
        view: (game.yaw, game.pitch),
        alive,
        health: player.health.health(),
        held_pearl: held.id.as_deref() == Some("ender_pearl"),
        held,
        pearl_slot: hotbar(game, "minecraft:ender_pearl"),
        charge_slot: hotbar(game, "minecraft:wind_charge"),
        selected: game.selected as u8,
        pearls: &pearls,
        people: &people,
    };
    let base = game.mob_world_base(&[]);
    let world = crate::game::creatures::ProjWorld { w: &base, targets: Vec::new() };
    let mut ray = |from: [f64; 3], to: [f64; 3]| {
        world
            .clip(Vec3::new(from[0], from[1], from[2]), Vec3::new(to[0], to[1], to[2]))
            .map(|(at, _, _)| Hit { position: [at.x, at.y, at.z], water: false })
    };
    let out = with_client(&client, |r| r.pearls_tick(&input, &mut ray));
    for action in out.actions {
        match action {
            PearlAction::Select(slot) => {
                if slot < 9 && alive && game.title.is_none() {
                    game.selected = usize::from(slot);
                }
            }
            // The use key's click, through the host's input path.
            PearlAction::Use(None) => crate::game::remote::vape_macros::use_once(game),
            PearlAction::Use(Some(rotation)) => silent_use(game, rotation),
        }
    }
}

/// `onPlayerUseItem`: the local player used an ender pearl.
pub fn used(game: &Game<'_>, item: &str) {
    if item == "minecraft:ender_pearl" {
        with_client(&game.__horizon_mixins_vape_rs_client(), |r| r.pearl_used(Instant::now()));
    }
}
