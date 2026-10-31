// Host side of the movement/world modules (the shared rotation manager,
// Scaffold Legit, InvWalk, WindCharge, NoFall's jump release). It sits under game/remote.rs so the
// remote use path, the open menu id and the physical keys are reachable.
use crate::game::Game;
use crate::game::vape_rs::core::movement_input::{Axis, HostView, Owner, Request};
use crate::game::vape_rs::core::{ModuleId, with_client};
use crate::game::vape_rs::modules::movement::{inv_walk, scaffold, wind_charge};
use winit::keyboard::KeyCode;

thread_local! {
    /// WindCharge's jump for the writer this tick.
    static JUMP: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Keys physically held, through the key mapping (`KeyMapping.isDown` of
/// the physical state, whatever screen is open).
fn physical(game: &Game<'_>, key: KeyCode) -> bool {
    game.physical_keys
        .iter()
        .any(|&held| game.settings.keys.translate_all(held).into_iter().any(|m| m == Ok(key)))
}

/// Scaffold Legit's edge test: the box shrunk by 0.2 horizontally, moved by
/// this tick's horizontal motion and one block down, touches nothing.
fn scaffold_edge(game: &mut Game<'_>) -> bool {
    let entities = game.player_entity_collisions();
    let Some(player) = game.player.as_mut() else { return false; };
    let (vx, vz) = (player.velocity.x, player.velocity.z);
    let bounds = player
        .bounds()
        .deflate(horizon_core::geometry::Vec3::new(0.2, 0.0, 0.2))
        .move_by(horizon_core::geometry::Vec3::new(vx, -1.0, vz));
    let mut motion = horizon_client::motion::MotionWorld { entities: &entities, player, scene: &mut game.world, assets: &game.pack.assets, collision_binding: game.collision, resources: game.motion };
    motion.no_player_collision(bounds).unwrap_or(false)
}

/// `isOnLadder`: the player is in a climbable block (`LivingEntity.onClimbable`).
fn on_ladder(game: &mut Game<'_>) -> bool {
    let entities = game.player_entity_collisions();
    let Some(player) = game.player.as_mut() else { return false; };
    let mut motion = horizon_client::motion::MotionWorld { entities: &entities, player, scene: &mut game.world, assets: &game.pack.assets, collision_binding: game.collision, resources: game.motion };
    motion.climbable().unwrap_or(false)
}

/// The movement writer's requests from these modules.
pub(crate) fn requests(game: &mut Game<'_>, active: bool, again: bool, view: &HostView) -> Vec<Request> {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let mut out = Vec::new();
    // NoFall Normal released jump on a claimed landing; it stays released
    // until the user lets go of jump.
    if with_client(&client, |runtime| {
        let user = runtime.movement().user(Axis::Jump, view);
        runtime.no_fall_jump_released(user)
    }) {
        out.push(Request { axis: Axis::Jump, owner: Owner::NoFall, priority: 3, pressed: false });
    }
    if !again && JUMP.with(|j| j.take()) {
        out.push(Request { axis: Axis::Jump, owner: Owner::WindCharge, priority: 4, pressed: true });
    }
    let scaffold = active && with_client(&client, |runtime| runtime.enabled(ModuleId::Scaffold));
    if scaffold && !again && game.player.is_some() {
        let edge = scaffold_edge(game);
        let on_ladder = on_ladder(game);
        let held = crate::game::vape_combat::held(game);
        let player = game.player.as_ref().unwrap();
        let on_ground = player.ground.on_ground;
        let sneaking = player.shift_key_down || player.pose == horizon_client::pose::Pose::Crouching;
        let gui_open = with_client(&client, |runtime| runtime.gui.open);
        let sneak = with_client(&client, |runtime| {
            let movement = runtime.movement();
            let forward = f32::from(u8::from(movement.user(Axis::Forward, view))) - f32::from(u8::from(movement.user(Axis::Back, view)));
            let sneak_held = movement.user(Axis::Sneak, view);
            let tick = scaffold::Tick {
                now_ms: runtime.now_ms(),
                no_screen: game.screen == crate::game::Screen::Playing && !gui_open,
                sneak_held,
                sneaking,
                pitch: game.pitch,
                forward,
                on_ground,
                on_ladder,
                edge,
                held,
            };
            runtime.scaffold_sneak(&tick)
        });
        if let Some(sneak) = sneak {
            out.push(Request { axis: Axis::Sneak, owner: Owner::Scaffold, priority: 3, pressed: sneak == scaffold::Sneak::Press });
        }
    }
    out
}

fn inv_walk_screen(game: &Game<'_>, gui_open: bool, capturing: bool) -> inv_walk::Screen {
    use crate::game::Screen;
    if capturing {
        return inv_walk::Screen::TextField;
    }
    match game.screen {
        Screen::Playing if gui_open => inv_walk::Screen::Other,
        Screen::Playing => inv_walk::Screen::None,
        Screen::Chat => inv_walk::Screen::Chat,
        Screen::Inventory if game.container.is_none() && game.remote_menu_id() == 0 => inv_walk::Screen::Inventory,
        Screen::Paused => inv_walk::Screen::Pause,
        _ => inv_walk::Screen::Other,
    }
}

/// Client tick HEAD, before the movement writer.
pub(crate) fn tick(game: &mut Game<'_>) {
    if game.player.is_none() {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (gui_open, capturing) = with_client(&client, |runtime| (runtime.gui.open, runtime.gui.capturing()));
    // The shared rotation manager (`RotationManager.onTick`): a visible
    // controller turns the view; it waits while a screen is open.
    let no_screen = matches!(game.screen, crate::game::Screen::Playing) && !gui_open;
    let sensitivity = f64::from(game.settings.sensitivity);
    let view = (game.yaw, game.pitch);
    if let Some([yaw, pitch]) = with_client(&client, |runtime| runtime.rotation_tick(view, sensitivity, no_screen)) {
        game.yaw = yaw;
        game.pitch = pitch;
    }
    // InvWalk.
    let screen = inv_walk_screen(game, gui_open, capturing);
    let options = with_client(&client, |runtime| runtime.inv_walk(screen));
    let keys = options.map(|o| {
        let held = inv_walk::Held {
            forward: physical(game, KeyCode::KeyW),
            back: physical(game, KeyCode::KeyS),
            left: physical(game, KeyCode::KeyA),
            right: physical(game, KeyCode::KeyD),
            jump: physical(game, KeyCode::Space),
            sneak: o.sneak && physical(game, KeyCode::ShiftLeft),
            up: game.physical_keys.contains(&KeyCode::ArrowUp),
            down: game.physical_keys.contains(&KeyCode::ArrowDown),
            turn_left: game.physical_keys.contains(&KeyCode::ArrowLeft),
            turn_right: game.physical_keys.contains(&KeyCode::ArrowRight),
        };
        let (yaw, pitch) = inv_walk::rotate(&o, &held, game.yaw, game.pitch);
        game.yaw = yaw;
        game.pitch = pitch;
        horizon_client::sprint::Keys {
            forward: held.forward,
            backward: held.back,
            left: held.left,
            right: held.right,
            jump: held.jump,
            shift: held.sneak,
            sprint: false,
            analog: None,
        }
    });
    if let Some(player) = game.player.as_ref() {
        player.vape_rs_set_inv_walk(keys);
    }
    wind_charge_tick(game, &client);
}

fn wind_charge_tick(game: &mut Game<'_>, client: &crate::game::vape_rs::core::Client) {
    let charge_slot = game
        .inventory
        .iter()
        .take(9)
        .position(|s| s.as_ref().is_some_and(|s| &*s.item == "minecraft:wind_charge"));
    let Some(player) = game.player.as_ref() else { return; };
    let tick = wind_charge::Tick {
        charge_slot,
        selected: game.selected,
        on_ground: player.ground.on_ground,
        view: [game.yaw, game.pitch],
    };
    let alive = player.health.health() > 0.0;
    let actions = with_client(client, |runtime| runtime.wind_charge_tick(&tick));
    if let Some(slot) = actions.select.filter(|s| *s < 9) {
        if alive && game.title.is_none() {
            game.selected = slot;
        }
    }
    if actions.throw && game.remote.is_some() {
        // The throw (UseItem carries the rotation) and its crosshair use the
        // managed rotation; with silent aim the camera stays.
        let view = (game.yaw, game.pitch);
        if let Some([yaw, pitch]) = actions.rotation {
            game.yaw = yaw;
            game.pitch = pitch;
        }
        game.remote_use();
        if actions.silent {
            (game.yaw, game.pitch) = view;
        }
    }
    if actions.jump {
        JUMP.with(|j| j.set(true));
    }
}
