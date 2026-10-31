// HitSelect's host side: the views of the local player and its crosshair
// target, the cancellable attack, use and block-hit clicks
// (`EventClickMouse`, `EventRightClickMouse`/`EventPlayerUseItem`,
// `EventSendClickBlockToController`) and `ClientSettings.applyAttackEffects`.
use super::Game;
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::modules::combat::hit_select::{Target, View};
use core::ops::ControlFlow::{self, Break, Continue};

fn local(game: &Game<'_>) -> bool {
    game.serving.is_none() && game.options.dedicated.is_none()
}

/// The living entity of the extended-reach ray trace (`objectMouseOver`).
fn crosshair(game: &Game<'_>) -> Option<usize> {
    game.mob_hit()
        .filter(|(_, d)| game.before_block(*d))
        .map(|(i, _)| i)
}

pub(crate) fn view(game: &Game<'_>) -> View {
    let Some(p) = game.player.as_ref() else {
        return View::default();
    };
    View {
        player: true,
        on_ground: p.ground.on_ground,
        position: [p.position.x, p.position.y, p.position.z],
        motion: [p.velocity.x, p.velocity.y, p.velocity.z],
        target: crosshair(game).map(|i| {
            let m = &game.mobs[i];
            Target {
                position: [m.position.x, m.position.y, m.position.z],
                hurt_time: m.hurt_time,
            }
        }),
    }
}

/// `EntityLivingBase.isOnLadder`: a climbable block, or an open trapdoor
/// over a ladder facing the same way, at the feet.
fn on_ladder(game: &Game<'_>) -> bool {
    let Some(p) = game.player.as_ref() else {
        return false;
    };
    let at = [p.position.x, p.position.y, p.position.z].map(|v| v.floor() as i32);
    let state = |y: i32| game.world.block(at[0], y, at[2]);
    let name = |s| game.res.facts().get(s).name.clone();
    let property = |s, key: &str| {
        game.pack
            .assets
            .registry
            .state(s)
            .and_then(|s| s.properties.get(key).cloned())
    };
    let here = state(at[1]);
    let here_name = name(here);
    if game.block_in_tag("minecraft:climbable", &here_name) {
        return true;
    }
    let below = state(at[1] - 1);
    here_name.ends_with("_trapdoor")
        && property(here, "open").as_deref() == Some("true")
        && &*name(below) == "minecraft:ladder"
        && property(here, "facing") == property(below, "facing")
}

/// `ClientSettings.applyAttackEffects`: the local swing, the crit particles
/// of a falling hit and the enchanted-hit particles, without an attack.
fn attack_effects(game: &mut Game<'_>, mob: usize) {
    use horizon_client::particles::{ParticleOptions, ParticleType};
    game.swing_arm();
    let crit = game.player.as_ref().is_some_and(|p| {
        p.fall_distance > 0.0 && !p.ground.on_ground && !p.fluids.was_touching_water
    }) && !on_ladder(game)
        && game.potions.effects.get("blindness").is_none()
        && game.riding.is_none();
    let kind = format!(
        "minecraft:{}",
        game.mobs[mob].kind.id.trim_start_matches("minecraft:")
    );
    let magic = game.enchant_damage_bonus(&kind) > 0.0;
    let area = game.mobs[mob].bounds();
    let (feet, size) = (area.bottom_center(), area.size());
    for (on, particle) in [(crit, ParticleType::Crit), (magic, ParticleType::EnchantedHit)] {
        if on {
            // `EffectRenderer.emitParticleAtEntity`: local only.
            game.particles.tracking_emitter(
                ParticleOptions::simple(particle),
                [feet.x, feet.y, feet.z],
                size.x,
                size.y,
            );
        }
    }
}

/// `EventPreTick`: before the clickers and the auras.
pub(crate) fn tick(game: &Game<'_>) {
    if !local(game) {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let view = view(game);
    with_client(&client, |rt| rt.hit_select_tick(&view));
}

/// LeftClicker's `isHitSelectActive` with the crosshair entity still
/// invulnerable and the player on the ground.
pub(crate) fn defers(game: &Game<'_>) -> bool {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let view = view(game);
    view.on_ground
        && view.target.is_some_and(|t| t.hurt_time > 2)
        && with_client(&client, |rt| rt.hit_select_defers())
}

/// `remote_start_attack` HEAD: a cancelled click does nothing (false: no
/// instant block break).
pub(crate) fn attack(game: &mut Game<'_>) -> ControlFlow<bool> {
    if !local(game) {
        return Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let view = view(game);
    let click = with_client(&client, |rt| rt.hit_select_click(&view));
    if click.effects
        && let Some(mob) = crosshair(game)
    {
        attack_effects(game, mob);
    }
    if click.cancel { Break(false) } else { Continue(()) }
}

/// `remote_use` and `remote_continue_attack` HEAD: mode Normal holds back
/// use, right clicks and block hitting until the next tick.
pub(crate) fn hold_use(game: &Game<'_>) -> ControlFlow<()> {
    if !local(game) {
        return Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if with_client(&client, |rt| rt.hit_select_cancel_use()) { Break(()) } else { Continue(()) }
}
