use crate::game::vape_rs::ui::{
    HudPanel,
    gui::Rect,
    hud::{Command, biome_title},
};
use horizon_client::hud::HudBatch;

pub(super) fn biome(game: &crate::game::Game<'_>, position: [f64; 3]) -> String {
    let [x, y, z] = position.map(|v| v.floor() as i32);
    game.world
        .chunks
        .get(&(x >> 4, z >> 4))
        .map(|chunk| biome_title(&game.res.biomes().get(chunk.biome(x, y, z)).name))
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
                name: biome_title(effect.id),
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
