use crate::game::vape_rs::ui::{
    HudPanel,
    gui::Rect,
    hud::{Command, biome_title},
    resource_names::resource_translation_key,
};
use horizon_client::hud::HudBatch;

pub(super) fn biome(game: &crate::game::Game<'_>, position: [f64; 3]) -> String {
    let [x, y, z] = position.map(|v| v.floor() as i32);
    game.world
        .chunks
        .get(&(x >> 4, z >> 4))
        .map(|chunk| {
            let name = &game.res.biomes().get(chunk.biome(x, y, z)).name;
            horizon_client::lang::get_or(&resource_translation_key("biome", name), &biome_title(name))
        })
        .unwrap_or_default()
}
pub(super) fn snapshot(
    game: &crate::game::Game<'_>,
    show_hud: bool,
) -> Option<crate::game::vape_rs::ui::Snapshot> {
    use crate::game::vape_rs::ui::{
        Snapshot,
        clock::ClockTime,
        hud::{ArmorPiece, PotionView},
    };
    use chrono::{Datelike, Timelike};
    use winit::keyboard::KeyCode;
    let player = game.player.as_ref()?;
    let position = [player.position.x, player.position.y, player.position.z];
    let now = chrono::Local::now();
    static AMERICAN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let american = *AMERICAN.get_or_init(|| {
        std::env::var("TZ")
            .ok()
            .is_some_and(|s| s.contains("America"))
            || std::fs::read_link("/etc/localtime")
                .ok()
                .is_some_and(|p| p.to_string_lossy().contains("America"))
            || std::fs::read_to_string("/etc/timezone")
                .ok()
                .is_some_and(|s| s.contains("America"))
    });
    let armor = std::array::from_fn(|slot| {
        game.armor[slot].as_ref().map(|stack| ArmorPiece {
            slot,
            max_damage: game
                .res
                .items()
                .get(&stack.item)
                .map_or(0, |i| i.max_damage),
            damage: stack.damage,
            preview: false,
            broken: false,
        })
    });
    let effects = game
        .potions
        .effects
        .list
        .iter()
        .take(64)
        .map(|effect| {
            let icon = horizon_client::hud_overlays::EffectIcon::from(effect);
            PotionView {
                id: icon.id,
                name: horizon_client::lang::get_or(
                    &resource_translation_key("effect", effect.id),
                    &biome_title(effect.id),
                ),
                duration: effect.duration,
                maximum: effect.duration,
                negative: !icon.beneficial,
                amplifier: effect.amplifier,
                color: 0xff000000 | (icon.color as u32 & 0xffffff),
            }
        })
        .collect();
    Some(Snapshot {
        position,
        fps: game.fps,
        yaw: game.yaw,
        biome: biome(game, position),
        clock: ClockTime {
            hour: now.hour() as u8,
            minute: now.minute() as u8,
            second: now.second() as u8,
            day: now.day() as u8,
            month: now.month() as u8,
            weekday: now.weekday().num_days_from_monday() as u8,
            american_date: american,
        },
        keys: [
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::Space,
        ]
        .map(|k| game.keys.contains(&k)),
        mouse: [game.left, game.right],
        armor,
        effects,
        in_world: game.title.is_none(),
        show_hud,
    })
}
fn line(
    batch: &mut HudBatch,
    atlas: &horizon_client::hud::HudAtlas,
    from: [f32; 2],
    to: [f32; 2],
    width: f32,
    color: u32,
) {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return;
    }
    let nx = -dy / length * width / 2.0;
    let ny = dx / length * width / 2.0;
    batch.fill_quad(
        atlas,
        [
            [from[0] + nx, from[1] + ny],
            [to[0] + nx, to[1] + ny],
            [to[0] - nx, to[1] - ny],
            [from[0] - nx, from[1] - ny],
        ],
        color,
    );
}
fn color(argb: u32, enabled: bool) -> u32 {
    let alpha = if enabled {
        argb & 0xff000000
    } else {
        ((argb >> 24) / 3) << 24
    };
    alpha | (argb & 0x0000ff00) | ((argb & 0x00ff0000) >> 16) | ((argb & 0x000000ff) << 16)
}
pub(super) fn panel(game: &crate::game::Game<'_>, batch: &mut HudBatch, panel: &HudPanel) -> Rect {
    let atlas = &game.gui.atlas;
    if let crate::game::vape_rs::ui::hud::Content::TextGui(view) = &panel.content {
        return text_gui(atlas, batch, panel, view);
    }
    let layout = panel.layout(|text| HudBatch::text_width(atlas, text));
    let w = layout.width * panel.scale;
    let h = layout.height * panel.scale;
    let x = panel.x.clamp(0.0, (batch.width - w).max(0.0));
    let y = panel.y.clamp(0.0, (batch.height - h).max(0.0));
    let start = batch.vertices.len();
    if panel.background {
        batch.fill(
            atlas,
            x,
            y,
            layout.width,
            layout.height,
            color(0x99000000, panel.enabled),
        );
    }
    for command in layout.commands {
        match command {
            Command::Line {
                from,
                to,
                width,
                color: argb,
            } => line(
                batch,
                atlas,
                [x + from[0], y + from[1]],
                [x + to[0], y + to[1]],
                width,
                color(argb, panel.enabled),
            ),
            Command::Arc {
                center,
                radius,
                start,
                end,
                width,
                color: argb,
            } => {
                let span = end - start;
                let steps = ((span.abs() / 6.0).ceil() as usize).clamp(1, 64);
                for step in 0..steps {
                    let a = (start + span * step as f32 / steps as f32).to_radians();
                    let b = (start + span * (step + 1) as f32 / steps as f32).to_radians();
                    line(
                        batch,
                        atlas,
                        [
                            x + center[0] + a.cos() * radius,
                            y + center[1] + a.sin() * radius,
                        ],
                        [
                            x + center[0] + b.cos() * radius,
                            y + center[1] + b.sin() * radius,
                        ],
                        width,
                        color(argb, panel.enabled),
                    );
                }
            }
            Command::ArmorIcon {
                slot,
                x: tx,
                y: ty,
                preview,
            } => {
                let placeholder = preview.then(|| horizon_client::items::ItemStack {
                    item: std::sync::Arc::from(
                        [
                            "minecraft:diamond_helmet",
                            "minecraft:diamond_chestplate",
                            "minecraft:diamond_leggings",
                            "minecraft:diamond_boots",
                        ][slot],
                    ),
                    count: 1,
                    damage: 0,
                    extra: None,
                });
                if let Some(stack) = game.armor[slot].as_ref().or(placeholder.as_ref()) {
                    let first = batch.vertices.len();
                    game.draw_item_model(batch, stack, x + tx, y + ty);
                    if !panel.enabled {
                        for vertex in &mut batch.vertices[first..] {
                            vertex.color =
                                (vertex.color & 0x00ffffff) | ((vertex.color >> 24) / 3 << 24);
                        }
                    }
                }
            }
            Command::Sprite {
                id,
                x: tx,
                y: ty,
                width,
                height,
            } => batch.sprite_scaled(
                atlas,
                &id,
                x + tx,
                y + ty,
                width,
                height,
                color(0xffffffff, panel.enabled),
            ),
            Command::Text {
                x: tx,
                y: ty,
                scale,
                text,
                color: argb,
            } => {
                let first = batch.vertices.len();
                batch.text(
                    atlas,
                    &text,
                    x + tx,
                    y + ty,
                    color(argb, panel.enabled),
                    !panel.background,
                );
                batch.scale_since(first, x + tx, y + ty, scale, scale);
            }
            Command::Fill {
                x: tx,
                y: ty,
                width,
                height,
                color: argb,
            } => batch.fill(
                atlas,
                x + tx,
                y + ty,
                width,
                height,
                color(argb, panel.enabled),
            ),
            Command::Triangle {
                points,
                color: argb,
            } => {
                let points = points.map(|[px, py]| [x + px, y + py]);
                batch.fill_quad(
                    atlas,
                    [points[0], points[1], points[2], points[2]],
                    color(argb, panel.enabled),
                );
            }
        }
    }
    batch.scale_since(start, x, y, panel.scale, panel.scale);
    // Compass headings extend above the strip, as specified by its frame.
    if panel.module == "hud.compass" {
        batch.clip_since(start, x, y - 10.0 * panel.scale, x + w, y + h);
    }
    batch.clip_since(start, 0.0, 0.0, batch.width, batch.height);
    Rect(x, y, w, h)
}

