fn freecam_camera(
    game: &mut crate::game::Game<'_>,
) -> core::ops::ControlFlow<Option<CameraEntity>> {
    if let Some(entity) = crate::game::vape_freecam::camera_entity(game) {
        return core::ops::ControlFlow::Break(Some(entity));
    }
    // Freelook: the player's own camera, turned to the freelook's rotation
    // (`applyCameraRotation`, both the rotation and the previous one).
    let Some(camera) = crate::game::vape_freecam::freelook_frame(game) else {
        return core::ops::ControlFlow::Continue(());
    };
    let mut entity = crate::game::vape_freecam::freelook_entered(|| game.player_camera_entity());
    if let Some(entity) = entity.as_mut() {
        entity.x_rot = camera.pitch;
        entity.x_rot_o = camera.pitch;
        entity.y_rot = camera.yaw;
        entity.y_rot_o = camera.yaw;
    }
    core::ops::ControlFlow::Break(entity)
}
