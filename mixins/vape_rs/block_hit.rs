// BlockHit's host side: the view of the local player for its modes, the use
// key they press and release (`KeyBinding.setPressed`, the synthetic
// right-button messages as `use_once` + the use mapping), Auto/Predict's
// cancelled use presses (`EventMouseButton`), Predict's damage notice, the
// clickers' simulated block hits and Lag's held release-use packet
// (`EventPacketSend` at LOWEST: after the other send hooks).
use super::{Game, Screen};
use crate::game::vape_rs::core::{try_with_client, with_client};
use crate::game::vape_rs::modules::combat::block_hit::{Input, Key, Mode, Send};
use crate::game::vape_rs::targeting::entities::PlayerView;
use core::ops::ControlFlow::{self, Break, Continue};
use horizon_protocol::packets::play::{Clientbound as C, Serverbound as S};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

thread_local! {
    /// A use press Auto/Predict cancelled: the physical button is still down.
    static SWALLOWED: Cell<bool> = const { Cell::new(false) };
    /// AutoClicker's simulated block, let go with its attack.
    static SIMULATED: Cell<bool> = const { Cell::new(false) };
    /// SilentAura's simulated block, let go on the next tick.
    static SILENT: Cell<bool> = const { Cell::new(false) };
    /// Lag's held packets, in send order.
    static QUEUE: RefCell<VecDeque<S>> = const { RefCell::new(VecDeque::new()) };
}

fn local(game: &Game<'_>) -> bool {
    game.serving.is_none() && game.options.dedicated.is_none()
}

fn no_screen(game: &Game<'_>) -> bool {
    game.screen == Screen::Playing && game.title.is_none() && !game.dialog_open() && game.server_packs.prompt.is_none()
}

/// A sword in the main hand (`ItemStack.getItem() instanceof ItemSword`).
fn sword(game: &Game<'_>) -> bool {
    game.held().is_some_and(|s| {
        game.res.items().item_tags.get("minecraft:swords").is_some_and(|tag| tag.contains(&*s.item))
    })
}

/// The host's part of BlockHit's view.
fn input(game: &Game<'_>) -> Input {
    let Some(p) = game.player.as_ref() else {
        return Input::default();
    };
    let hurt = p.health.hurt_animation_time;
    let sword = sword(game);
    Input {
        player: p.health.health() > 0.0,
        holding_sword: no_screen(game) && sword,
        sword_in_hand: sword,
        use_down: game.toggles.is_down(3) || SWALLOWED.with(Cell::get),
        attack_down: game.toggles.is_down(2),
        using: game.using.is_some(),
        hurt_time: hurt,
        // `hurtResistantTime`: 20 at a hit while the hurt time counts 10.
        hurt_resistant_time: if hurt > 0 { hurt + 10 } else { 0 },
        focused: game.focused,
        targeting_entity: game.mob_hit().is_some_and(|(_, d)| game.before_block(d)),
        ..Input::default()
    }
}

/// The player as the server sees it: the managed rotation when one is held.
fn player_view(game: &Game<'_>) -> PlayerView {
    let Some(p) = game.player.as_ref() else {
        return PlayerView::default();
    };
    let position = [p.position.x, p.position.y, p.position.z];
    let (yaw, pitch) = crate::game::remote::vape_combat_remote::rotation((game.yaw, game.pitch));
    PlayerView { position, tick_position: position, yaw, pitch, strafe: 0.0 }
}

fn apply(game: &mut Game<'_>, key: Key) {
    match key {
        Key::Press => game.right = true,
        Key::Click => {
            crate::game::remote::vape_macros::use_once(game);
            game.right = true;
        }
        Key::Release => game.right = false,
    }
}

/// Sends what Lag held, in order.
fn flush(game: &Game<'_>) {
    let packets: Vec<S> = QUEUE.with(|q| q.borrow_mut().drain(..).collect());
    if let Some(r) = &game.remote {
        for packet in packets {
            r.session.send(packet);
        }
    }
}

