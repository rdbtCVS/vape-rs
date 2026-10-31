use super::Game;
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::modules::render::{esp::Camera, storage_esp::Storage};
use core::ops::ControlFlow;
use horizon_client::gizmos::Line;

/// StorageESP's containers: the host's rendered block entities (so ender
/// chests and shulker boxes too) with each one's lid openness.
pub(crate) fn storage_blocks(game: &Game<'_>, partial: f32) -> Vec<(Storage, [i32; 3], f32)> {
    let Some(r) = game.renderer.as_ref() else {
        return Vec::new();
    };
    let facts = game.res.facts();
    r.visible_block_entities(game.settings.chunk_fade)
        .filter_map(|&(pos, state)| {
            let kind = Storage::from_block(&facts.get(horizon_world::registry::BlockStateId(state)).name)?;
            Some((kind, pos, game.chest_lid(pos, partial)))
        })
        .collect()
}

/// Explosions' primed TNT at the partial tick (age `80 - fuse`).
pub(crate) fn tnts(game: &Game<'_>, partial: f64) -> Vec<crate::game::vape_rs::modules::render::explosions::Tnt> {
    game.tnts
        .iter()
        .map(|t| crate::game::vape_rs::modules::render::explosions::Tnt {
            position: [
                t.previous.x + (t.position.x - t.previous.x) * partial,
                t.previous.y + (t.position.y - t.previous.y) * partial,
                t.previous.z + (t.position.z - t.previous.z) * partial,
            ],
            age: 80 - t.fuse,
        })
        .collect()
}

/// PropHunt's props: each falling block's position at the partial tick.
pub(crate) fn falling_blocks(game: &Game<'_>, partial: f64) -> Vec<[f64; 3]> {
    game.falling
        .iter()
        .map(|f| {
            [
                f.previous.x + (f.position.x - f.previous.x) * partial,
                f.previous.y + (f.position.y - f.previous.y) * partial,
                f.previous.z + (f.position.z - f.previous.z) * partial,
            ]
        })
        .collect()
}

/// Search's scan of loaded chunks, a few per frame, around the player.
struct SearchScan {
    query: Option<crate::game::vape_rs::modules::render::search::Query>,
    /// Whether each block state is searched for, as resolved so far.
    states: std::collections::BTreeMap<u32, bool>,
    /// The matches of each scanned chunk.
    found: std::collections::BTreeMap<(i32, i32), Vec<([i32; 3], String)>>,
    /// Chunks left in this pass, nearest last.
    pending: Vec<(i32, i32)>,
}

static SEARCH: std::sync::Mutex<SearchScan> = std::sync::Mutex::new(SearchScan {
    query: None,
    states: std::collections::BTreeMap::new(),
    found: std::collections::BTreeMap::new(),
    pending: Vec::new(),
});

/// Chunks scanned per frame.
const SEARCH_CHUNKS_PER_FRAME: usize = 2;

fn search_found(game: &Game<'_>, query: Option<crate::game::vape_rs::modules::render::search::Query>) -> Vec<([i32; 3], String)> {
    let Ok(mut scan) = SEARCH.lock() else {
        return Vec::new();
    };
    let Some(query) = query else {
        if scan.query.take().is_some() {
            scan.states.clear();
            scan.found.clear();
            scan.pending.clear();
        }
        return Vec::new();
    };
    let Some(player) = game.player.as_ref() else {
        return Vec::new();
    };
    if scan.query.as_ref() != Some(&query) {
        scan.states.clear();
        scan.found.clear();
        scan.pending.clear();
        scan.query = Some(query.clone());
    }
    let (px, pz) = (player.position.x.floor() as i32, player.position.z.floor() as i32);
    let (cx, cz) = (px >> 4, pz >> 4);
    let reach = (query.range + 15) >> 4;
    let in_range = |(x, z): (i32, i32)| (x - cx).abs() <= reach && (z - cz).abs() <= reach;
    scan.found.retain(|&k, _| in_range(k) && game.world.chunks.contains_key(&k));
    if scan.pending.is_empty() {
        let mut next: Vec<(i32, i32)> = (-reach..=reach)
            .flat_map(|dx| (-reach..=reach).map(move |dz| (cx + dx, cz + dz)))
            .filter(|k| game.world.chunks.contains_key(k))
            .collect();
        next.sort_by_key(|&(x, z)| std::cmp::Reverse((x - cx).pow(2) + (z - cz).pow(2)));
        scan.pending = next;
    }
    let facts = game.res.facts();
    let (bottom, top) = (game.world.bottom_y(), game.world.top_y());
    for _ in 0..SEARCH_CHUNKS_PER_FRAME {
        let Some(chunk) = scan.pending.pop() else {
            break;
        };
        if !game.world.chunks.contains_key(&chunk) {
            continue;
        }
        let SearchScan { states, found, .. } = &mut *scan;
        let mut wanted = |state: horizon_world::registry::BlockStateId| {
            *states
                .entry(state.0)
                .or_insert_with(|| query.color(&facts.get(state).name).is_some())
        };
        let mut hits = Vec::new();
        let (x0, z0) = (chunk.0 << 4, chunk.1 << 4);
        let mut y0 = bottom;
        while y0 < top {
            let lo = [x0, y0, z0];
            let hi = [x0 + 15, (y0 + 15).min(top - 1), z0 + 15];
            if game.world.may_hold(lo, hi, &mut wanted) {
                for y in lo[1]..=hi[1] {
                    for x in x0..x0 + 16 {
                        for z in z0..z0 + 16 {
                            let state = game.world.block(x, y, z);
                            if !wanted(state) {
                                continue;
                            }
                            if query.only_caves {
                                let open = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
                                    .iter()
                                    .any(|d| facts.get(game.world.block(x + d[0], y + d[1], z + d[2])).name.ends_with("air"));
                                if !open {
                                    continue;
                                }
                            }
                            let name = facts.get(state).name.trim_start_matches("minecraft:").to_owned();
                            hits.push(([x, y, z], name));
                        }
                    }
                }
            }
            y0 += 16;
        }
        found.insert(chunk, hits);
    }
    let range = f64::from(query.range);
    scan.found
        .values()
        .flatten()
        .filter(|(p, _)| {
            let (dx, dz) = (f64::from(p[0]) + 0.5 - player.position.x, f64::from(p[2]) + 0.5 - player.position.z);
            dx * dx + dz * dz <= range * range
        })
        .cloned()
        .collect()
}

