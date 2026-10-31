fn gui_render(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    use crate::game::vape_rs::{
        core::with_client,
        ui::gui::{Align, HudRegion, Paint, Rect},
    };
    // Interface-font em size per game-font pixel scale (the bitmap font is 8px).
    const TEXT_EM: f32 = 9.0;
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let frame = with_client(&client, |runtime| {
        runtime.gui_frame((batch.width, batch.height), game.cursor)
    });
    let Some(frame) = frame else {
        return;
    };
    let atlas = &game.gui.atlas;
    if !frame.hud_editor {
        // The reference's click GUI is its own screen over the game: the HUD
        // stays under it, blurred with the world only by "Blur background"
        // (the gui-blur hook), and there is no pause title or dim.
        crate::game::vape_hud::drop_paused_title(game, batch);
        // Its Text GUI and Target Info overlays draw over that blur.
        let panels = crate::game::vape_hud::snapshot(game, !game.ui.hide_gui)
            .map(|snapshot| with_client(&client, |runtime| runtime.panels(snapshot)))
            .unwrap_or_default();
        with_client(&client, |runtime| runtime.set_text_gui_boxes(Vec::new()));
        for panel in &panels {
            if crate::game::vape_hud::GUI_OVERLAYS.contains(&panel.module) {
                crate::game::vape_hud::panel(game, batch, panel);
            }
        }
    }
    if frame.hud_editor {
        // Retain glyph uploads already gathered by the host while replacing
        // only the owned screen's surface with the HUD previews.
        batch.vertices.clear();
        batch.blends.clear();
        batch.blur_at = 0;
        batch.fill(atlas, 0.0, 0.0, batch.width, batch.height, 0x33000000);
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
    // Draws go straight to their final on-screen place so text is rasterized
    // at its real size in the interface font, never a scaled bitmap.
    let (fs, (ox, oy)) = (frame.scale, frame.origin);
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
                let (x, y, w, h, scale) = (ox + x * fs, oy + y * fs, w * fs, h * fs, scale * fs);
                let (size, line) = (TEXT_EM * scale, 8.0 * scale);
                let mut ui = horizon_client::ui_theme::Ui::new(atlas, batch, (0.0, 0.0));
                let prefix = |n| text.chars().take(n).collect::<String>();
                let cursor_x = ui.vape_rs_measure(&prefix(caret), size);
                let anchor_x = ui.vape_rs_measure(&prefix(anchor), size);
                let scroll = (cursor_x - (w - 9.0 * fs).max(0.0)).max(0.0);
                let left = x + 4.0 * fs - scroll;
                let top = y + (h - line) / 2.0;
                if caret != anchor {
                    ui.batch.fill(
                        atlas,
                        left + cursor_x.min(anchor_x),
                        top,
                        (cursor_x - anchor_x).abs(),
                        line,
                        0xff6b5641,
                    );
                }
                ui.vape_rs_text(&text, left, top, size, line, color);
                ui.batch.fill(atlas, left + cursor_x, top, 0.5 * fs, line, color);
                ui.finish();
            }
            Paint::Fill(Rect(x, y, w, h), _) => {
                batch.fill(atlas, ox + x * fs, oy + y * fs, w * fs, h * fs, color)
            }
            Paint::Text {
                x,
                y,
                text,
                scale,
                align,
                ..
            } => {
                let scale = scale * fs;
                let size = TEXT_EM * scale;
                let mut ui = horizon_client::ui_theme::Ui::new(atlas, batch, (0.0, 0.0));
                let width = ui.vape_rs_measure(&text, size);
                let x = ox + x * fs
                    - match align {
                        Align::Left => 0.0,
                        Align::Center => width / 2.0,
                        Align::Right => width,
                    };
                ui.vape_rs_text(&text, x, oy + y * fs, size, 8.0 * scale, color);
                ui.finish();
            }
        }
        let Rect(x, y, w, h) = draw.clip;
        batch.clip_since(first, ox + x * fs, oy + y * fs, ox + (x + w) * fs, oy + (y + h) * fs);
    }
    batch.clip_since(start, 0.0, 0.0, batch.width, batch.height);
    if !frame.hud_editor {
        crate::game::vape_hud::notifications(game, batch);
    }
}