/// The Text GUI list: anchored to whichever half of the screen it sits in,
/// drawn with the smooth GUI font or the game font.
fn text_gui(
    atlas: &horizon_client::hud::HudAtlas,
    batch: &mut HudBatch,
    panel: &HudPanel,
    view: &crate::game::vape_rs::ui::text_gui::View,
) -> Rect {
    use crate::game::vape_rs::ui::text_gui::{Draw, layout};
    const TEXT_EM: f32 = 9.0;
    let s = panel.scale;
    let start = batch.vertices.len();
    let mut ui = horizon_client::ui_theme::Ui::new(atlas, batch, (0.0, 0.0));
    let measure = |text: &str, size: f32| {
        if view.smooth {
            ui.vape_rs_measure(text, TEXT_EM * size)
        } else {
            HudBatch::text_width(atlas, text) * size
        }
    };
    let width = layout(view, s, true, measure).width;
    let screen = (ui.batch.width, ui.batch.height);
    let right = panel.x + width / 2.0 >= screen.0 / 2.0;
    let out = layout(view, s, right, measure);
    let x = panel.x.clamp(0.0, (screen.0 - out.width).max(0.0));
    let y = panel.y.clamp(0.0, (screen.1 - out.height).max(0.0));
    for draw in out.draws {
        match draw {
            Draw::Fill {
                x: dx,
                y: dy,
                w,
                h,
                color: argb,
            } => ui
                .batch
                .fill(atlas, x + dx, y + dy, w, h, color(argb, panel.enabled)),
            Draw::Text {
                x: dx,
                y: dy,
                size,
                text,
                color: argb,
                shadow,
            } => {
                // The shadow sits one font pixel down-right and fades with
                // the text it belongs to.
                let alpha = argb >> 24;
                let layers = shadow
                    .map(|shadow| {
                        let a = (shadow >> 24) * alpha / 255;
                        (size, (a << 24) | (shadow & 0xffffff))
                    })
                    .into_iter()
                    .chain(std::iter::once((0.0, argb)));
                for (offset, argb) in layers {
                    let (tx, ty) = (x + dx + offset, y + dy + offset);
                    let abgr = color(argb, panel.enabled);
                    if view.smooth {
                        ui.vape_rs_text(&text, tx, ty, TEXT_EM * size, 8.0 * size, abgr);
                    } else {
                        let first = ui.batch.vertices.len();
                        ui.batch.text(atlas, &text, tx, ty, abgr, false);
                        ui.batch.scale_since(first, tx, ty, size, size);
                    }
                }
            }
        }
    }
    ui.finish();
    batch.clip_since(start, 0.0, 0.0, batch.width, batch.height);
    Rect(x, y, out.width, out.height)
}
