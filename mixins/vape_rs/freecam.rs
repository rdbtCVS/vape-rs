// Host side of Freecam (reference `FreecamModernController`). It sits under
// game.rs beside the view and the physical keys: the tick feeds the module
// the player and the movement keys and holds the player's rotation, the
// camera hook renders from the module's camera instead of the player, the
// turn hook sends the mouse to the camera, and the body hook draws the
// player where it stands (Spawn Fake). The movement keys are released
// through the movement writer (`input_delays.rs`).
use crate::game::{Game, Screen};
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::modules::render::freecam;
use winit::keyboard::KeyCode;

/// Keys physically held, through the key mapping
/// (`ClientSettings.isPhysicalKeyDown`, whatever the key set says).
fn physical(game: &Game<'_>, key: KeyCode) -> bool {
    game.physical_keys
        .iter()
        .any(|&held| game.settings.keys.translate_all(held).into_iter().any(|m| m == Ok(key)))
}

fn host(game: &Game<'_>) -> bool {
    game.serving.is_none() && game.options.dedicated.is_none()
}

/// Client tick HEAD (`EventPreTick`), before the movement writer.
pub(crate) fn tick(game: &mut Game<'_>) {
    if !host(game) {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (wanted, gui_open) = with_client(&client, |runtime| (runtime.freecam_wanted(), runtime.gui.open));
    if !wanted {
        return;
    }
    let player = game.player.as_ref();
    let input = freecam::Input {
        player: player.is_some(),
        health: player.map_or(0.0, |p| p.health.health()),
        eye: player.map_or([0.0; 3], |p| {
            [p.position.x, p.position.y + f64::from(p.dimensions.eye_height), p.position.z]
        }),
        view: (game.yaw, game.pitch),
        no_screen: game.screen == Screen::Playing && !gui_open,
        keys: freecam::Keys {
            forward: physical(game, KeyCode::KeyW),
            back: physical(game, KeyCode::KeyS),
            left: physical(game, KeyCode::KeyA),
            right: physical(game, KeyCode::KeyD),
            jump: physical(game, KeyCode::Space),
            sneak: physical(game, KeyCode::ShiftLeft),
            sprint: physical(game, KeyCode::ControlLeft),
        },
        // `KeyboardInput.isKeyDown(37..=40)`: the raw arrow keys.
        arrows: freecam::Arrows {
            up: game.physical_keys.contains(&KeyCode::ArrowUp),
            down: game.physical_keys.contains(&KeyCode::ArrowDown),
            left: game.physical_keys.contains(&KeyCode::ArrowLeft),
            right: game.physical_keys.contains(&KeyCode::ArrowRight),
        },
    };
    let out = with_client(&client, |runtime| runtime.freecam_tick(&input));
    // `restorePlayerRotation` (the body) while the camera is free,
    // `restorePlayerView` (the saved rotation) on the disable.
    if let Some((yaw, pitch)) = out.rotation
        && (game.yaw, game.pitch) != (yaw, pitch)
    {
        game.yaw = yaw;
        game.pitch = pitch;
        game.snap_player_model_pitch();
    }
    // `applyMovementInput` writes the player's input itself: InvWalk's keys
    // behind a screen do not reach it.
    if out.body_keys.is_some()
        && let Some(player) = game.player.as_ref()
    {
        player.vape_rs_set_inv_walk(None);
    }
}

/// `applyCameraToPlayer` and `updateRenderViewPosition`: the camera entity
/// turned to the module's camera, its eye exactly at the camera's
/// interpolated position (the view adds its own eye height back) and with no
/// third-person distance, as the reference sets the render position after
/// `Camera.setup`. Third person in front still mirrors the rotation.
pub(crate) fn camera_entity(game: &Game<'_>) -> Option<horizon_client::camera::CameraEntity> {
    if !host(game) {
        return None;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let camera = with_client(&client, |runtime| runtime.freecam_camera())?;
    let p = game.player.as_ref()?;
    let (eye, eye_old) = (f64::from(game.view.eye_height), f64::from(game.view.eye_height_old));
    Some(horizon_client::camera::CameraEntity {
        position: [camera.position[0], camera.position[1] - eye, camera.position[2]],
        previous: [camera.previous[0], camera.previous[1] - eye_old, camera.previous[2]],
        x_rot: camera.pitch,
        y_rot: camera.yaw,
        x_rot_o: camera.pitch,
        y_rot_o: camera.yaw,
        eye_height: p.dimensions.eye_height,
        living: Some(horizon_client::camera::LivingCamera {
            scale: 1.0,
            camera_distance: 0.0,
            sleeping: None,
            dead: None,
        }),
        mount: None,
    })
}

/// `updateMouseLook`: a view turn goes to the camera while it is free.
pub(crate) fn turn(game: &Game<'_>, xo: f64, yo: f64) -> bool {
    if !host(game) {
        return false;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    with_client(&client, |runtime| runtime.freecam_turn(xo, yo))
}

/// Spawn Fake: the player's body shows where it stands in first person.
pub(crate) fn body(game: &Game<'_>) -> bool {
    if !host(game) {
        return false;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    with_client(&client, |runtime| runtime.freecam_body())
}

/// `suppressViewBobbing`: no view bob while the arrow keys walk the player.
pub(crate) fn no_bob(game: &Game<'_>) -> bool {
    if !host(game) {
        return false;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    with_client(&client, |runtime| runtime.freecam_no_bob())
}

/// The movement writer's requests (the seven movement keys released, the
/// arrows' directions with Move Fake).
pub(crate) fn requests(game: &Game<'_>) -> Vec<crate::game::vape_rs::core::movement_input::Request> {
    if !host(game) {
        return Vec::new();
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    with_client(&client, |runtime| runtime.freecam_requests())
}
