// Blink, FakeLag, NoItemRelease and PotionSaver at the serverbound send
// (`EventPacketSend`), and Regen's extra packets. Held packets wait here in
// send order with their due time; releasing goes straight to the session so
// it does not pass the hook again. Remote servers only: the integrated
// server's packets never reach `send`.
//
// Inbound, three modules hold packets in one ordered queue, each entry with
// its own due time (never earlier than the one before it): KnockbackDelay
// holds the local player's velocity packet and every packet behind it
// (JumpReset only observes it), Backtrack holds a target's position updates
// while it moves away, and Blink's Bi-directional mode holds everything until
// Blink sends. Blink wins over the others; KnockbackDelay over Backtrack's
// flushes. Held packets replay in order through the host's `handle_packet`;
// `REPLAYING` makes the hook step aside for them. The queue is bounded, and connection-critical packets (keep-alive,
// ping, login/respawn, disconnect, configuration, cookies, custom payloads,
// resource packs, chunk batches, bundles, teleports) are never held: they
// flush everything held before them and pass. Disable, disconnect, world
// open/close, respawn and dimension change flush the queue.
use super::{C, Game, S, Screen};
use crate::game::vape_rs::core::{try_with_client, with_client};
use crate::game::vape_rs::modules::blatant::outbound::{Context, Packet, Verdict, Vitals};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

thread_local! {
    static HELD: RefCell<VecDeque<(u64, S)>> = const { RefCell::new(VecDeque::new()) };
    static INBOUND: RefCell<VecDeque<Held>> = const { RefCell::new(VecDeque::new()) };
    static REPLAYING: Cell<bool> = const { Cell::new(false) };
}

/// Most inbound packets held at once; past it everything is replayed.
const INBOUND_LIMIT: usize = 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Knockback,
    Backtrack,
    Blink,
}

struct Held {
    due: u64,
    owner: Owner,
    packet: C,
}

fn critical(packet: &C) -> bool {
    matches!(
        packet,
        C::BundleDelimiter(_)
            | C::Ping(_)
            | C::PongResponse(_)
            | C::KeepAlive(_)
            | C::Login(_)
            | C::Respawn(_)
            | C::Disconnect(_)
            | C::StartConfiguration(_)
            | C::Transfer(_)
            | C::CookieRequest(_)
            | C::StoreCookie(_)
            | C::CustomPayload(_)
            | C::ResourcePackPush(_)
            | C::ResourcePackPop(_)
            | C::ChunkBatchStart(_)
            | C::ChunkBatchFinished(_)
            | C::PlayerPosition(_)
    )
}

fn queued() -> usize {
    INBOUND.with(|q| q.borrow().len())
}

/// Replays held packets through the host, in order: all of them, or those
/// whose release time has passed. The guard keeps them out of the hook.
fn replay(game: &mut Game<'_>, all: bool) -> super::Result<()> {
    loop {
        let now = now_ms();
        let Some(packet) = INBOUND.with(|q| {
            let mut q = q.borrow_mut();
            match q.front() {
                Some(held) if all || held.due <= now => q.pop_front().map(|h| h.packet),
                _ => None,
            }
        }) else {
            break;
        };
        if game.remote.is_none() {
            // The connection is gone: nothing to replay into.
            INBOUND.with(|q| q.borrow_mut().clear());
            break;
        }
        let before = REPLAYING.with(|r| r.replace(true));
        let result = game.handle_packet(packet);
        REPLAYING.with(|r| r.set(before));
        result?;
    }
    Ok(())
}

/// Replays everything held (disable, disconnect, world or dimension change).
pub fn flush_inbound(game: &mut Game<'_>) {
    if queued() == 0 {
        return;
    }
    if let Err(error) = replay(game, true) {
        eprintln!("[vape-rs] replaying held packets failed: {error}");
    }
}

/// What Backtrack makes of a clientbound packet.
fn tracked(game: &Game<'_>, packet: &C) -> crate::game::vape_rs::modules::blatant::backtrack::Packet {
    use crate::game::vape_rs::modules::blatant::backtrack::Packet as T;
    use horizon_protocol::packets::play::{self, VecDelta};
    let host = |id: i32| game.remote.as_ref().and_then(|r| r.entities.get(&id)).map(|t| t.position);
    match packet {
        C::MoveEntityPos(play::MoveEntityPos(m)) | C::MoveEntityPosRot(play::MoveEntityPosRot(m)) | C::MoveEntityRot(play::MoveEntityRot(m)) => T::Moved {
            id: m.id,
            host: host(m.id),
            steps: match &m.delta {
                Some(VecDelta::Linear(d)) => vec![*d],
                Some(VecDelta::Stepped(steps)) => steps.iter().map(|(d, _)| *d).collect(),
                None => Vec::new(),
            },
        },
        C::EntityPositionSync(p) => T::Synced { id: p.id, at: p.path.end() },
        C::TeleportEntity(p) => {
            let base = host(p.id).unwrap_or([0.0; 3]);
            let rel = |bit: i32, i: usize| if p.relatives & bit != 0 { base[i] + p.position[i] } else { p.position[i] };
            T::Teleported { id: p.id, at: [rel(play::relative::X, 0), rel(play::relative::Y, 1), rel(play::relative::Z, 2)] }
        }
        C::RemoveEntities(p) => T::Removed(p.ids.clone()),
        C::SetEntityMotion(p) if game.remote.as_ref().is_some_and(|r| r.entity_id == p.id) => T::OwnVelocity,
        _ => T::Other,
    }
}

