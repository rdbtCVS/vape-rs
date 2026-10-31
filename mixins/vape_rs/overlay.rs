// Screen-space overlays drawn under the game HUD (the spyglass scope is the
// HUD's first layer): ESP 2D, Health, Arrows, Indicators, BedPlates and the
// NameTags equipment row. The frame's camera comes from the gizmos pass, the
// projection from the host's own FOV, view bobbing and nausea spin.
use super::Game;
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::modules::render::overlay::{Align, Draw, Frame, Item, Living, Own, Projection};
use horizon_client::hud::{HudAtlas, HudBatch};

/// ARGB to the batch's ABGR.
fn abgr(argb: u32) -> u32 {
    (argb & 0xff00_ff00) | ((argb & 0x00ff_0000) >> 16) | ((argb & 0x0000_00ff) << 16)
}

fn line(batch: &mut HudBatch, atlas: &HudAtlas, from: [f32; 2], to: [f32; 2], width: f32, color: u32) {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return;
    }
    let (nx, ny) = (-dy / length * width / 2.0, dx / length * width / 2.0);
    batch.fill_quad(
        atlas,
        [[from[0] + nx, from[1] + ny], [to[0] + nx, to[1] + ny], [to[0] - nx, to[1] - ny], [from[0] - nx, from[1] - ny]],
        color,
    );
}

fn item(game: &Game<'_>, it: &Item) -> Option<horizon_client::items::ItemStack> {
    if it.id.is_empty() || game.res.items().get(&it.id).is_none() {
        return None;
    }
    let mut stack = horizon_client::items::ItemStack {
        item: std::sync::Arc::from(it.id.as_str()),
        count: it.count.max(1),
        damage: it.damage,
        extra: None,
    };
    // Any enchantment gives the host's foil (an item's own glint comes from
    // its id).
    if it.enchanted {
        stack.enchant("minecraft:unbreaking", 1);
    }
    Some(stack)
}

fn max_damage(game: &Game<'_>, id: &str) -> u32 {
    game.res.items().get(id).map_or(0, |i| u32::from(i.max_damage))
}

/// A mob's worn or held item: its id, with the slot's enchantments.
fn mob_item(game: &Game<'_>, id: Option<&str>, enchanted: bool) -> Option<Item> {
    let id = id?;
    Some(Item { id: id.to_owned(), count: 1, damage: 0, max_damage: max_damage(game, id), enchanted })
}

/// A guest's stack as the overlay sees it.
fn stack_item(game: &Game<'_>, stack: Option<&horizon_client::items::ItemStack>) -> Option<Item> {
    let stack = stack?;
    Some(Item {
        id: stack.item.to_string(),
        count: stack.count,
        damage: stack.damage,
        max_damage: max_damage(game, &stack.item),
        enchanted: stack.has_foil(false),
    })
}

/// SpawnerFinder's spawners: each loaded monster spawner and the id of the
/// mob it shows (`Spawner.nextSpawnData`).
fn spawners(game: &Game<'_>) -> Vec<([i32; 3], String)> {
    use horizon_client::containers::BlockEntity;
    use horizon_client::mobs::spawner::{SpawnDataExt, SpawnerExt};
    game.world
        .block_entities
        .iter()
        .filter_map(|(&(x, y, z), e)| match e {
            BlockEntity::Spawner(s) => Some(([x, y, z], s.next_data()?.id()?)),
            _ => None,
        })
        .collect()
}

/// Indicators' simulation, once a tick (the projectiles only move per
/// tick): the tick, its followed flights and each one's closest approach.
type IndicatorCache = (i64, Vec<crate::game::fishing::vape_world_lines::InFlight>, Vec<f64>);
static INDICATORS: std::sync::Mutex<IndicatorCache> = std::sync::Mutex::new((i64::MIN, Vec::new(), Vec::new()));

