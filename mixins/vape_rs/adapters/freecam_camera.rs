fn freecam_camera(
    game: &crate::game::Game<'_>,
) -> core::ops::ControlFlow<Option<CameraEntity>> {
    match crate::game::vape_freecam::camera_entity(game) {
        Some(entity) => core::ops::ControlFlow::Break(Some(entity)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
