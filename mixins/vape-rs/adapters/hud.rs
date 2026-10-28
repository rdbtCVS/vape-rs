fn hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    use crate::game::vape_rs::{core::with_client, ui::Snapshot};
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let panels = with_client(&client, |runtime| {
        runtime.panels(Snapshot {
            position: [player.position.x, player.position.y, player.position.z],
            fps: game.fps,
            yaw: game.yaw,
            biome: crate::game::vape_hud::biome(
                game,
                [player.position.x, player.position.y, player.position.z],
            ),
            in_world: true,
            show_hud: !game.ui.hide_gui && game.screen == crate::game::Screen::Playing,
        })
    });
    for panel in panels {
        crate::game::vape_hud::panel(game, batch, &panel);
    }
}
