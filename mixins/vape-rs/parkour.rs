// Parkour consults host collision shapes at the current box and one block ahead.
pub(crate) fn tick(game: &mut crate::game::Game<'_>) {
    use winit::keyboard::KeyCode;
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (enabled, previous) = crate::game::vape_rs::core::with_client(&client, |runtime| {
        (runtime.enabled(crate::game::vape_rs::core::ModuleId::Parkour), runtime.replace_parkour_input_owner(false))
    });
    if previous && !game.physical_keys.contains(&KeyCode::Space) { game.keys.remove(&KeyCode::Space); }
    if previous || !enabled || !game.focused || game.screen != crate::game::Screen::Playing || game.title.is_some() || game.sleeping.is_some() || game.dialog_open() || game.server_packs.prompt.is_some() || game.spectator() || game.keys.contains(&KeyCode::Space) || !game.keys.contains(&KeyCode::KeyW) { return; }
    let entities = game.player_entity_collisions();
    let Some(player) = game.player.as_mut() else { return; };
    if !player.ground.on_ground || player.health.health() <= 0.0 { return; }
    let bounds = player.bounds();
    let angle = f64::from(game.yaw).to_radians();
    let mut motion = horizon_client::motion::MotionWorld { entities: &entities, player, scene: &mut game.world, assets: &game.pack.assets, collision_binding: game.collision, resources: game.motion };
    let near = bounds.move_by(horizon_core::geometry::Vec3::new(0.0, -0.1, 0.0));
    let far = bounds.move_by(horizon_core::geometry::Vec3::new(-angle.sin(), -0.1, angle.cos()));
    if motion.no_player_collision(near).unwrap_or(false) && motion.no_player_collision(far).unwrap_or(false) {
        game.keys.insert(KeyCode::Space);
        crate::game::vape_rs::core::with_client(&client, |runtime| { runtime.replace_parkour_input_owner(true); });
    }
}
