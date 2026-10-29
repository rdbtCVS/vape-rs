// Parkour consults host collision shapes at the current box and one block ahead.
// It only answers whether the ground ends; vape_input_delays::movement owns the key.
pub(crate) fn edge_ahead(game: &mut crate::game::Game<'_>) -> bool {
    let entities = game.player_entity_collisions();
    let Some(player) = game.player.as_mut() else { return false; };
    if !player.ground.on_ground || player.health.health() <= 0.0 { return false; }
    let bounds = player.bounds();
    let angle = f64::from(game.yaw).to_radians();
    let mut motion = horizon_client::motion::MotionWorld { entities: &entities, player, scene: &mut game.world, assets: &game.pack.assets, collision_binding: game.collision, resources: game.motion };
    let near = bounds.move_by(horizon_core::geometry::Vec3::new(0.0, -0.1, 0.0));
    let far = bounds.move_by(horizon_core::geometry::Vec3::new(-angle.sin(), -0.1, angle.cos()));
    motion.no_player_collision(near).unwrap_or(false) && motion.no_player_collision(far).unwrap_or(false)
}
