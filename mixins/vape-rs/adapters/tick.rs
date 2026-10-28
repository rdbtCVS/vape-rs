fn tick(game: &mut crate::game::Game<'_>) {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    crate::game::vape_rs::core::with_client(|runtime| {
        if runtime.tick() {
            eprintln!("[vape-rs] client runtime ready; F8: coordinates, F9: FPS, F10: panic");
        }
        if game.player.is_none() || !game.focused || game.screen != crate::game::Screen::Playing {
            runtime.release_input();
        }
    });
}
