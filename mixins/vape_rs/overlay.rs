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

fn item(game: &Game<'_>, id: &str, count: u32, damage: u32) -> Option<horizon_client::items::ItemStack> {
    (!id.is_empty()).then(|| horizon_client::items::ItemStack {
        item: std::sync::Arc::from(id),
        count: count.max(1) as _,
        damage: damage as _,
        extra: None,
    })
    .filter(|_| game.res.items().get(id).is_some())
}

fn stack_item(game: &Game<'_>, id: Option<&str>) -> Option<Item> {
    let id = id?;
    let max_damage = game.res.items().get(id).map_or(0, |i| u32::from(i.max_damage));
    Some(Item { id: id.to_owned(), count: 1, damage: 0, max_damage, enchanted: false })
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
    let tab = crate::game::remote::vape_remote_tab::players(game);
    let entities = crate::game::vape_combat::snapshot(game, epoch, None)
        .into_iter()
        .map(|view| {
            let mob = game.mobs.iter().find(|m| m.id as i32 == view.handle.id);
            let listed = tab.iter().find(|t| t.0 as i32 == view.handle.id);
            let equipment = mob.map_or_else(Default::default, |m| {
                let e = &m.extra;
                [&e.main_hand, &e.off_hand, &e.head, &e.chest, &e.legs, &e.feet].map(|s| stack_item(game, s.as_deref()))
            });
            Living {
                max_health: mob.map_or(20.0, horizon_client::mobs::finalize::max_health),
                absorption: 0.0,
                display_name: listed.and_then(|t| t.2.clone()),
                creative: listed.is_some_and(|t| t.1 == 1),
                equipment,
                hurt: mob.is_some_and(|m| m.hurt_time > 0),
                view,
            }
        })
        .collect();
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
    let spawners = if with_client(&game.__horizon_mixins_vape_rs_client().clone(), |r| r.spawners_wanted()) {
        spawners(game)
    } else {
        Vec::new()
    };
    Some(Frame {
        spawners,
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
    let draws = with_client(&client, |runtime| runtime.overlay(&frame));
    paint(game, batch, &draws);
}

fn paint(game: &Game<'_>, batch: &mut HudBatch, draws: &[Draw]) {
    let atlas = &game.gui.atlas;
    for draw in draws {
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
                let start = batch.vertices.len();
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
                let _ = start;
            }
            Draw::Item { x, y, scale, item: it } => {
                if let Some(stack) = item(game, &it.id, it.count, it.damage) {
                    let start = batch.vertices.len();
                    game.draw_item_model(batch, &stack, *x, *y);
                    if (*scale - 1.0).abs() > f32::EPSILON {
                        batch.scale_since(start, *x, *y, *scale, *scale);
                    }
                }
            }
        }
    }
}
