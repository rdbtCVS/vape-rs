fn control_key(game: &crate::game::Game<'_>, key: winit::keyboard::KeyCode) -> core::ops::ControlFlow<Result<()>> {
    if game.serving.is_some() || game.options.dedicated.is_some() { return core::ops::ControlFlow::Continue(()); }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    crate::game::vape_rs::core::with_client(&client, |runtime| {
        if runtime.suppress_game_input() {
            return core::ops::ControlFlow::Break(Ok(()));
        }
        // Socket key presses enter the key set directly, outside the toggle
        // mappings; the movement writer must treat them as user input.
        if let Some(axis) = crate::game::vape_input_delays::axis(key) {
            runtime.movement().direct_key(axis);
        }
        core::ops::ControlFlow::Continue(())
    })
}
