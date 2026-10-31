// HitSwap, AutoMace and ShieldBreaker on the host: the hotbar with each
// mace's Density/Breach level, the crosshair player's shield hand and the
// nearby player, and the selected slot and attack mapping steps the
// modules ask for.
use crate::game::vape_rs::core::lifecycle::Epoch;
use crate::game::vape_rs::core::modern_combat_runtime::Frame;
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::inventory::item_limit::HeldItem;
use crate::game::vape_rs::modules::combat::trigger_bot::Cooldown;
use crate::game::vape_rs::modules::combat::weapon_swap::{Crosshair, Op, Shield, Slot, Source, Target, View};
use crate::game::vape_rs::targeting::entities::{PlayerView, distance, int_yaw_difference};
use crate::game::{Game, Screen};

fn off(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

/// One hotbar slot, as `vape_combat::held` reads the selected one.
fn slot(game: &Game<'_>, i: usize) -> Slot {
    let stack = game.inventory.get(i).and_then(|s| s.as_ref());
    let info = stack.and_then(|s| game.res.items().get(&s.item));
    Slot {
        item: HeldItem {
            id: stack.map(|s| s.item.trim_start_matches("minecraft:").to_owned()),
            display_name: stack
                .map(|s| horizon_client::stations::hover_name(&|id| game.pack.names.display_item(id), s))
                .unwrap_or_default(),
            block: info.is_some_and(|i| i.block.is_some()),
            food: info.is_some_and(|i| i.food.is_some()),
            potion: stack.is_some_and(|s| s.item.ends_with("potion")),
            slot: i,
        },
        density: stack.map_or(0, |s| s.enchantment_level("minecraft:density") as i32),
        breach: stack.map_or(0, |s| s.enchantment_level("minecraft:breach") as i32),
    }
}

/// `RotationUtil.q`: the off hand when it holds a shield, else the main
/// hand; its use counts from 1 on the tick the use flag arrives.
fn shield(mob: &horizon_client::mobs::Mob) -> Option<Shield> {
    let e = &mob.extra;
    let is_shield = |h: &Option<std::sync::Arc<str>>| h.as_deref().is_some_and(|i| i.trim_start_matches("minecraft:") == "shield");
    let off_hand = if is_shield(&e.off_hand) {
        true
    } else if is_shield(&e.main_hand) {
        false
    } else {
        return None;
    };
    let using = e.using_item > 0 && e.using_off_hand == off_hand;
    Some(Shield { using, elapsed: if using { e.using_item - 1 } else { 0 } })
}

fn armor(mob: &horizon_client::mobs::Mob) -> (f64, f64) {
    let (a, t) = mob.worn_armor();
    (f64::from(mob.kind.armor + a), f64::from(mob.kind.armor_toughness + t))
}

/// The entity under the crosshair before any block: a mob (remote players
/// included) or, when hosting LAN, a guest (through its model).
fn crosshair(game: &Game<'_>) -> Option<Crosshair> {
    let mob = game.mob_hit().filter(|(_, d)| game.before_block(*d)).and_then(|(i, d)| Some((game.mobs.get(i)?, d)));
    let guest = game.guest_hit().filter(|(_, d)| game.before_block(*d)).and_then(|(conn, d)| {
        let host = game.lan.as_ref()?;
        let g = host.hub.guests.iter().find(|g| g.conn == conn)?;
        Some((g.entity_id, host.models.get(&conn), d))
    });
    let mob = mob.map(|(m, d)| {
        let (armor, toughness) = armor(m);
        let player = (&*m.kind.id == "minecraft:player").then(|| Target { shield: shield(m) });
        (Crosshair { id: m.id as i32, player, mace_target: false, armor, toughness }, d)
    });
    let guest = guest.map(|(id, model, d)| {
        let (armor, toughness) = model.map_or((0.0, 0.0), armor);
        let player = Some(Target { shield: model.and_then(shield) });
        (Crosshair { id, player, mace_target: false, armor, toughness }, d)
    });
    match (mob, guest) {
        (Some(m), Some(g)) => Some(if g.1 < m.1 { g.0 } else { m.0 }),
        (m, g) => m.or(g).map(|(c, _)| c),
    }
}

/// `RotationUtil.m(player, 5, 90, true)` before the friend check: other
/// players within 5 blocks and under 90 degrees of yaw, by name.
fn nearby(game: &Game<'_>) -> Vec<(String, Target)> {
    let Some(player) = game.player.as_ref() else { return Vec::new() };
    let me = [player.position.x, player.position.y, player.position.z];
    let view = PlayerView { position: me, tick_position: me, yaw: game.yaw, pitch: game.pitch, strafe: 0.0 };
    let near = |at: [f64; 3]| distance(me, at) < 5.0 && int_yaw_difference(&view, at) < 90;
    let names = game.remote_player_names();
    let mut out = Vec::new();
    for mob in game.mobs.iter().filter(|m| m.alive() && &*m.kind.id == "minecraft:player") {
        if near([mob.position.x, mob.position.y, mob.position.z]) {
            let name = names.iter().find(|(id, _)| *id == mob.id).map_or("", |(_, n)| n.as_str());
            out.push((name.to_owned(), Target { shield: shield(mob) }));
        }
    }
    if let Some(host) = &game.lan {
        for g in host.hub.guests.iter().filter(|g| host.hub.here(g) && g.loaded && !g.dead) {
            if near(g.position) {
                out.push((g.name.clone(), Target { shield: host.models.get(&g.conn).and_then(shield) }));
            }
        }
    }
    out
}

fn frame(game: &Game<'_>, epoch: Epoch, now_ms: u64) -> Frame {
    let player = game.player.as_ref();
    let descending = player.is_some_and(|p| p.position.y - game.previous_position.y < 0.0);
    let fall_distance = player.map_or(0.0, |p| p.fall_distance) as f32;
    let crosshair = crosshair(game);
    // AutoMace's target filter reads the crosshair entity.
    let entity = crosshair.and_then(|c| {
        crate::game::vape_combat::snapshot(game, epoch, None).into_iter().find(|e| e.handle.id == c.id)
    });
    Frame {
        view: View {
            hotbar: (0..9).map(|i| slot(game, i)).collect(),
            selected: game.selected,
            screen_open: game.screen != Screen::Playing || game.dialog_open(),
            using_item: game.using.is_some(),
            crosshair,
            nearby_player: None,
            will_smash: descending && fall_distance >= 1.5,
            on_ground: player.is_some_and(|p| p.ground.on_ground),
            descending,
            fall_distance,
            fall_flying: player.is_some_and(|p| p.fall_flying),
            ticks_existed: game.ticks as i32,
            // The runtime clears it while its click GUI is open.
            input_enabled: true,
            now_ms,
        },
        entity,
        nearby: nearby(game),
    }
}

/// The steps, in order: the selected slot, and the attack mapping the way
/// AutoClicker holds it (`setKeyBindState` + `KeyMapping.click`).
fn apply(game: &mut Game<'_>, ops: &[Op]) {
    for op in ops {
        match *op {
            Op::Select(slot) if slot < 9 => game.selected = slot,
            Op::Select(_) => {}
            Op::Release => game.left = false,
            Op::Press => {
                crate::game::remote::vape_macros::attack_once(game);
                game.left = true;
            }
        }
    }
}

/// Applies what waits (disable cleanup, a post-attack follow-up) and
/// returns the time and session for a new frame, or None when nothing runs.
fn begin(game: &mut Game<'_>) -> Option<(Epoch, u64)> {
    if off(game) {
        return None;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (wanted, pending, epoch, now) = with_client(&client, |runtime| {
        (runtime.modern_combat_wanted(), runtime.modern_combat_take_pending(), runtime.session().epoch(), runtime.now_ms())
    });
    apply(game, &pending);
    (wanted && game.player.is_some()).then_some((epoch, now))
}

/// Client tick HEAD, after TriggerBot: `EventPreTick`.
pub(super) fn tick(game: &mut Game<'_>) {
    let Some((epoch, now)) = begin(game) else { return };
    let frame = frame(game, epoch, now);
    let cooldown = Cooldown {
        ticks: game.hand_strength.ticker as f32,
        delay: horizon_client::attack_strength::delay(game.main_hand_attack_speed()),
    };
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let ops = with_client(&client, |runtime| runtime.modern_combat_tick(frame, cooldown));
    apply(game, &ops);
}

/// The physical attack button went down (`cancelled` when TriggerBot
/// swallowed it). True swallows it.
pub(super) fn press(game: &mut Game<'_>, cancelled: bool) -> bool {
    let Some((epoch, now)) = begin(game) else { return false };
    let frame = frame(game, epoch, now);
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (cancel, ops) = with_client(&client, |runtime| runtime.modern_combat_press(frame, cancelled));
    apply(game, &ops);
    cancel
}

/// `requestSyntheticAttack(source)`: true when the caller may press.
pub(super) fn request(game: &mut Game<'_>, source: Source) -> bool {
    let Some((epoch, now)) = begin(game) else { return true };
    let frame = frame(game, epoch, now);
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (allowed, ops) = with_client(&client, |runtime| runtime.modern_combat_request(source, frame));
    apply(game, &ops);
    allowed
}

/// TriggerBot's `canBypassCooldown` and its Shield check exemption.
pub(super) fn trigger_assist(game: &Game<'_>, epoch: Epoch, now_ms: u64) -> (bool, bool) {
    if off(game) || game.player.is_none() {
        return (false, false);
    }
    let frame = frame(game, epoch, now_ms);
    with_client(&game.__horizon_mixins_vape_rs_client().clone(), |runtime| runtime.modern_combat_trigger_assist(frame))
}

/// Attack HEAD, after Reach and WTap: `EventPostAttack` on the crosshair
/// entity. Its follow-up waits for the next frame.
pub(super) fn post_attack(game: &mut Game<'_>) {
    if off(game) || game.player.is_none() || game.spectator() {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (wanted, epoch, now) = with_client(&client, |runtime| (runtime.modern_combat_wanted(), runtime.session().epoch(), runtime.now_ms()));
    if !wanted {
        return;
    }
    let frame = frame(game, epoch, now);
    let Some(target) = frame.view.crosshair.map(|c| c.id) else { return };
    with_client(&client, |runtime| runtime.modern_combat_post_attack(frame, target));
}
