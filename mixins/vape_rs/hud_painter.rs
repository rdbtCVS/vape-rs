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
/// Armor points an item adds (`ItemArmor.damageReduceAmount`).
fn armor_points(game: &crate::game::Game<'_>, item: Option<&str>) -> f32 {
    let Some(info) = item.and_then(|id| game.res.items().get(id)) else {
        return 0.0;
    };
    info.modifiers
        .iter()
        .filter(|(attribute, ..)| attribute == "minecraft:armor" || attribute == "armor")
        .map(|(_, _, amount, _, _)| *amount as f32)
        .sum()
}
/// `RotationUtil.j`: the fist (1) plus the weapon bonus, reduced by the
/// defender's armor as `(25 - armor) / 25`.
fn hit_damage(game: &crate::game::Game<'_>, weapon: Option<&str>, armor: f32) -> f32 {
    let bonus = weapon
        .and_then(|id| game.res.items().get(id))
        .map_or(0.0, |info| info.attack_damage as f32);
    (1.0 + bonus) * (25.0 - armor) / 25.0
}
fn own_loadout(game: &crate::game::Game<'_>) -> (Option<String>, f32) {
    let weapon = game
        .inventory
        .get(game.selected)
        .and_then(|s| s.as_ref())
        .map(|s| s.item.to_string());
    let armor = game
        .armor
        .iter()
        .map(|s| armor_points(game, s.as_ref().map(|s| &*s.item)))
        .sum();
    (weapon, armor)
}
fn seen_mob(
    game: &crate::game::Game<'_>,
    mob: &horizon_client::mobs::Mob,
    names: &[(u32, String)],
) -> crate::game::vape_rs::ui::target_info::Seen {
    let player = game.player.as_ref();
    let distance = player.map_or(f32::MAX, |p| {
        let (dx, dy, dz) = (
            p.position.x - mob.position.x,
            p.position.y - mob.position.y,
            p.position.z - mob.position.z,
        );
        (dx * dx + dy * dy + dz * dz).sqrt() as f32
    });
    let name = names
        .iter()
        .find(|(id, _)| *id == mob.id)
        .map(|(_, name)| name.clone())
        .or_else(|| mob.custom_name.clone())
        .unwrap_or_else(|| {
            horizon_client::lang::get_or(
                &resource_translation_key("entity", &mob.kind.id),
                &biome_title(&mob.kind.id),
            )
        });
    let (weapon, own_armor) = own_loadout(game);
    let extra = &mob.extra;
    let armor = [&extra.head, &extra.chest, &extra.legs, &extra.feet]
        .into_iter()
        .map(|item| armor_points(game, item.as_deref()))
        .sum();
    let dealt = hit_damage(game, weapon.as_deref(), armor).max(1.0);
    let taken = hit_damage(game, extra.main_hand.as_deref(), own_armor).max(1.0);
    crate::game::vape_rs::ui::target_info::Seen {
        id: mob.id as i32,
        name,
        health: mob.health,
        max_health: horizon_client::mobs::finalize::max_health(mob),
        player: &*mob.kind.id == "minecraft:player",
        distance,
        hurt_time: mob.hurt_time,
        damage: (dealt - taken) as i32,
    }
}
/// The crosshair entity Target Info may show: living, not an armor stand.
fn crosshair_mob<'a>(game: &'a crate::game::Game<'_>) -> Option<&'a horizon_client::mobs::Mob> {
    crosshair_hit(game).map(|(m, _)| m)
}
/// The crosshair entity and the eye-to-hitbox distance along the look ray.
fn crosshair_hit<'a>(game: &'a crate::game::Game<'_>) -> Option<(&'a horizon_client::mobs::Mob, f64)> {
    game.mob_hit()
        .filter(|(_, d)| game.before_block(*d))
        .and_then(|(i, d)| game.mobs.get(i).map(|m| (m, d)))
        .filter(|(m, _)| m.alive() && &*m.kind.id != "minecraft:armor_stand")
}
/// Attack HEAD (`EventPostAttack` on this host): the crosshair entity
/// becomes the Target Info target.
/// Reach Display records the same hit's distance (`calculateIntercept` of
/// the eye ray against the target's box).
pub(super) fn target_attack(game: &crate::game::Game<'_>) {
    let Some((mob, distance)) = crosshair_hit(game) else {
        return;
    };
    let seen = seen_mob(game, mob, &game.remote_player_names());
    let client = game.__horizon_mixins_vape_rs_client().clone();
    crate::game::vape_rs::core::with_client(&client, |runtime| {
        runtime.target_info_attack(seen);
        runtime.reach_display_attack(distance as f32);
    });
}
fn target_feed(game: &crate::game::Game<'_>) -> crate::game::vape_rs::ui::target_info::Feed {
    use crate::game::vape_rs::ui::target_info::{Feed, Seen};
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let tracked_id = crate::game::vape_rs::core::with_client(&client, |runtime| runtime.target_info_id());
    let names = game.remote_player_names();
    let tracked = tracked_id.and_then(|id| {
        game.mobs
            .iter()
            .find(|m| m.id as i32 == id && !m.removed)
            .map(|m| seen_mob(game, m, &names))
    });
    static OWN_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let own = game.player.as_ref().map(|p| Seen {
        name: game
            .options
            .player_name
            .clone()
            .unwrap_or_else(|| OWN_NAME.get_or_init(|| game.options.player_name()).clone()),
        health: p.health.health(),
        max_health: 20.0,
        player: true,
        ..Seen::default()
    });
    Feed {
        hovered: crosshair_mob(game).map(|m| seen_mob(game, m, &names)),
        tracked,
        own_hurt_time: game.player.as_ref().map_or(0, |p| p.health.hurt_animation_time),
        own,
    }
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
        target: target_feed(game),
        scoreboard: None,
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
    if let crate::game::vape_rs::ui::hud::Content::TargetInfo(view) = &panel.content {
        return target_info(atlas, batch, panel, view);
    }
    if let crate::game::vape_rs::ui::hud::Content::Scoreboard { .. } = &panel.content {
        if let Some(rect) = scoreboard(game, batch, panel) {
            return rect;
        }
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
/// Target Info, drawn with the smooth GUI font at the panel's scale.
fn target_info(
    atlas: &horizon_client::hud::HudAtlas,
    batch: &mut HudBatch,
    panel: &HudPanel,
    view: &crate::game::vape_rs::ui::target_info::View,
) -> Rect {
    use crate::game::vape_rs::ui::text_gui::Draw;
    const TEXT_EM: f32 = 9.0;
    let s = panel.scale;
    let start = batch.vertices.len();
    let mut ui = horizon_client::ui_theme::Ui::new(atlas, batch, (0.0, 0.0));
    let out = crate::game::vape_rs::ui::target_info::layout(view, |text, size| {
        ui.vape_rs_measure(text, TEXT_EM * size)
    });
    let (w, h) = (out.width * s, out.height * s);
    let x = panel.x.clamp(0.0, (ui.batch.width - w).max(0.0));
    let y = panel.y.clamp(0.0, (ui.batch.height - h).max(0.0));
    for draw in out.draws {
        match draw {
            Draw::Fill { x: dx, y: dy, w, h, color: argb } => ui.batch.fill(
                atlas,
                x + dx * s,
                y + dy * s,
                w * s,
                h * s,
                color(argb, panel.enabled),
            ),
            Draw::Text { x: dx, y: dy, size, text, color: argb, .. } => ui.vape_rs_text(
                &text,
                x + dx * s,
                y + dy * s,
                TEXT_EM * size * s,
                8.0 * size * s,
                color(argb, panel.enabled),
            ),
        }
    }
    ui.finish();
    batch.clip_since(start, 0.0, 0.0, batch.width, batch.height);
    Rect(x, y, w, h)
}
/// The bottom-right notification stack (`NotificationManager.renderNotifications`).
pub(super) fn notifications(game: &crate::game::Game<'_>, batch: &mut HudBatch) {
    use crate::game::vape_rs::ui::text_gui::Draw;
    const TEXT_EM: f32 = 9.0;
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let atlas = &game.gui.atlas;
    let start = batch.vertices.len();
    let screen = (batch.width, batch.height);
    let mut ui = horizon_client::ui_theme::Ui::new(atlas, batch, (0.0, 0.0));
    let draws = crate::game::vape_rs::core::with_client(&client, |runtime| {
        runtime.notification_frame(screen, |text, size| ui.vape_rs_measure(text, TEXT_EM * size))
    });
    for draw in draws {
        match draw {
            Draw::Fill { x, y, w, h, color: argb } => ui.batch.fill(atlas, x, y, w, h, color(argb, true)),
            Draw::Text { x, y, size, text, color: argb, .. } => {
                ui.vape_rs_text(&text, x, y, TEXT_EM * size, 8.0 * size, color(argb, true));
            }
        }
    }
    ui.finish();
    batch.clip_since(start, 0.0, 0.0, batch.width, batch.height);
}

/// Scoreboard: `Game::sidebar_objective` (private to the scoreboard module),
/// the team's sidebar slot before the shared one.
fn sidebar_objective<'a>(
    game: &crate::game::Game<'_>,
    board: &'a horizon_client::scoreboard::Scoreboard,
) -> Option<&'a horizon_client::scoreboard::Objective> {
    if board.display.is_empty() {
        return None;
    }
    let name = game
        .remote
        .as_ref()
        .map_or(crate::game::chat::PLAYER_NAME, |r| r.profile_name.as_str());
    board
        .players_team(name)
        .and_then(|t| t.color.as_ref())
        .map(|c| format!("sidebar.team.{c}"))
        .and_then(|slot| board.display_objective(&slot))
        .or_else(|| board.display_objective(horizon_client::scoreboard::SLOT_SIDEBAR))
}