/// Indicators' shots: each followed projectile at the partial tick, its
/// distance from the player's feet and its simulated closest approach.
fn indicator_shots(game: &Game<'_>, client: &crate::game::vape_rs::core::Client, partial: f64) -> Vec<crate::game::vape_rs::modules::render::indicators::Shot> {
    use crate::game::vape_rs::modules::render::indicators::{self, Me, Projectile, Shot};
    let Some(player) = game.player.as_ref() else {
        return Vec::new();
    };
    let feet = [player.position.x, player.position.y, player.position.z];
    let flying: Vec<_> = crate::game::fishing::vape_world_lines::in_flight(game)
        .into_iter()
        .filter_map(|f| {
            let (id, position, previous, velocity, in_water, stuck) = f;
            let p = Projectile { kind: indicators::Kind::from_entity(id)?, position, previous, velocity, in_water, stuck };
            Some((f, p))
        })
        .collect();
    let followed: Vec<_> = with_client(client, |runtime| flying.into_iter().filter(|(_, p)| runtime.indicators_follow(p, feet)).collect());
    if followed.is_empty() {
        return Vec::new();
    }
    let Ok(mut cache) = INDICATORS.lock() else {
        return Vec::new();
    };
    let key: Vec<_> = followed.iter().map(|(f, _)| *f).collect();
    if cache.0 != game.world.game_time || cache.1 != key {
        let (w, h) = (f64::from(player.dimensions.width) / 2.0, f64::from(player.dimensions.height));
        let me = Me { bounds: ([feet[0] - w, feet[1], feet[2] - w], [feet[0] + w, feet[1] + h, feet[2] + w]), feet };
        let bottom = f64::from(game.world.bottom_y());
        let mut block = |a: [f64; 3], b: [f64; 3]| {
            let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if length < 1e-9 {
                return None;
            }
            game.clip_ray(a, d.map(|v| v / length), length).map(|hit| hit.point)
        };
        let closest = followed.iter().map(|(_, p)| indicators::closest(p, &me, bottom, &mut block)).collect();
        *cache = (game.world.game_time, key, closest);
    }
    followed
        .iter()
        .zip(&cache.2)
        .map(|((_, p), &closest)| Shot {
            kind: p.kind,
            position: [0, 1, 2].map(|i| p.previous[i] + (p.position[i] - p.previous[i]) * partial),
            distance: (0..3).map(|i| (p.position[i] - feet[i]).powi(2)).sum::<f64>().sqrt(),
            closest,
        })
        .collect()
}

/// BedPlates' survey, every half second: when it ran and its plates.
static BED_PLATES: std::sync::Mutex<Option<(std::time::Instant, Vec<crate::game::vape_rs::modules::render::bed_plates::Plate>)>> =
    std::sync::Mutex::new(None);

/// BedPlates' plates: the heads of the rendered beds within range
/// (sideways), each surveyed by its blocks' clone items.
fn bed_plates(game: &Game<'_>, wanted: bool) -> Vec<crate::game::vape_rs::modules::render::bed_plates::Plate> {
    use crate::game::vape_rs::modules::render::bed_plates::{self, Facing, Plate};
    let Ok(mut cache) = BED_PLATES.lock() else {
        return Vec::new();
    };
    if !wanted {
        *cache = None;
        return Vec::new();
    }
    if let Some((at, plates)) = cache.as_ref()
        && at.elapsed() < std::time::Duration::from_millis(500)
    {
        return plates.clone();
    }
    let (Some(r), Some(player)) = (game.renderer.as_ref(), game.player.as_ref()) else {
        return Vec::new();
    };
    let facts = game.res.facts();
    let registry = &game.pack.assets.registry;
    let mut plates = Vec::new();
    for &(head, state) in r.visible_block_entities(game.settings.chunk_fade) {
        let id = horizon_world::registry::BlockStateId(state);
        if !facts.get(id).name.ends_with("_bed") {
            continue;
        }
        let Some(props) = registry.state(id).map(|s| &s.properties) else {
            continue;
        };
        let Some(facing) = props.get("facing").and_then(|f| Facing::parse(f)) else {
            continue;
        };
        if props.get("part").map(String::as_str) != Some("head") {
            continue;
        }
        let (dx, dz) = (f64::from(head[0]) + 0.5 - player.position.x, f64::from(head[2]) + 0.5 - player.position.z);
        if dx.hypot(dz) > bed_plates::RANGE {
            continue;
        }
        let layers = bed_plates::survey(head, facing, &mut |p| game.picked_stack(p, false).map(|s| s.item.to_string()));
        plates.push(Plate { head, items: bed_plates::shown(&layers) });
    }
    *cache = Some((std::time::Instant::now(), plates.clone()));
    plates
}

