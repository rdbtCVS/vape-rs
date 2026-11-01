// Client tick HEAD for KillAura and SilentAura: snapshots for the runtime,
// then the attacks by id and the managed rotation for this tick's motion
// packet (game::remote::vape_combat_remote).
use crate::game::remote::vape_combat_remote as remote;
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::modules::combat::{kill_aura, rotation, silent_aura, trigger_bot::Cooldown};
use crate::game::vape_rs::targeting::entities::PlayerView;
use crate::game::{Game, Screen};

fn no_screen(game: &Game<'_>) -> bool {
    game.screen == Screen::Playing && game.title.is_none() && !game.dialog_open() && game.server_packs.prompt.is_none()
}

fn cooldown_ready(game: &Game<'_>) -> bool {
    Cooldown {
        ticks: game.hand_strength.ticker as f32,
        delay: horizon_client::attack_strength::delay(game.main_hand_attack_speed()),
    }
    .ready(0.0)
}

/// Every snapshot entity's previous tick position.
fn previous(game: &Game<'_>) -> Vec<(i32, [f64; 3])> {
    let mut out: Vec<_> = game.mobs.iter().map(|m| (m.id as i32, [m.previous.x, m.previous.y, m.previous.z])).collect();
    if let Some(host) = &game.lan {
        out.extend(host.hub.guests.iter().map(|g| (g.entity_id, g.previous_position)));
    }
    out
}

pub(super) fn tick(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (kill, silent, epoch, now, walls) = with_client(&client, |runtime| {
        let walls = |id: &str| runtime.settings().get(&format!("{id}.targets.ignore-walls")).and_then(serde_json::Value::as_bool).unwrap_or(false);
        (
            runtime.kill_aura_active(),
            runtime.silent_aura_wanted(),
            runtime.session().epoch(),
            runtime.now_ms(),
            walls(kill_aura::ID) || walls(silent_aura::ID),
        )
    });
    remote::set_rotation(None);
    let Some(p) = game.player.as_ref() else { return; };
    if (!kill && !silent) || game.title.is_some() {
        return;
    }
    let position = [p.position.x, p.position.y, p.position.z];
    let player = PlayerView { position, tick_position: position, yaw: game.yaw, pitch: game.pitch, strafe: 0.0 };
    let alive = p.health.health() > 0.0;
    let eye_height = f64::from(p.dimensions.eye_height);
    let motion = [
        p.position.x - game.previous_position.x,
        p.position.y - game.previous_position.y,
        p.position.z - game.previous_position.z,
    ];
    let entities = super::vape_combat::snapshot(game, epoch, walls.then_some(8.0));
    // Break blocks' `RayTraceUtil.o()`: the view's 3-block ray ends on a
    // block with no entity before it.
    let block_hit = silent
        && game.aim().is_some_and(|(origin, dir)| {
            game.clip_ray(origin, dir, silent_aura::BASE_RANGE).is_some_and(|hit| {
                let block = (0..3).map(|i| (hit.point[i] - origin[i]).powi(2)).sum::<f64>().sqrt();
                !entities.iter().any(|e| {
                    e.alive() && !e.vehicle && rotation::ray_box(origin, dir, block, e.bounds.min, e.bounds.max).is_some()
                })
            })
        });
    let held = super::vape_combat::held(game);
    if kill {
        let tick = kill_aura::Tick {
            now_ms: now,
            player,
            entities: entities.clone(),
            attack_down: game.left,
            use_down: game.right,
            using: game.using.is_some(),
            no_screen: no_screen(game),
            focused: game.focused,
            alive,
            held: held.clone(),
            cooldown_ready: cooldown_ready(game),
        };
        let effects = with_client(&client, |runtime| runtime.kill_aura_tick(&tick));
        if effects.release_use {
            game.right = false;
        }
        for (id, attack) in effects.swings {
            if !(attack && remote::attack(game, id)) {
                remote::swing(game);
            }
        }
        if effects.reuse {
            crate::game::remote::vape_macros::use_once(game);
        }
    }
    if silent {
        let tick = silent_aura::Tick {
            now_ms: now,
            player,
            eye_height,
            motion,
            entities,
            previous: previous(game),
            attack_down: game.left,
            focused: game.focused,
            no_screen: no_screen(game),
            alive,
            held,
            sensitivity: f64::from(game.settings.sensitivity),
            cooldown_ready: cooldown_ready(game),
            shield_bypass: false,
            block_hit,
            // `GuiContainer`: the inventory screens (a container's menu
            // replaces the player's in the same screen).
            container_open: matches!(game.screen, Screen::Inventory | Screen::Creative),
        };
        // HitFlick's start paths (the attack intercepts) are not hooked yet,
        // so it never leaves Idle and needs no host view here.
        let flick = crate::game::vape_rs::core::combat_runtime::Flick::default();
        let result = with_client(&client, |runtime| runtime.silent_aura_tick(&tick, &flick));
        remote::set_rotation(result.sent);
        if result.hold_attack {
            game.left = true;
        }
        match result.click {
            silent_aura::Click::Attack(id) => {
                if !remote::attack(game, id) {
                    remote::miss(game);
                }
                crate::game::vape_block_hit::silent_click(game);
            }
            silent_aura::Click::Miss => {
                remote::miss(game);
                crate::game::vape_block_hit::silent_click(game);
            }
            silent_aura::Click::None => {}
        }
    }
}

