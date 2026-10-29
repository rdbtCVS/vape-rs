fn mapped_key(game: &crate::game::Game<'_>, key: winit::keyboard::KeyCode, pressed: bool) -> core::ops::ControlFlow<Result<()>> {
    if game.serving.is_some() || game.options.dedicated.is_some() { return core::ops::ControlFlow::Continue(()); }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    crate::game::vape_rs::core::with_client(&client, |runtime| {
        if runtime.suppress_game_input() {
            return core::ops::ControlFlow::Break(Ok(()));
        }
        // Every canonical key event the host processes (rebound keys,
        // controllers, socket input) keeps the movement writer's view of
        // what the user holds current.
        if let Some(axis) = crate::game::vape_input_delays::axis(key) {
            runtime.movement().host_event(axis, pressed);
        }
        core::ops::ControlFlow::Continue(())
    })
}
