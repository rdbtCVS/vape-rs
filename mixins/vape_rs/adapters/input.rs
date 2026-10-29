fn input(
    game: &mut crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
    action: horizon_client::debug_keys::KeyAction,
) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
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
    let macro_context = crate::game::remote::vape_macros::context(game);
    let (before, after) = with_client(&client, |runtime| {
        let before = runtime.gui.open;
        runtime.macro_context(macro_context);
        runtime.input(
            &format!("{key:?}"),
            action,
            accepts_input && !game.dialog_open() && game.server_packs.prompt.is_none(),
        );
        let name = format!("{key:?}");
        if runtime.gui.clipboard_key(&name, action) {
            use horizon_client::clipboard::Clipboard;
            if name == "KeyV" {
                runtime.gui.commit_text(&game.clipboard.get(), false);
            } else if let Some(text) = runtime.gui.copy_selection(name == "KeyX") {
                game.clipboard.set(&text);
            }
        }
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
