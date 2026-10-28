fn input(
    game: &mut crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
    action: horizon_client::debug_keys::KeyAction,
) {
    use crate::game::vape_rs::{core::with_client, input::KeyAction};
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let accepts_input = game.focused
        && game.player.is_some()
        && game.screen == crate::game::Screen::Playing
        && game.title.is_none();
    let action = match action {
        horizon_client::debug_keys::KeyAction::Release => KeyAction::Release,
        horizon_client::debug_keys::KeyAction::Press => KeyAction::Press,
        horizon_client::debug_keys::KeyAction::Repeat => KeyAction::Repeat,
    };
    let (before, after) = with_client(|runtime| {
        let before = runtime.gui.open;
        runtime.input(
            &format!("{key:?}"),
            action,
            accepts_input && !game.dialog_open() && game.server_packs.prompt.is_none(),
        );
        if !before && runtime.gui.open {
            runtime.ui_screen_owned = true;
        }
        (before, runtime.gui.open)
    });
    if after && !before {
        game.screen = crate::game::Screen::Paused;
        game.pause_menu = false;
        game.capture(false);
        if let Some(window) = &game.window {
            window.set_ime_allowed(true);
        }
    }
    // Closing is deferred to the next client tick, after this input event has
    // passed through the captured host path, so a close click cannot attack.
}
