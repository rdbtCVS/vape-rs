fn clear_water(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    sky: &horizon_client::environment::Sky,
    biome: u16,
    water_fog_color: [f32; 3],
) -> core::ops::ControlFlow<FogData> {
    use crate::game::vape_rs::core::{ModuleId, with_client};
    let enabled = game.player.is_some() && game.title.is_none()
        && with_client(game.__horizon_mixins_vape_rs_client(), |r| r.enabled(ModuleId::ClearWater));
    if !enabled || game.camera_fluid(camera.to_array()) != horizon_client::camera::CameraFluid::Water
        || game.potions.effects.get("blindness").is_some() || game.potions.effects.get("darkness").is_some()
    { return core::ops::ControlFlow::Continue(()); }
    // Pinned frame_fog snapshot: all native inputs and math are preserved;
    // only the completed underwater environmental fog span changes below.
        let forward = game.forward();
        let at = [camera.x, camera.y, camera.z];
        let fog_type = fog_type(&game.world, game.res.facts(), at, forward);
        let (rain, thunder) = if game.dim == Dim::Overworld {
            game.world.weather.levels(game.clock.fraction() as f32)
        } else {
            (0.0, 0.0)
        };
        let title_fog = horizon_client::fog::FogTables::default();
        let tables = if game.title.is_some() {
            &title_fog
        } else {
            game.res.fog()
        };
        let input = FogInputs {
            fog_type,
            sky,
            water_fog_color,
            biome,
            render_distance_chunks: game.terrain_distance(),
            distant_terrain: game.lod.is_some(),
            cloud_range_chunks: game.settings.cloud_range,
            camera_y: camera.y,
            min_y: game.level_min_y(),
            flat: game.level_flat(),
            forward: forward.map(|c| c as f32),
            rain,
            thunder,
            skybox: game.dim == Dim::Overworld,
            blindness: game
                .potions
                .effects
                .get("blindness")
                .map(|effect| (effect.duration, effect.duration == -1)),
            darkness: game.darkness_fog(),
            darken_world: game
                .boss_overlay
                .darken_amount(game.clock.fraction() as f32),
            boss_world_fog: game.boss_overlay.should_create_world_fog(),
            spectator: game.spectator(),
            fire_resistance: game.potions.effects.get("fire_resistance").is_some(),
            night_vision: game.fog_night_vision(),
        };
        let mut value = horizon_client::fog::setup_fog(tables, &game.fog, &input);
        if !game.settings.horizon.fog {
            horizon_client::fog::clear_distance_fog(&mut value, &input);
        }
    value.environmental_start = value.render_distance_end.max(0.0);
    value.environmental_end = (value.environmental_start + 1.0).max(value.environmental_end);
    core::ops::ControlFlow::Break(value)
}
