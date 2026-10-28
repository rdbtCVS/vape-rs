fn hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    use crate::game::vape_rs::{core::with_client, ui::Snapshot};
    use horizon_client::hud::HudBatch;
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let panels = with_client(|runtime| {
        runtime.panels(Snapshot {
            position: [player.position.x, player.position.y, player.position.z],
            fps: game.fps,
            in_world: true,
            show_hud: !game.ui.hide_gui && game.screen == crate::game::Screen::Playing,
        })
    });
    let atlas = &game.gui.atlas;
    for panel in panels {
        let texts: Vec<String> = panel
            .lines
            .into_iter()
            .map(|line| {
                if line.label.is_empty() {
                    line.value
                } else {
                    format!("{} {}", line.label, line.value)
                }
            })
            .collect();
        let width = texts
            .iter()
            .map(|s| HudBatch::text_width(atlas, s))
            .fold(0.0f32, f32::max)
            + 8.0;
        let height = texts.len() as f32 * 12.0 + 4.0;
        let x = panel
            .x
            .clamp(0.0, (batch.width - width * panel.scale).max(0.0));
        let y = panel
            .y
            .clamp(0.0, (batch.height - height * panel.scale).max(0.0));
        let start = batch.vertices.len();
        if panel.background {
            batch.fill(atlas, x, y, width, height, 0x99000000);
        }
        let argb = panel.color;
        let color = (argb & 0xff00ff00) | ((argb & 0x00ff0000) >> 16) | ((argb & 0x000000ff) << 16);
        for (index, text) in texts.iter().enumerate() {
            batch.text(
                atlas,
                text,
                x + 4.0,
                y + 3.0 + index as f32 * 12.0,
                color,
                !panel.background,
            );
        }
        batch.scale_since(start, x, y, panel.scale, panel.scale);
        batch.clip_since(start, 0.0, 0.0, batch.width, batch.height);
    }
}
