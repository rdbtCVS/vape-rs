// Blink, FakeLag, NoItemRelease and PotionSaver at the serverbound send
// (`EventPacketSend`), and Regen's extra packets. Held packets wait here in
// send order with their due time; releasing goes straight to the session so
// it does not pass the hook again. Remote servers only: the integrated
// server's packets never reach `send`.
use super::{Game, S, Screen};
use crate::game::vape_rs::core::{try_with_client, with_client};
use crate::game::vape_rs::modules::blatant::outbound::{Context, Packet, Verdict, Vitals};
use std::cell::RefCell;
use std::collections::VecDeque;

thread_local! {
    static HELD: RefCell<VecDeque<(u64, S)>> = const { RefCell::new(VecDeque::new()) };
}

fn now_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
}

fn classify(game: &Game<'_>, packet: &S) -> Packet {
    match packet {
        S::MovePlayerPos(_) | S::MovePlayerPosRot(_) | S::MovePlayerRot(_) | S::MovePlayerStatus(_) => Packet::Movement,
        S::Attack(a) => Packet::Attack {
            hurt_time: game.mobs.iter().find(|m| m.id as i32 == a.id && !m.removed).map(|m| m.hurt_time),
        },
        S::PlayerAction(a) if a.action == 5 => Packet::ReleaseUse,
        _ => Packet::Other,
    }
}

/// Sends held packets that are due (all of them with `all`).
fn release(game: &Game<'_>, all: bool) {
    let Some(r) = &game.remote else {
        HELD.with(|h| h.borrow_mut().clear());
        return;
    };
    let now = now_ms();
    loop {
        let next = HELD.with(|h| {
            let mut h = h.borrow_mut();
            match h.front() {
                Some((due, _)) if all || *due <= now => h.pop_front().map(|(_, p)| p),
                _ => None,
            }
        });
        let Some(packet) = next else { break };
        r.session.send(packet);
    }
}

/// `send` HEAD: Break when the packet is dropped or held.
pub fn send(game: &Game<'_>, packet: &S) -> core::ops::ControlFlow<()> {
    if game.remote.is_none() {
        return core::ops::ControlFlow::Continue(());
    }
    let Some(player) = game.player.as_ref() else { return core::ops::ControlFlow::Continue(()); };
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let ctx = Context {
        now_ms: now_ms(),
        in_gui: game.screen != Screen::Playing,
        held: crate::game::vape_combat::held(game),
        effects: !game.potions.effects.list.is_empty(),
        moving: player.move_vector != [0.0, 0.0],
        on_ground: player.ground.on_ground,
        position: [player.position.x, player.position.y, player.position.z],
        yaw: game.yaw,
        queued: HELD.with(|h| h.borrow().len()),
        jitter: now_ms().wrapping_mul(0x9e37_79b9) >> 7,
    };
    let kind = classify(game, packet);
    let Some((verdict, flush)) = try_with_client(&client, |runtime| {
        let verdict = runtime.outbound_packet(kind, &ctx);
        (verdict, runtime.outbound_flush())
    }) else {
        return core::ops::ControlFlow::Continue(());
    };
    let flow = match verdict {
        Verdict::Send => core::ops::ControlFlow::Continue(()),
        Verdict::Drop => core::ops::ControlFlow::Break(()),
        Verdict::Hold { due } => {
            HELD.with(|h| h.borrow_mut().push_back((due, packet.clone())));
            core::ops::ControlFlow::Break(())
        }
    };
    release(game, flush);
    flow
}

/// Client tick: due and flushed packets go, and Regen sends its packets.
pub fn tick(game: &Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let flush = with_client(&client, |runtime| runtime.outbound_tick());
    release(game, flush || game.player.is_none());
    let Some(player) = game.player.as_ref() else { return; };
    if game.remote.is_none() {
        return;
    }
    let vitals = Vitals {
        health: player.health.health(),
        food: player.health.food.food,
        dead: player.health.health() <= 0.0,
        on_ground: player.ground.on_ground,
        jumping: player.jumping,
        in_liquid: player.fluids.was_touching_water,
        riding: game.riding.is_some(),
        age: player.health.tick_count.max(0) as u64,
    };
    let count = with_client(&client, |runtime| runtime.regen_packets(vitals));
    for _ in 0..count {
        game.send(S::MovePlayerStatus(horizon_protocol::packets::play::MovePlayerStatus { flags: 0 }));
    }
}
