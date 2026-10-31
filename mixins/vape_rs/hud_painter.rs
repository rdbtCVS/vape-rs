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
/// `ItemStackScoreUtil.a`: a sword's or tool's default attack-damage
/// modifiers (any slot), 0 for anything else.
fn weapon_bonus(game: &crate::game::Game<'_>, stack: Option<&horizon_client::items::ItemStack>) -> f32 {
    let Some((id, info)) = stack.and_then(|s| game.res.items().get(&s.item).map(|i| (&*s.item, i))) else {
        return 0.0;
    };
    if !id.ends_with("_sword") && !info.components.iter().any(|c| c == "minecraft:tool") {
        return 0.0;
    }
    info.modifiers
        .iter()
        .filter(|(attribute, ..)| attribute == "minecraft:attack_damage" || attribute == "attack_damage")
        .map(|(_, _, amount, _, _)| *amount as f32)
        .sum()
}
/// What `RotationUtil.j` reads from one side: the held stack's Sharpness and
/// weapon bonus, the armor points (`RotationUtil.g`, players only) and the
/// Resistance amplifier.
fn fighter(
    game: &crate::game::Game<'_>,
    held: Option<&horizon_client::items::ItemStack>,
    armor: f32,
    resistance: Option<i32>,
) -> crate::game::vape_rs::ui::target_info::Fighter {
    crate::game::vape_rs::ui::target_info::Fighter {
        sharpness: held.map_or(0, |s| s.enchantment_level("minecraft:sharpness")),
        weapon_bonus: weapon_bonus(game, held),
        armor,
        resistance,
    }
}
/// The local player's side of `RotationUtil.y` and its attack-damage
/// attribute (`b` reads the local player's for both sides).
fn own_fighter(game: &crate::game::Game<'_>) -> (crate::game::vape_rs::ui::target_info::Fighter, Option<f32>) {
    let held = game.inventory.get(game.selected).and_then(|s| s.as_ref());
    let armor = game
        .armor
        .iter()
        .map(|s| armor_points(game, s.as_ref().map(|s| &*s.item)))
        .sum();
    let resistance = game
        .potions
        .effects
        .list
        .iter()
        .find(|e| e.id == "resistance")
        .map(|e| e.amplifier as i32);
    let attack = game.player.as_ref().and_then(|player| {
        player
            .attributes
            .lookup(player.attributes.key("minecraft:attack_damage").as_ref())
            .and_then(|h| player.attributes.instance(&h).ok())
            .map(|i| i.cached_value() as f32)
    });
    (fighter(game, held, armor, resistance), attack)
}
/// `onPlayerUseItem` / `onEntityJoinWorld`: a potion item whose contents
/// (custom effects included) heal: instant health or regeneration.
pub(super) fn heal_potion(stack: &horizon_client::items::ItemStack) -> bool {
    if !matches!(
        &*stack.item,
        "minecraft:potion" | "minecraft:splash_potion" | "minecraft:lingering_potion"
    ) {
        return false;
    }
    let heals = |effect: &str| {
        matches!(effect.strip_prefix("minecraft:").unwrap_or(effect), "instant_health" | "regeneration")
    };
    stack
        .potion()
        .and_then(horizon_client::brewing::potion)
        .is_some_and(|d| d.effects.iter().any(|(effect, _, _)| heals(effect)))
        || stack.custom_potion_effects().iter().any(|(effect, _, _)| heals(effect))
}
/// Target Info runs on the player's game, not on a server thread's.
fn target_pots_skip(game: &crate::game::Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some() || game.guest_use.is_some()
}
/// `TargetInfoHudModule.onPlayerUseItem`: the local player used a heal
/// potion (`use_potion_item` in a local world, `remote_use_item` on a
/// server). A drink counts when its use starts.
pub(super) fn target_heal_used(game: &crate::game::Game<'_>, stack: Option<&horizon_client::items::ItemStack>) {
    let Some(stack) = stack else {
        return;
    };
    if target_pots_skip(game) || (&*stack.item == "minecraft:potion" && game.using.is_some()) || !heal_potion(stack) {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    crate::game::vape_rs::core::with_client(&client, |runtime| runtime.target_info_heal_used());
}
/// `TargetInfoHudModule.onEntityJoinWorld` for a thrown heal potion: at
/// `launch_thrown` in a local world; from a server, when the potion
/// entity's item first arrives (its spawn carries no item: the stack comes
/// in its entity data right after).
pub(super) fn target_heal_joined(game: &crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) {
    if target_pots_skip(game) || !heal_potion(stack) {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    crate::game::vape_rs::core::with_client(&client, |runtime| runtime.target_info_heal_joined());
}
/// The remote side of [`target_heal_joined`]: a thrown potion's item
/// (`ThrowableItemProjectile.DATA_ITEM_STACK`, index 8) counts once, when
/// the stack the host knew for it was not yet a heal potion.
pub(super) fn target_remote_potion(game: &crate::game::Game<'_>, id: i32, stack: Option<horizon_client::items::ItemStack>) {
    let Some(stack) = stack else {
        return;
    };
    let known = game
        .potions
        .thrown_tracked()
        .find(|(thrown, ..)| *thrown == id as u32)
        .is_some_and(|(_, _, known, ..)| heal_potion(known));
    if game.potions.has_thrown(id as u32) && !known {
        target_heal_joined(game, &stack);
    }
}
/// The skin a player is drawn with (the default skin when it has none).
const DEFAULT_SKIN: &str = "entity/player/wide/steve";
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
    let player = &*mob.kind.id == "minecraft:player";
    let extra = &mob.extra;
    let armor = if player {
        [&extra.head, &extra.chest, &extra.legs, &extra.feet]
            .into_iter()
            .map(|item| armor_points(game, item.as_deref()))
            .sum()
    } else {
        0.0
    };
    let resistance = extra
        .effects
        .iter()
        .find(|(id, ..)| *id == "minecraft:resistance" || *id == "resistance")
        .map(|(_, _, amplifier)| *amplifier);
    let target = fighter(game, extra.main_hand_stack(), armor, resistance);
    let (me, attack) = own_fighter(game);
    crate::game::vape_rs::ui::target_info::Seen {
        id: mob.id as i32,
        name,
        health: mob.health,
        max_health: horizon_client::mobs::finalize::max_health(mob),
        player,
        distance,
        hurt_time: mob.hurt_time,
        damage: crate::game::vape_rs::ui::target_info::damage_comparison(attack, &me, &target),
        skin: player.then(|| mob.skin.clone().unwrap_or_else(|| DEFAULT_SKIN.into())),
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
        skin: Some(
            game.player_model
                .as_ref()
                .and_then(|m| m.skin.clone())
                .unwrap_or_else(|| DEFAULT_SKIN.into()),
        ),
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
    let american = crate::game::vape_rs::ui::clock::american_zone();
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
    // `TargetInfoEntityPreviewComponent`: a player's 12x12 face (skin
    // u 8..16, no hat layer) in white at the panel's alpha, drawn with the
    // host's own skin sprite like the tab list.
    if let (Some((fx, fy)), Some(skin)) = (crate::game::vape_rs::ui::target_info::face_origin(view), &view.skin) {
        use horizon_client::hud_layout::{GuiDraw, PoseOp};
        let mut face = vec![GuiDraw::Push(vec![PoseOp::Translate(x + fx * s, y + fy * s), PoseOp::Scale(s, s)])];
        horizon_client::tab_list::face(&mut face, skin, 0, 0, 12, false, false, color(0xffffffff, panel.enabled));
        face.push(GuiDraw::Pop);
        horizon_client::hud_layout::paint(batch, atlas, &face);
    }
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
    let (numbers, replacements) = {
        let client = game.__horizon_mixins_vape_rs_client().clone();
        crate::game::vape_rs::core::with_client(&client, |runtime| {
            (runtime.scoreboard_numbers(), runtime.scoreboard_replacements())
        })
    };
    let (mut draws, [left, top, right, bottom]) = sidebar(game, batch.width, batch.height, numbers)?;
    // At or above 1.16.5 `renderScoreboard` draws the rows and the title
    // components unreplaced; the replaced title only sets the centring.
    if let Some(GuiDraw::Text { text, x, .. }) = draws.get_mut(2)
        && !replacements.is_empty()
    {
        let font = &game.gui.atlas.font;
        let plain = text.plain();
        let replaced = crate::game::vape_rs::ui::scoreboard::replace_all(&replacements, &plain);
        // The host centred the title by its own width (`title_width`).
        let original = horizon_client::hud_layout::width(font, text);
        let shown = horizon_client::hud_layout::width(font, &horizon_client::text::Text::legacy(&replaced));
        *x += original / 2 - shown / 2;
    }
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
