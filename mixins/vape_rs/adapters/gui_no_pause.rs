fn gui_no_pause(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    use crate::game::vape_rs::core::try_with_client;
    // ClientSettings opens no vanilla screen, so `Minecraft.pause` keeps a
    // single-player world ticking behind the click GUI and the HUD editor.
    // The Paused screen the GUI owns for input must not pause it either.
    if game.screen != crate::game::Screen::Paused || game.pause_menu {
        return core::ops::ControlFlow::Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    match try_with_client(&client, |runtime| runtime.ui_screen_owned) {
        Some(true) => core::ops::ControlFlow::Break(false),
        _ => core::ops::ControlFlow::Continue(()),
    }
}
