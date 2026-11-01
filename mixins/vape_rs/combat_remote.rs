// Runs inside game::remote: attacks by entity id for the auras, KeepSprint
// at the attacker's extra-knockback cut, and the managed rotation in the
// motion packet.
use super::super::Game;
use super::super::lan_pvp::Victim;
use crate::game::vape_rs::core::{ModuleId, with_client};
use crate::game::vape_rs::modules::combat::keep_sprint::Hit;
use horizon_protocol::packets::{Empty, play::{self, Serverbound as S}};
use std::cell::Cell;

thread_local! {
    static ROTATION: Cell<Option<(f32, f32)>> = const { Cell::new(None) };
    static KEEP_SPRINT_INSIDE: Cell<bool> = const { Cell::new(false) };
}

/// The managed rotation this tick's motion packet carries, if any.
pub fn set_rotation(rotation: Option<(f32, f32)>) {
    ROTATION.with(|r| r.set(rotation));
}

/// `send-state-rotation` (MODIFY_ARG): the rotation argument of
/// `position_sender.update`.
pub fn rotation(value: (f32, f32)) -> (f32, f32) {
    ROTATION.with(Cell::get).unwrap_or(value)
}

/// Attack the mob with this entity id the way the host's own attack does
/// after its pick: `MultiPlayerGameMode.attack` + swing on a server, or the
/// integrated attack locally; a LAN guest with this id takes the host's
/// player attack. False when there is no such living mob or guest.
pub fn attack(game: &mut Game<'_>, id: i32) -> bool {
    let Some(index) = game.mobs.iter().position(|m| m.alive() && m.id as i32 == id) else {
        return attack_guest(game, id);
    };
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let player = game.remote_player_names().iter().any(|(mob, _)| *mob as i32 == id);
    let hurt_time = game.mobs[index].hurt_time;
    let wtap = with_client(&client, |runtime| {
        runtime.wtap_attack(player, hurt_time);
        runtime.enabled(ModuleId::WTap)
    });
    if game.remote.is_some() {
        if game.clicks.miss_time > 0 {
            return false;
        }
        game.send(S::Attack(play::Attack { id }));
        game.remote_player_attack(index);
        game.reset_attack_strength();
        game.swing_arm();
        game.send(S::Punch(Empty));
    } else {
        game.attack_mob(index);
        game.swing_arm();
    }
    if wtap {
        crate::game::vape_input_delays::movement_again(game);
    }
    true
}

/// The LAN host's `attack_guest` on the guest with this entity id rather
/// than the one under the crosshair: the strike (with its swing), its
/// landing on the guest, the weapon's effects, the exhaustion and the
/// sprint stop.
fn attack_guest(game: &mut Game<'_>, id: i32) -> bool {
    let Some(conn) = game.lan.as_ref().and_then(|host| {
        host.hub
            .guests
            .iter()
            .find(|g| g.entity_id == id && host.hub.here(g) && g.loaded && !g.dead && !g.spectator())
            .map(|g| g.conn)
    }) else {
        return false;
    };
    let Some(mut host) = game.lan.take() else {
        return false;
    };
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let wtap = with_client(&client, |runtime| {
        runtime.wtap_attack(true, 0);
        runtime.enabled(ModuleId::WTap)
    });
    if let Some(strike) = game.pvp_strike()
        && game.pvp_land(&mut host, Victim::Guest(conn), &strike)
    {
        game.weapon_hit_player(Game::pvp_victim_on_ground(&host, Victim::Guest(conn), None));
        game.pvp_exhaustion();
        if strike.stops_sprint() {
            game.stop_sprint_after_extra_knockback();
        }
    }
    game.lan = Some(host);
    if wtap {
        crate::game::vape_input_delays::movement_again(game);
    }
    true
}

/// A click whose ray hits nothing: the host's miss (`missTime`, the attack
/// strength reset and the swing).
pub fn miss(game: &mut Game<'_>) {
    if game.remote.is_some() {
        if game.clicks.miss_time > 0 {
            return;
        }
        if !game.creative {
            game.clicks.miss_time = 10;
        }
        game.reset_attack_strength();
        game.swing_arm();
        game.send(S::Punch(Empty));
    } else {
        game.swing_arm();
    }
}

/// `remote_player_attack` HEAD (cancellable): KeepSprint replaces the
/// attacker's 0.6 cut and sprint stop after the host applied them.
pub fn keep_sprint(game: &mut Game<'_>, mob: usize) -> core::ops::ControlFlow<()> {
    use core::ops::ControlFlow;
    if KEEP_SPRINT_INSIDE.with(Cell::get) || game.serving.is_some() || game.options.dedicated.is_some() {
        return ControlFlow::Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if !with_client(&client, |runtime| runtime.enabled(ModuleId::KeepSprint)) {
        return ControlFlow::Continue(());
    }
    let Some(p) = game.player.as_ref() else { return ControlFlow::Continue(()); };
    let before = [p.velocity.x, p.velocity.z];
    KEEP_SPRINT_INSIDE.with(|c| c.set(true));
    game.remote_player_attack(mob);
    KEEP_SPRINT_INSIDE.with(|c| c.set(false));
    let Some(p) = game.player.as_ref() else { return ControlFlow::Break(()); };
    let hit = Hit {
        before,
        after: [p.velocity.x, p.velocity.z],
        on_ground: p.ground.on_ground,
        health: p.health.health(),
        hurt_time: p.health.hurt_animation_time,
    };
    if let Some(keep) = with_client(&client, |runtime| runtime.keep_sprint_hit(hit))
        && let Some(p) = game.player.as_mut()
    {
        p.velocity.x = keep.motion[0];
        p.velocity.z = keep.motion[1];
        if keep.sprinting {
            let _ = p.set_sprinting(true);
        }
    }
    ControlFlow::Break(())
}

/// A swing without an attack (`LocalPlayer.swing`): the arm and, on a
/// server, the swing packet.
pub fn swing(game: &mut Game<'_>) {
    game.swing_arm();
    if game.remote.is_some() {
        game.send(S::Punch(Empty));
    }
}
