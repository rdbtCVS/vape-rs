fn night_vision(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<f32> {
    if game.serving.is_some() || game.options.dedicated.is_some() || game.player.is_none() || game.title.is_some() {
        return core::ops::ControlFlow::Continue(());
    }
    let enabled = crate::game::vape_rs::core::with_client(game.__horizon_mixins_vape_rs_client(), |runtime| runtime.fullbright_night_vision());
    if enabled { core::ops::ControlFlow::Break(1.0) } else { core::ops::ControlFlow::Continue(()) }
}
