fn no_hurt_camera(
    game: &crate::game::Game<'_>,
    partial: f32,
) -> core::ops::ControlFlow<glam::Mat4> {
    use crate::game::vape_rs::core::{ModuleId, with_client};
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return core::ops::ControlFlow::Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let no_hurt = with_client(&client, |runtime| runtime.enabled(ModuleId::NoHurtCamera));
    // Freecam's `suppressViewBobbing`: the view bob is off for the frame.
    let no_bob = crate::game::vape_freecam::no_bob(game);
    if !no_hurt && !no_bob {
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
    let tilt = game.settings.file.double("damageTiltStrength");
    let bob_view = game.settings.view_bobbing && !no_bob;
    if !no_hurt {
        return core::ops::ControlFlow::Break(glam::Mat4::from_cols_array(
            &horizon_client::camera::bob_matrix(&state, tilt, bob_view).m,
        ));
    }
    core::ops::ControlFlow::Break(crate::game::vape_camera::without_hurt(state, tilt, bob_view))
}
