fn hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    use crate::game::vape_rs::core::with_client;
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let Some(snapshot) = crate::game::vape_hud::snapshot(
        game,
        !game.ui.hide_gui && game.screen == crate::game::Screen::Playing,
    ) else {
        return;
    };
    let panels = with_client(&client, |runtime| {
        runtime.update_hud(&snapshot);
        runtime.panels(snapshot)
    });
    for panel in panels {
        crate::game::vape_hud::panel(game, batch, &panel);
    }
}
