use super::Game;
use crate::game::vape_rs::core::{ModuleId, with_client};
use core::ops::ControlFlow;
use horizon_client::gizmos::{Gizmos, Line};

pub(crate) fn fishing_line(game: &Game<'_>, alpha: f64, size: [u32; 2]) -> ControlFlow<Vec<Line>> {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let options = with_client(&client, |runtime| {
        if !runtime.enabled(ModuleId::StorageEsp) { return None; }
        let settings = runtime.settings();
        let boolean = |key: &str| settings.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false);
        let color = |key: &str| settings.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0) as u32;
        Some((boolean("render.storage-esp.chests"), color("render.storage-esp.chest-color"), boolean("render.storage-esp.furnace"), color("render.storage-esp.furnace-color")))
    });
    let Some((chests, chest_color, furnaces, furnace_color)) = options else { return ControlFlow::Continue(()); };
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
    let mut gizmos = Gizmos::default();
    for (&(x,y,z), entity) in &game.world.block_entities {
        use horizon_client::chunk::block_entity::BlockEntity;
        let color = match entity { BlockEntity::Chest(_) if chests => chest_color, BlockEntity::Furnace(_) if furnaces => furnace_color, _ => continue };
        let min = [f64::from(x), f64::from(y), f64::from(z)];
        let max = [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0];
        gizmos.cuboid(min, max, color, 1.5);
    }
    // A negative width marks a line for the always-on-top pass, so storage
    // outlines show through walls (see the `vape_on_top` gizmos module).
    lines.extend(gizmos.lines.into_iter().map(|l| Line { width: -l.width, ..l }));
    ControlFlow::Break(lines)
}
