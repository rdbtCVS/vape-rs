fn freecam_body(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    if crate::game::vape_freecam::body(game) {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
