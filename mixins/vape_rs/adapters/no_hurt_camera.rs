fn no_hurt_camera(
    game: &crate::game::Game<'_>,
    partial: f32,
) -> core::ops::ControlFlow<glam::Mat4> {
    use crate::game::vape_rs::core::{ModuleId, with_client};
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return core::ops::ControlFlow::Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if !with_client(&client, |runtime| runtime.enabled(ModuleId::NoHurtCamera)) {
        return core::ops::ControlFlow::Continue(());
    }
    let Some(player) = game.player.as_ref() else {
        return core::ops::ControlFlow::Continue(());
    };
    let hurt = &player.health.hurt;
    let state = horizon_client::camera::BobState {
        living: Some(horizon_client::camera::HurtBob {
            hurt_time: player.health.hurt_animation_time as f32 - partial,
            hurt_duration: hurt.hurt_duration,
            hurt_dir: player.health.hurt_dir,
            death_time: game.death_time().map(|time| time as f32 + partial),
        }),
        player: Some((
            game.avatar.backwards_interpolated_walk_distance(partial),
            game.avatar.interpolated_bob(partial),
        )),
    };
    core::ops::ControlFlow::Break(crate::game::vape_camera::without_hurt(
        state,
        game.settings.file.double("damageTiltStrength"),
        game.settings.view_bobbing,
    ))
}
