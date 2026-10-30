use super::Game;
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::modules::render::{esp::Camera, storage_esp::Storage};
use core::ops::ControlFlow;
use horizon_client::gizmos::Line;

pub(crate) fn fishing_line(game: &Game<'_>, alpha: f64, size: [u32; 2]) -> ControlFlow<Vec<Line>> {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (storage_on, entities_on, epoch) =
        with_client(&client, |runtime| {
            let (s, e) = runtime.world_line_sources();
            (s, e, runtime.session().epoch())
        });
    if !storage_on && !entities_on {
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
    let mut storage = Vec::new();
    if storage_on {
        let facts = game.res.facts();
        for (&(x, y, z), entity) in &game.world.block_entities {
            use horizon_client::chunk::block_entity::BlockEntity;
            if !matches!(entity, BlockEntity::Chest(_) | BlockEntity::Furnace(_)) {
                continue;
            }
            if let Some(kind) = Storage::from_block(&facts.get(game.world.block(x, y, z)).name) {
                storage.push((kind, [x, y, z]));
            }
        }
    }
    let entities = if entities_on {
        crate::game::vape_combat::snapshot(game, epoch, None)
    } else {
        Vec::new()
    };
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let camera = Camera {
        eye: at.to_array(),
        look: [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()],
        yaw,
        pitch,
    };
    let extra = with_client(&client, |runtime| runtime.world_lines(&camera, &entities, &storage));
    // A negative width marks a line for the always-on-top pass, so these
    // show through walls (see the `vape_on_top` gizmos module).
    lines.extend(extra.into_iter().map(|l| Line {
        start: l.start.0,
        end: l.end.0,
        color: l.color,
        width: -l.width,
    }));
    ControlFlow::Break(lines)
}