/// `handle_packet` HEAD: Break drops or holds the packet (the host sees it
/// later through `replay`), Continue hands it to the host now.
pub fn inbound(game: &mut Game<'_>, packet: &C) -> core::ops::ControlFlow<super::Result<()>> {
    use core::ops::ControlFlow::{Break, Continue};
    use crate::game::vape_rs::modules::blatant::backtrack as bt;
    use crate::game::vape_rs::modules::combat::knockback::{Context, Verdict};
    if REPLAYING.with(Cell::get) {
        return Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let now = now_ms();
    // Repel's expected hit delay: damage packets are timed on arrival.
    match packet {
        C::EntityEvent(p) if p.event == 2 => with_client(&client, |runtime| runtime.outbound_hit(p.id, now)),
        C::HurtAnimation(p) => with_client(&client, |runtime| runtime.outbound_hit(p.id, now)),
        _ => {}
    }
    let (hold, blink, flush) = with_client(&client, |runtime| {
        runtime.backtrack_held(queued());
        (runtime.inbound_hold_active(), runtime.blink_holds_inbound(), runtime.take_inbound_flush())
    });
    if queued() > 0 {
        // Everything when nobody holds or a module let go, else what is due.
        if let Err(error) = replay(game, !hold || flush) {
            return Break(Err(error));
        }
    }
    let mut verdict = Verdict::Pass;
    if let C::SetEntityMotion(p) = packet
        && game.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
        && game.player.is_some()
    {
        let friends = with_client(&client, |runtime| runtime.friend_keys());
        let facing = crate::game::vape_combat::facing(game, &friends);
        let in_water = game.player.as_ref().is_some_and(|pl| pl.fluids.was_touching_water);
        let ctx = Context {
            now_ms: now,
            in_water,
            facing: facing.player_facing_target && facing.target_facing_player,
            target: facing.target_facing_player,
            queued: queued(),
        };
        verdict = with_client(&client, |runtime| runtime.knockback_packet(&ctx, p.velocity));
    }
    let mut tracked_verdict = bt::Verdict::Pass;
    if let Some(player) = game.player.as_ref() {
        let ctx = bt::Context { now_ms: now, player: [player.position.x, player.position.y, player.position.z] };
        let event = tracked(game, packet);
        tracked_verdict = with_client(&client, |runtime| runtime.backtrack_packet(&ctx, &event));
    }
    if critical(packet) {
        if queued() > 0
            && let Err(error) = replay(game, true)
        {
            return Break(Err(error));
        }
        return Continue(());
    }
    if !hold {
        return Continue(());
    }
    let tail = INBOUND.with(|q| q.borrow().back().map(|h| (h.due, h.owner)));
    let want = if blink {
        Some((u64::MAX, Owner::Blink))
    } else if let Verdict::Hold { release_at } = verdict {
        Some((release_at, Owner::Knockback))
    } else if let bt::Verdict::Hold { due } = tracked_verdict {
        Some((due, Owner::Backtrack))
    } else {
        None
    };
    let (due, owner) = match (want, tail) {
        (Some((due, owner)), tail) => (due.max(tail.map_or(0, |t| t.0)), owner),
        // Behind what is held, unless Backtrack's own flush frees it all.
        (None, Some((_, owner))) if tracked_verdict == bt::Verdict::Flush && owner == Owner::Backtrack => {
            return match replay(game, true) {
                Ok(()) => Continue(()),
                Err(error) => Break(Err(error)),
            };
        }
        (None, Some(tail)) => tail,
        (None, None) => return Continue(()),
    };
    if queued() >= INBOUND_LIMIT {
        return match replay(game, true) {
            Ok(()) => Continue(()),
            Err(error) => Break(Err(error)),
        };
    }
    INBOUND.with(|q| q.borrow_mut().push_back(Held { due, owner, packet: packet.clone() }));
    Break(Ok(()))
}

fn now_ms() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
}

fn classify(game: &Game<'_>, packet: &S) -> Packet {
    match packet {
        S::MovePlayerPos(_) | S::MovePlayerPosRot(_) | S::MovePlayerRot(_) | S::MovePlayerStatus(_) => Packet::Movement,
        S::Attack(a) => Packet::Attack {
            id: a.id,
            hurt_time: game.mobs.iter().find(|m| m.id as i32 == a.id && !m.removed).map(|m| m.hurt_time),
        },
        S::PlayerAction(a) if a.action == 6 => Packet::ReleaseUse, // RELEASE_USE_ITEM (5 is DROP_ITEM)
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
        if let S::Attack(a) = packet {
            runtime.backtrack_attack(a.id);
        }
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
pub fn tick(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (hold, flush) = with_client(&client, |runtime| {
        runtime.backtrack_tick();
        (runtime.inbound_hold_active(), runtime.take_inbound_flush())
    });
    if queued() > 0 {
        // Due packets go; all of them when nobody holds, a module let go, or
        // there is no player.
        if let Err(error) = replay(game, !hold || flush || game.player.is_none()) {
            eprintln!("[vape-rs] replaying held packets failed: {error}");
        }
    }
    if let Some(player) = game.player.as_ref() {
        let motion = [player.velocity.x, player.velocity.y, player.velocity.z];
        let on_ground = player.ground.on_ground;
        let jump_down = game.keys.contains(&winit::keyboard::KeyCode::Space);
        with_client(&client, |runtime| runtime.knockback_tick(on_ground, motion, jump_down));
    }
    let target_hurt = with_client(&client, |runtime| runtime.outbound_target())
        .and_then(|id| game.mobs.iter().find(|m| m.id as i32 == id && !m.removed))
        .map(|m| m.hurt_time);
    let held_out = HELD.with(|h| h.borrow().len());
    let flush = with_client(&client, |runtime| runtime.outbound_tick(target_hurt, held_out));
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
