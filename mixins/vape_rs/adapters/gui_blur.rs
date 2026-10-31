fn gui_blur(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<Option<f32>> {
    use crate::game::vape_rs::core::with_client;
    if game.title.is_some() || game.player.is_none() {
        return core::ops::ControlFlow::Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    match with_client(&client, |runtime| runtime.gui_blur()) {
        None => core::ops::ControlFlow::Continue(()),
        Some(false) => core::ops::ControlFlow::Break(None),
        // The menu blur radius, or the vanilla default when it is turned off.
        Some(true) => {
            let radius = game.settings.menu_blur as f32;
            core::ops::ControlFlow::Break(Some(if radius >= 1.0 { radius } else { 5.0 }))
        }
    }
}
