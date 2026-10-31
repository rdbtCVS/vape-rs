fn freecam_turn(game: &crate::game::Game<'_>, xo: f64, yo: f64) -> core::ops::ControlFlow<()> {
    if crate::game::vape_freecam::turn(game, xo, yo) || crate::game::vape_freecam::freelook_turn(game, xo, yo) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