/// The living entities the overlay draws: the combat snapshot with each
/// one's tab listing, equipment, max health and hurt flash.
fn living(game: &Game<'_>, epoch: crate::game::vape_rs::core::lifecycle::Epoch) -> Vec<Living> {
    use std::collections::HashMap;
    let tab: HashMap<i32, (i32, Option<String>)> =
        crate::game::remote::vape_remote_tab::players(game).into_iter().map(|(id, mode, name)| (id as i32, (mode, name))).collect();
    let mobs: HashMap<i32, &horizon_client::mobs::Mob> = game.mobs.iter().map(|m| (m.id as i32, m)).collect();
    let guests: HashMap<i32, _> = game.lan.iter().flat_map(|host| host.hub.guests.iter().map(|g| (g.entity_id, g))).collect();
    crate::game::vape_combat::snapshot(game, epoch, None)
        .into_iter()
        .map(|view| {
            let id = view.handle.id;
            let listed = tab.get(&id);
            let mut living = Living {
                max_health: 20.0,
                absorption: 0.0,
                display_name: listed.and_then(|t| t.1.clone()),
                creative: listed.is_some_and(|t| t.0 == 1),
                equipment: Default::default(),
                hurt: false,
                view,
            };
            if let Some(m) = mobs.get(&id) {
                let e = &m.extra;
                let worn = [&e.main_hand, &e.off_hand, &e.head, &e.chest, &e.legs, &e.feet];
                living.equipment = std::array::from_fn(|slot| mob_item(game, worn[slot].as_deref(), horizon_client::mobs::gear::enchanted(m, slot)));
                living.max_health = horizon_client::mobs::finalize::max_health(m);
                living.hurt = m.hurt_time > 0;
            } else if let Some(g) = guests.get(&id) {
                let [head, chest, legs, feet] = g.armor.each_ref().map(Option::as_ref);
                let held = g.inventory.get(g.selected).and_then(Option::as_ref);
                living.equipment = [held, g.offhand.as_ref(), head, chest, legs, feet].map(|s| stack_item(game, s));
                living.creative = g.game_mode == 1;
            }
            living
        })
        .collect()
}

/// The frame the overlay modules draw from, or `None` outside a world.
fn frame(game: &Game<'_>, batch: &HudBatch, epoch: crate::game::vape_rs::core::lifecycle::Epoch) -> Option<Frame> {
    let player = game.player.as_ref()?;
    let (camera, view) = horizon_client::gizmos::vape_on_top::frame_camera()?;
    let partial = game.clock.camera_fraction() as f32;
    let fov = game.fov_degrees(partial, camera);
    let aspect = batch.width / batch.height.max(1.0);
    let pose = game.bob_matrix(partial) * game.spinning_matrix(partial);
    let matrix = glam::Mat4::perspective_rh(fov.to_radians(), aspect, 0.05, 1024.0) * pose * view;
    let projection = Projection { camera, matrix: matrix.to_cols_array(), width: batch.width, height: batch.height };
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (spawners_wanted, beds_wanted, entities_wanted, indicators_wanted) =
        with_client(&client, |r| (r.spawners_wanted(), r.bed_plates_wanted(), r.overlay_entities_wanted(), r.indicators_wanted()));
    // Only the modules that read entities pay for the snapshot and its
    // equipment (Health, BedPlates and the others read none).
    let entities = if entities_wanted { living(game, epoch) } else { Vec::new() };
    let own = Own {
        health: player.health.health(),
        absorption: player.health.absorption(),
        position: [player.position.x, player.position.y, player.position.z],
        yaw: game.yaw,
        hurt: player.health.hurt_animation_time > 0,
        totems: game
            .inventory
            .iter()
            .chain(std::iter::once(&game.offhand))
            .flatten()
            .filter(|s| s.item.contains("totem_of_undying"))
            .map(|s| s.count)
            .sum(),
    };
    let spawners = if spawners_wanted { spawners(game) } else { Vec::new() };
    let indicators = if indicators_wanted { indicator_shots(game, &client, f64::from(partial)) } else { Vec::new() };
    Some(Frame {
        spawners,
        indicators,
        bed_plates: bed_plates(game, beds_wanted),
        projection,
        entities,
        own,
        first_person: game.camera_type.first_person(),
        now_ms: game.started.elapsed().as_millis() as u64,
    })
}

