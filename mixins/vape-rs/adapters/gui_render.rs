fn gui_render(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    use crate::game::vape_rs::{
        core::with_client,
        ui::gui::{Paint, Rect},
    };
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let frame = with_client(|runtime| runtime.gui_frame((batch.width, batch.height), game.cursor));
    let Some(frame) = frame else {
        return;
    };
    // Keep glyph upload patches already collected by the host, but replace
    // the hidden vanilla pause-menu vertices/ranges with the owned mod UI.
    batch.vertices.clear();
    batch.blends.clear();
    batch.blur_at = 0;
    let atlas = &game.gui.atlas;
    batch.fill(atlas, 0.0, 0.0, batch.width, batch.height, 0x99000000);
    let start = batch.vertices.len();
    for draw in frame.draw {
        let first = batch.vertices.len();
        let argb = match &draw.paint {
            Paint::Fill(_, color) | Paint::Text(_, _, _, color) => *color,
        };
        let color = (argb & 0xff00ff00) | ((argb & 0x00ff0000) >> 16) | ((argb & 0x000000ff) << 16);
        match draw.paint {
            Paint::Fill(Rect(x, y, w, h), _) => batch.fill(atlas, x, y, w, h, color),
            Paint::Text(x, y, text, _) => {
                batch.text(atlas, &text, x, y, color, false);
            }
        }
        let Rect(x, y, w, h) = draw.clip;
        batch.clip_since(first, x, y, x + w, y + h);
    }
    batch.scale_since(start, 0.0, 0.0, frame.scale, frame.scale);
    for vertex in &mut batch.vertices[start..] {
        vertex.pos[0] += frame.origin.0;
        vertex.pos[1] += frame.origin.1;
    }
    batch.clip_since(start, 0.0, 0.0, batch.width, batch.height);
}
