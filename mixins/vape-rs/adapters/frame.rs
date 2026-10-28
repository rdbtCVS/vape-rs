fn frame(game: &mut crate::game::Game<'_>) {
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
        if runtime.ui_screen_owned {
            if runtime.gui.open
                && (game.player.is_none()
                    || game.title.is_some()
                    || game.screen != crate::game::Screen::Paused
                    || game.pause_menu
                    || game.dialog_open()
                    || game.server_packs.prompt.is_some())
            {
                runtime.gui.close();
            }
            if !runtime.gui.open {
                runtime.ui_screen_owned = false;
                if game.screen == crate::game::Screen::Paused && !game.pause_menu {
                    game.screen = crate::game::Screen::Playing;
                    game.pause_menu = true;
                    game.capture(true);
                    if let Some(window) = &game.window {
                        window.set_ime_allowed(false);
                    }
                }
            }
        }
        runtime.frame();
        for note in runtime.drain_notes() {
            eprintln!("[vape-rs] {note}");
        }
    });
}
