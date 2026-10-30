fn gui_render(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    use crate::game::vape_rs::{
        core::with_client,
        ui::gui::{Align, HudRegion, Paint, Rect},
    };
    use horizon_client::hud::HudBatch;
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let frame = with_client(&client, |runtime| {
        runtime.gui_frame((batch.width, batch.height), game.cursor)
    });
    let Some(frame) = frame else {
        return;
    };
    // Retain glyph uploads already gathered by the host while replacing only
    // the owned pause-menu surface. Closing restores the original path.
    batch.vertices.clear();
    batch.blends.clear();
    batch.blur_at = 0;
    let atlas = &game.gui.atlas;
    batch.fill(
        atlas,
        0.0,
        0.0,
        batch.width,
        batch.height,
        if frame.hud_editor {
            0x33000000
        } else {
            0x99000000
        },
    );
    if frame.hud_editor {
        let panels = crate::game::vape_hud::snapshot(game, true)
            .map(|snapshot| {
                with_client(&client, |runtime| {
                    runtime.update_hud(&snapshot);
                    runtime.preview_panels(snapshot)
                })
            })
            .unwrap_or_default();
        let mut regions = Vec::new();
        for panel in panels {
            let Rect(x, y, w, h) = crate::game::vape_hud::panel(game, batch, &panel);
            let outline = if panel.enabled {
                0xff7ea106
            } else {
                0xff737173
            };
            batch.fill(atlas, x, y, w, 1.0, outline);
            batch.fill(atlas, x, y + h - 1.0, w, 1.0, outline);
            batch.fill(atlas, x, y, 1.0, h, outline);
            batch.fill(atlas, x + w - 1.0, y, 1.0, h, outline);
            let hy = if y >= 8.0 { (y - 10.0).max(0.0) } else { y + h };
            let label = batch.vertices.len();
            batch.text(atlas, panel.name, x, hy + 1.0, outline, false);
            batch.clip_since(label, x, hy, x + (w - 26.0).max(0.0), hy + 10.0);
            // Small authored gear and on/off controls, without bundled images.
            let gx = x + w - 22.0;
            let gy = hy + 1.0;
            batch.fill(atlas, gx + 1.0, gy + 1.0, 6.0, 6.0, outline);
            batch.fill(atlas, gx + 3.0, gy + 3.0, 2.0, 2.0, 0xff141414);
            for (tx, ty, tw, th) in [
                (gx + 3.0, gy, 2.0, 1.0),
                (gx + 3.0, gy + 7.0, 2.0, 1.0),
                (gx, gy + 3.0, 1.0, 2.0),
                (gx + 7.0, gy + 3.0, 1.0, 2.0),
            ] {
                batch.fill(atlas, tx, ty, tw, th, outline);
            }
            batch.text(
                atlas,
                if panel.enabled { "x" } else { "+" },
                x + w - 9.0,
                hy + 1.0,
                outline,
                false,
            );
            regions.push(HudRegion {
                id: panel.module.into(),
                rect: Rect(x, y, w, h),
                enabled: panel.enabled,
            });
        }
        with_client(&client, |runtime| runtime.gui.hud_regions = regions);
    }
    let start = batch.vertices.len();
    for draw in frame.draw {
        let first = batch.vertices.len();
        let argb = match &draw.paint {
            Paint::Fill(_, color) | Paint::Text { color, .. } => *color,
            Paint::Edit { .. } => 0xffd1d1d1,
        };
        let color = (argb & 0xff00ff00) | ((argb & 0x00ff0000) >> 16) | ((argb & 0x000000ff) << 16);
        match draw.paint {
            Paint::Edit {
                rect: Rect(x, y, w, h),
                text,
                caret,
                anchor,
                scale,
            } => {
                let prefix = |n| text.chars().take(n).collect::<String>();
                let cursor_x = HudBatch::text_width(atlas, &prefix(caret)) * scale;
                let anchor_x = HudBatch::text_width(atlas, &prefix(anchor)) * scale;
                let scroll = (cursor_x - (w - 9.0).max(0.0)).max(0.0);
                let left = x + 4.0 - scroll;
                let top = y + (h - 7.0 * scale) / 2.0;
                if caret != anchor {
                    batch.fill(
                        atlas,
                        left + cursor_x.min(anchor_x),
                        top - 1.0,
                        (cursor_x - anchor_x).abs(),
                        9.0 * scale,
                        0xff6b5641,
                    );
                }
                let glyphs = batch.vertices.len();
                batch.text(atlas, &text, left, top, color, false);
                batch.scale_since(glyphs, left, top, scale, scale);
                batch.fill(atlas, left + cursor_x, top - 1.0, 0.5, 9.0 * scale, color);
            }
            Paint::Fill(Rect(x, y, w, h), _) => batch.fill(atlas, x, y, w, h, color),
            Paint::Text {
                x,
                y,
                text,
                scale,
                align,
                ..
            } => {
                let width = HudBatch::text_width(atlas, &text) * scale;
                let x = match align {
                    Align::Left => x,
                    Align::Center => x - width / 2.0,
                    Align::Right => x - width,
                };
                batch.text(atlas, &text, x, y, color, false);
                batch.scale_since(first, x, y, scale, scale);
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