/// Scoreboard: the vanilla sidebar draws for a `width` x `height` HUD and
/// their bounds `[left, top, right, bottom]`. Without score numbers the
/// objective and its scores use the blank number format.
pub(super) fn sidebar(
    game: &crate::game::Game<'_>,
    width: f32,
    height: f32,
    numbers: bool,
) -> Option<(Vec<horizon_client::hud_layout::GuiDraw>, [i32; 4])> {
    use horizon_client::hud_layout::GuiDraw;
    use horizon_client::scoreboard::NumberFormat;
    let mut board = std::borrow::Cow::Borrowed(&game.score.board);
    let objective = sidebar_objective(game, &board)?.name.clone();
    if !numbers {
        let board = board.to_mut();
        for scores in board.scores.values_mut() {
            if let Some(score) = scores.get_mut(&objective) {
                score.number_format = Some(NumberFormat::Blank);
            }
        }
        if let Some(o) = board.objectives.iter_mut().find(|o| o.name == objective) {
            o.number_format = Some(NumberFormat::Blank);
        }
    }
    let shown = board.objectives.iter().find(|o| o.name == objective)?;
    let draws = horizon_client::hud_overlays::sidebar(
        width as i32,
        height as i32,
        &board,
        shown,
        &game.gui.atlas.font,
        game.settings.text_background(0.3),
        game.settings.text_background(0.4),
    );
    // The header and body fills span the whole sidebar.
    let mut bounds: Option<[i32; 4]> = None;
    for draw in &draws {
        if let GuiDraw::Fill { x0, y0, x1, y1, .. } = *draw {
            let b = bounds.get_or_insert([x0, y0, x1, y1]);
            *b = [b[0].min(x0), b[1].min(y0), b[2].max(x1), b[3].max(y1)];
        }
    }
    Some((draws, bounds?))
}

/// Scoreboard: the sidebar painted at the panel's position.
fn scoreboard(game: &crate::game::Game<'_>, batch: &mut HudBatch, panel: &HudPanel) -> Option<Rect> {
    use horizon_client::hud_layout::{GuiDraw, PoseOp};
    let numbers = {
        let client = game.__horizon_mixins_vape_rs_client().clone();
        crate::game::vape_rs::core::with_client(&client, |runtime| runtime.scoreboard_numbers())
    };
    let (draws, [left, top, right, bottom]) = sidebar(game, batch.width, batch.height, numbers)?;
    let (w, h) = ((right - left) as f32, (bottom - top) as f32);
    let x = panel.x.clamp(0.0, (batch.width - w).max(0.0));
    let y = panel.y.clamp(0.0, (batch.height - h).max(0.0));
    let mut moved = Vec::with_capacity(draws.len() + 2);
    moved.push(GuiDraw::Push(vec![PoseOp::Translate(x - left as f32, y - top as f32)]));
    moved.extend(draws);
    moved.push(GuiDraw::Pop);
    horizon_client::hud_layout::paint(batch, &game.gui.atlas, &moved);
    Some(Rect(x, y, w, h))
}