pub(crate) fn fishing_line(game: &Game<'_>, alpha: f64, size: [u32; 2]) -> ControlFlow<Vec<Line>> {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (storage_on, entities_on, epoch) =
        with_client(&client, |runtime| {
            let (s, e) = runtime.world_line_sources();
            (s, e, runtime.session().epoch())
        });
    if !storage_on && !entities_on {
        crate::game::glowing::vape_esp_outline::set(Default::default());
        return ControlFlow::Continue(());
    }
    // The first-person eye at the camera's partial tick, as `camera_view`
    // starts from (that needs `&mut Game`; third-person offsets are not
    // applied, matching the reference's tracer origin at the player's eye).
    let Some(player) = game.player.as_ref() else {
        return ControlFlow::Continue(());
    };
    let t = game.clock.camera_fraction();
    let (now, before) = (player.position, game.previous_position);
    let at = glam::DVec3::new(
        before.x + (now.x - before.x) * t,
        before.y + (now.y - before.y) * t + f64::from(player.dimensions.eye_height),
        before.z + (now.z - before.z) * t,
    );
    let (yaw, pitch) = (game.yaw, game.pitch);
    let own = game.hook_position(alpha).zip(game.rod_hand_position(alpha, size));
    let width = horizon_client::gfx::outline_width(size[0]);
    let mut lines = Vec::new();
    for (hook, hand) in own.into_iter().chain(game.remote_hook_ends(alpha)) {
        let offset = (hand - (hook + glam::DVec3::new(0.0, 0.25, 0.0))).as_vec3();
        let point = |a: f32| {
            let y = offset.y * (a * a + a) * 0.5 + 0.25;
            hook + glam::Vec3::new(offset.x * a, y, offset.z * a).as_dvec3()
        };
        lines.extend((0..16).map(|i| Line { start: point(i as f32 / 16.0).to_array(), end: point((i + 1) as f32 / 16.0).to_array(), color: 0xff00_0000, width }));
    }
    let storage = if storage_on { storage_blocks(game, alpha as f32) } else { Vec::new() };
    let entities = if entities_on {
        crate::game::vape_combat::snapshot(game, epoch, None)
    } else {
        Vec::new()
    };
    let outlines = with_client(&client, |runtime| runtime.esp_outlines(&entities));
    crate::game::glowing::vape_esp_outline::set(outlines.into_iter().map(|(id, rgb)| (id as u32, rgb)).collect());
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let camera = Camera {
        eye: at.to_array(),
        look: [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()],
        yaw,
        pitch,
    };
    // Trajectories: the held item and how long it has been used.
    let held = game.inventory.get(game.selected).and_then(Option::as_ref);
    let use_ticks = game.using.filter(|(slot, _)| *slot == game.selected).map(|(_, t)| t);
    let launch = held.and_then(|stack| {
        use crate::game::vape_rs::modules::render::trajectories::Loaded;
        let view = horizon_client::item_render::stack_view(stack);
        let loaded = match view.patch.get("minecraft:charged_projectiles").and_then(|v| v.as_ref()?.as_array().cloned()) {
            Some(p) if p.iter().any(|t| t.to_string().contains("firework_rocket")) => Loaded::Firework,
            Some(p) if !p.is_empty() => Loaded::Arrow,
            _ => Loaded::Empty,
        };
        let on_ground = game.player.as_ref().is_some_and(|p| p.ground.on_ground);
        let movement = game.known_player_movement();
        with_client(&client, |runtime| runtime.trajectory_launch(&stack.item, use_ticks, loaded))
            .map(|l| l.with_shooter([movement.x, movement.y, movement.z], on_ground))
    });
    let mut block = |a: [f64; 3], b: [f64; 3]| {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if length < 1e-9 {
            return None;
        }
        game.clip_ray(a, d.map(|v| v / length), length).map(|hit| hit.point)
    };
    let trajectory = launch.map(|l| (l, &mut block as &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>));
    let extra = with_client(&client, |runtime| runtime.world_lines(&camera, &entities, &storage, trajectory));
    let query = with_client(&client, |runtime| runtime.search_query());
    let found = search_found(game, query);
    let props = if with_client(&client, |runtime| runtime.props_wanted()) { falling_blocks(game, alpha) } else { Vec::new() };
    let scene = with_client(&client, |runtime| runtime.scene_lines(&camera, &found, &props));
    let extra: Vec<_> = extra.into_iter().chain(scene).collect();
    // A negative width marks a line for the always-on-top pass, so these
    // show through walls (see the `vape_on_top` gizmos module).
    lines.extend(extra.into_iter().map(|l| Line {
        start: l.start.0,
        end: l.end.0,
        color: l.color,
        width: -l.width,
    }));
    // Explosions' blast rings stay depth-tested (the reference keeps the
    // depth test on).
    let blast = with_client(&client, |runtime| runtime.explosion_lines(&tnts(game, alpha), alpha as f32));
    lines.extend(blast.into_iter().map(|l| Line { start: l.start.0, end: l.end.0, color: l.color, width: l.width }));
    ControlFlow::Break(lines)
}