/// Show target, from the name tag geometry HEAD: KillAura's boxes and
/// SilentAura's Box through walls in the see-through pass (`RenderUtil.k`
/// disables depth testing), SilentAura's Ring depth tested and blended
/// without writing depth in the translucent item overlay, at full light.
pub(crate) fn render(
    game: &Game<'_>,
    client: &crate::game::vape_rs::core::Client,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
    see_through: &mut Vec<horizon_client::mesher::Vertex>,
) {
    use crate::game::vape_rs::modules::combat::aura_render::Shown;
    let screen = no_screen(game);
    let ids = with_client(client, |runtime| runtime.aura_render_ids(screen));
    let Some(p) = game.player.as_ref() else { return; };
    if ids.is_empty() {
        return;
    }
    let lerp = |a: [f64; 3], b: [f64; 3]| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * alpha);
    let me = [p.position.x, p.position.y, p.position.z];
    let distance = |at: [f64; 3]| (0..3).map(|i| (at[i] - me[i]).powi(2)).sum::<f64>().sqrt();
    let mut shown = Vec::new();
    for mob in game.mobs.iter().filter(|m| ids.contains(&(m.id as i32))) {
        let current = [mob.position.x, mob.position.y, mob.position.z];
        let b = mob.bounds();
        let (lo, hi) = (b.min_position(), b.max_position());
        let id = mob.kind.id.trim_start_matches("minecraft:");
        let s = Shown {
            id: mob.id as i32,
            feet: lerp([mob.previous.x, mob.previous.y, mob.previous.z], current),
            width: hi.x - lo.x,
            height: hi.y - lo.y,
            player: id == "player" || id == "armor_stand",
        };
        shown.push((s, distance(current)));
    }
    if let Some(host) = &game.lan {
        for guest in host.hub.guests.iter().filter(|g| ids.contains(&g.entity_id) && host.hub.here(g)) {
            let s = Shown {
                id: guest.entity_id,
                feet: lerp(guest.previous_position, guest.position),
                width: 0.6,
                height: 1.8,
                player: true,
            };
            shown.push((s, distance(guest.position)));
        }
    }
    let (boxes, rings) = with_client(client, |runtime| runtime.aura_quads(&shown, screen));
    let Some(uv) = super::names::vape_name_tags::white_uv(game) else { return; };
    if !boxes.is_empty() {
        super::names::vape_name_tags::push_quads(boxes, uv, camera, 2, 0, see_through);
    }
    if rings.is_empty() {
        return;
    }
    game.mark_translucent_entity(out.len(), "minecraft:name_tag");
    let layer = horizon_client::gfx::TRANSLUCENT_ITEM_OVERLAY;
    let light = horizon_client::block_entity_render::vertex_light(0xFF);
    for q in rings {
        for (corner, c) in q.corners.iter().zip(q.colors) {
            // ARGB to the vertex's ABGR.
            let color = (c & 0xff00_ff00) | ((c & 0x00ff_0000) >> 16) | ((c & 0x0000_00ff) << 16);
            let pos = (glam::DVec3::from_array(corner.0) - camera).as_vec3().to_array();
            out.push(horizon_client::mesher::Vertex { normal: [0; 4], pos, uv, layer, color, light, glint_uv: [0.0, 0.0] });
        }
    }
}
