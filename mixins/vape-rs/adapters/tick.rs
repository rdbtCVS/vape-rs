fn tick(game: &mut crate::game::Game<'_>) {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    crate::game::vape_rs::core::with_client(|runtime| {
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
