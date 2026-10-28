fn tick(game: &mut crate::game::Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    crate::game::vape_rs::core::with_client(&client, |runtime| {
        runtime.observe_context(crate::game::vape_rs::core::lifecycle::Context {
            in_world: game.player.is_some() && game.title.is_none(),
            alive: game.player.as_ref().is_some_and(|p| p.health.health() > 0.0),
            interactive: game.focused && game.screen == crate::game::Screen::Playing && game.title.is_none() && !game.dialog_open() && game.server_packs.prompt.is_none(),
        });
        if !runtime.initialized() {
            let home = horizon_client::logging::game_home();
            runtime.attach_storage(
                home.canonicalize()
                    .unwrap_or(home)
                    .join("native-mods/vape-rs"),
            );
        }
        if game.player.is_none() || !game.focused || game.screen != crate::game::Screen::Playing {
            runtime.release_input();
        }
        if runtime.tick() {
            eprintln!("[vape-rs] client runtime ready; F8: coordinates, F9: FPS, F10: panic");
        }
        for note in runtime.drain_notes() {
            eprintln!("[vape-rs] {note}");
        }
    });
}
