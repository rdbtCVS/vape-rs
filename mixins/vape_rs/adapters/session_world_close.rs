fn session_change(game: &mut crate::game::Game<'_>) {
    if game.serving.is_some() || game.options.dedicated.is_some() { return; }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    crate::game::vape_rs::core::with_client(&client, |runtime| runtime.session_change(crate::game::vape_rs::core::lifecycle::CleanupReason::CloseWorld));
}
