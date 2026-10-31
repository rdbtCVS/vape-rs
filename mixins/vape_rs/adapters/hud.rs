fn hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    use crate::game::vape_rs::core::with_client;
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    // The HUD stays drawn under the open click GUI, whose own hook draws
    // the Text GUI, Target Info and the toasts over the GUI blur.
    let under_gui = with_client(&client, |runtime| runtime.gui_shows_hud());
    let Some(mut snapshot) = crate::game::vape_hud::snapshot(
        game,
        !game.ui.hide_gui && (game.screen == crate::game::Screen::Playing || under_gui),
    ) else {
        return;
    };
    // Scoreboard: the panel takes the size of the sidebar it moves.
    let numbers = with_client(&client, |runtime| runtime.scoreboard_numbers());
    snapshot.scoreboard = crate::game::vape_hud::sidebar(game, batch.width, batch.height, numbers)
        .map(|(_, [left, top, right, bottom])| ((right - left) as f32, (bottom - top) as f32));
    let panels = with_client(&client, |runtime| {
        runtime.update_hud(&snapshot);
        runtime.panels(snapshot.clone())
    });
    for panel in panels {
        if under_gui && crate::game::vape_hud::GUI_OVERLAYS.contains(&panel.module) {
            continue;
        }
        crate::game::vape_hud::panel(game, batch, &panel);
    }
    if !under_gui {
        crate::game::vape_hud::notifications(game, batch);
    }
}