/// `EventPreTick`, after HitSelect: Lag's released queue, SilentAura's
/// simulated block, then the mode's use key.
pub(crate) fn tick(game: &mut Game<'_>) {
    if !local(game) {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let mode = with_client(&client, |rt| rt.block_hit_mode());
    let buffering = with_client(&client, |rt| rt.block_hit_buffering());
    let gone = game.player.is_none() || game.remote.is_none();
    if buffering && gone {
        // No player to block with: what Lag held goes, the claim with it.
        with_client(&client, |rt| rt.block_hit_flushed());
    }
    if QUEUE.with(|q| !q.borrow().is_empty()) && (!buffering || gone) {
        flush(game);
    }
    if !game.focused || game.screen != Screen::Playing {
        SWALLOWED.with(|s| s.set(false));
    }
    if SILENT.with(|s| s.replace(false)) {
        game.right = false;
    }
    if mode.is_none() || game.player.is_none() {
        return;
    }
    let input = input(game);
    let player = player_view(game);
    let epoch = with_client(&client, |rt| rt.session().epoch());
    let entities = if matches!(mode, Some(Mode::Lag | Mode::Predict)) {
        super::vape_combat::snapshot(game, epoch, None)
    } else {
        Vec::new()
    };
    if let Some(key) = with_client(&client, |rt| rt.block_hit_tick(input, &player, &entities)) {
        apply(game, key);
    }
}

/// `play_mouse` HEAD below the binds: Manual's block at an attack press,
/// Auto/Predict's cancelled use press while attacking with a sword.
pub(crate) fn mouse(game: &mut Game<'_>, button: winit::event::MouseButton, pressed: bool) -> ControlFlow<()> {
    use winit::event::MouseButton;
    if !local(game) || game.screen != Screen::Playing {
        return Continue(());
    }
    let attack = match button {
        MouseButton::Left => true,
        MouseButton::Right => false,
        _ => return Continue(()),
    };
    if !pressed {
        if !attack {
            SWALLOWED.with(|s| s.set(false));
        }
        return Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let input = input(game);
    let mouse = with_client(&client, |rt| rt.block_hit_press(input, attack));
    if let Some(key) = mouse.key {
        apply(game, key);
    }
    if mouse.cancel {
        SWALLOWED.with(|s| s.set(true));
        Break(())
    } else {
        Continue(())
    }
}

/// `ClickerMod.shouldSimulateBlockHit` at AutoClicker's press: the use key
/// goes down with the attack.
pub(crate) fn clicker_press(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let input = input(game);
    if with_client(&client, |rt| rt.block_hit_simulate(input)) {
        apply(game, Key::Click);
        SIMULATED.with(|s| s.set(true));
    }
}

/// AutoClicker's release: the simulated block goes up with the attack.
pub(crate) fn clicker_release(game: &mut Game<'_>) {
    if SIMULATED.with(|s| s.replace(false)) {
        game.right = false;
    }
}

/// SilentAuraClicker's click: a simulated block held until the next tick.
pub(crate) fn silent_click(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let input = input(game);
    if with_client(&client, |rt| rt.block_hit_simulate(input)) {
        apply(game, Key::Click);
        SILENT.with(|s| s.set(true));
    }
}

/// Inbound: Predict's `onDamaged` (an entity status for the local player):
/// a damage event, or the shield's block sound (status 29).
pub(crate) fn inbound(game: &Game<'_>, packet: &C) {
    let Some(me) = game.remote.as_ref().map(|r| r.entity_id) else {
        return;
    };
    let hit = match packet {
        C::DamageEvent(p) => p.id == me,
        C::EntityEvent(p) => p.event == 29 && p.id == me,
        _ => false,
    };
    if hit {
        let client = game.__horizon_mixins_vape_rs_client().clone();
        with_client(&client, |rt| rt.block_hit_damaged());
    }
}

/// `remote::Game::send` HEAD below vape-rs's own: Lag holds the release-use
/// packet and everything after it until its delay ends or the target is lost.
pub(crate) fn send(game: &Game<'_>, packet: &S) -> ControlFlow<()> {
    if game.remote.is_none() || game.player.is_none() || !local(game) {
        return Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let release_use = matches!(packet, S::PlayerAction(a) if a.action == 6);
    let Some((lag, buffering)) =
        try_with_client(&client, |rt| (rt.block_hit_mode() == Some(Mode::Lag), rt.block_hit_buffering()))
    else {
        return Continue(());
    };
    let queued = QUEUE.with(|q| !q.borrow().is_empty());
    if !lag {
        if queued {
            flush(game);
        }
        return Continue(());
    }
    // The target and the sword decide only while buffering.
    let (target, sword) = if buffering {
        let epoch = try_with_client(&client, |rt| rt.session().epoch());
        let entities = epoch.map(|e| super::vape_combat::snapshot(game, e, None)).unwrap_or_default();
        let player = player_view(game);
        (
            try_with_client(&client, |rt| rt.block_hit_target(&player, &entities)).unwrap_or(false),
            no_screen(game) && sword(game),
        )
    } else {
        (false, false)
    };
    let Some(verdict) = try_with_client(&client, |rt| rt.block_hit_send(release_use, target, sword)) else {
        return Continue(());
    };
    match verdict {
        Send::Queue => {
            QUEUE.with(|q| q.borrow_mut().push_back(packet.clone()));
            Break(())
        }
        Send::Flush | Send::Pass => {
            if queued {
                flush(game);
            }
            Continue(())
        }
    }
}