pub(crate) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) {
    if game.title.is_some() || game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let Some(epoch) = with_client(&client, |runtime| runtime.overlay_active().then(|| runtime.session().epoch())) else {
        return;
    };
    let Some(frame) = frame(game, batch, epoch) else {
        return;
    };
    let (draws, callout) = with_client(&client, |runtime| (runtime.overlay(&frame), runtime.murder_finder_callout()));
    // MurdererFinder's callout (`sendChatMessage`): ClientPacketListener's
    // sendChat on a server (single player's included), the chat input in a
    // serverless world; both keep to the Chat option.
    if let Some(message) = callout {
        if game.remote.is_some() {
            game.send_chat(&message);
        } else {
            game.handle_chat_input(&message);
        }
    }
    paint(game, batch, &draws);
}

fn paint(game: &Game<'_>, batch: &mut HudBatch, draws: &[Draw]) {
    for draw in draws {
        paint_one(game, batch, draw);
    }
}

fn paint_one(game: &Game<'_>, batch: &mut HudBatch, draw: &Draw) {
    let atlas = &game.gui.atlas;
    match draw {
        Draw::Fill { x, y, w, h, color } => batch.fill(atlas, *x, *y, *w, *h, abgr(*color)),
        Draw::Quad { corners, color } => batch.fill_quad(atlas, *corners, abgr(*color)),
        Draw::Line { from, to, width, color } => line(batch, atlas, *from, *to, *width, abgr(*color)),
        Draw::Text { x, y, scale, text, color, shadow, align, backdrop } => {
            let width = HudBatch::text_width(atlas, text);
            let left = match align {
                Align::Left => *x,
                Align::Center => *x - width * scale / 2.0,
                Align::CenterOn(n) => {
                    let label = text.get(..*n).unwrap_or(text);
                    *x - (HudBatch::text_width(atlas, label) * scale / 2.0).ceil() + scale
                }
            };
            if let Some(b) = backdrop {
                let (x0, y0) = (left - b.pad, *y - b.pad);
                let (x1, y1) = (left + width * scale + b.pad, *y + 8.0 * scale + b.pad);
                let edge = abgr(b.edge_color);
                batch.fill(atlas, x0, y0, x1 - x0, y1 - y0, abgr(b.fill));
                batch.fill(atlas, x0 - b.edge, y0 - b.edge, x1 - x0 + 2.0 * b.edge, b.edge, edge);
                batch.fill(atlas, x0 - b.edge, y1, x1 - x0 + 2.0 * b.edge, b.edge, edge);
                batch.fill(atlas, x0 - b.edge, y0, b.edge, y1 - y0, edge);
                batch.fill(atlas, x1, y0, b.edge, y1 - y0, edge);
            }
            let text_start = batch.vertices.len();
            batch.text(atlas, text, left, *y, abgr(*color), *shadow);
            if (*scale - 1.0).abs() > f32::EPSILON {
                batch.scale_since(text_start, left, *y, *scale, *scale);
            }
        }
        Draw::Item { x, y, scale, item: it } => {
            if let Some(stack) = item(game, it) {
                let start = batch.vertices.len();
                game.draw_item_model(batch, &stack, *x, *y);
                if (*scale - 1.0).abs() > f32::EPSILON {
                    batch.scale_since(start, *x, *y, *scale, *scale);
                }
            }
        }
        Draw::Turned { pivot, degrees, inner } => {
            let start = batch.vertices.len();
            paint_one(game, batch, inner);
            let (sin, cos) = degrees.to_radians().sin_cos();
            for v in &mut batch.vertices[start..] {
                let (dx, dy) = (v.pos[0] - pivot[0], v.pos[1] - pivot[1]);
                v.pos = [pivot[0] + dx * cos - dy * sin, pivot[1] + dx * sin + dy * cos];
            }
        }
    }
}
