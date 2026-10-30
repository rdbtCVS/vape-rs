// NoFall and Fly at the local player's motion packet (`EventPreMotion`). The
// send-state HEAD decides the on-ground flag; the argument modifier on the
// position sender's update applies it to this packet only.
use crate::game::Game;
use crate::game::vape_rs::core::{ModuleId, with_client};
use horizon_protocol::packets::play::{self, Serverbound as S};

thread_local! {
    static GROUND: core::cell::Cell<Option<bool>> = const { core::cell::Cell::new(None) };
}

/// `remote_send_state` HEAD.
pub fn pre_motion(game: &mut Game<'_>) {
    GROUND.with(|g| g.set(None));
    let Some(player) = game.player.as_ref() else { return; };
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let dead = player.health.health() <= 0.0;
    let skip = dead || game.creative || player.flying;
    let (fall, motion_y, collision) = (player.fall_distance as f32, player.velocity.y, player.collision.horizontal);
    let (fly, effects) = with_client(&client, |runtime| {
        (runtime.enabled(ModuleId::Fly) && runtime.session().in_world(), runtime.no_fall_motion(skip, fall, motion_y))
    });
    if effects.status_pair {
        for on_ground in [true, false] {
            game.send(S::MovePlayerStatus(play::MovePlayerStatus { flags: u8::from(on_ground) | u8::from(collision) << 1 }));
        }
    }
    let ground = if fly { Some(false) } else { effects.claim_ground.then_some(true) };
    GROUND.with(|g| g.set(ground));
}

/// The position sender's on-ground argument.
pub fn on_ground(value: bool) -> bool {
    GROUND.with(|g| g.take()).unwrap_or(value)
}
