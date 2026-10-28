fn hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    use crate::game::vape_rs::{core::with_client, ui::Snapshot};
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let lines = with_client(|runtime| {
        runtime.hud(Snapshot {
            position: [player.position.x, player.position.y, player.position.z],
            fps: game.fps,
            in_world: game.player.is_some(),
            show_hud: !game.ui.hide_gui && game.screen == crate::game::Screen::Playing,
        })
    });
    let atlas = &game.gui.atlas;
    let mut y = 8.0;
    for line in lines {
        let text = if line.label.is_empty() {
            line.value
        } else {
            format!("{} {}", line.label, line.value)
        };
        batch.text(atlas, &text, 8.0, y, 0xffffffff, true);
        y += 12.0;
    }
}
