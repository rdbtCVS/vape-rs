// Host side of the movement/world modules (the shared rotation manager,
// Scaffold and its bridge modes, InvWalk, WindCharge, NoFall's jump release). It sits under game/remote.rs so the
// remote use path, the open menu id and the physical keys are reachable.
use crate::game::Game;
use crate::game::vape_rs::core::movement_input::{Axis, HostView, Owner, Request};
use crate::game::vape_rs::core::movement_runtime::BridgeTick;
use crate::game::vape_rs::core::{ModuleId, with_client};
use crate::game::vape_rs::modules::movement::movement_task::{Body, World};
use crate::game::vape_rs::modules::movement::scaffold_bridge::{Hit, Inventory, Stack};
use crate::game::vape_rs::modules::movement::{inv_walk, scaffold, wind_charge};
use horizon_core::geometry::Vec3;
use winit::keyboard::KeyCode;

thread_local! {
    /// WindCharge's jump for the writer this tick.
    static JUMP: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    /// AutoTotem's `suppressInput` this tick.
    static TOTEM_HOLD: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
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

/// The bridge modes' world queries.
struct HostWorld<'g, 'a>(&'g mut Game<'a>);

impl World for HostWorld<'_, '_> {
    fn air(&mut self, pos: [i32; 3]) -> bool {
        let name = &self.0.res.facts().get(self.0.world.block(pos[0], pos[1], pos[2])).name;
        matches!(&**name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
    }
    fn clear_below(&mut self, at: [f64; 2], shrink: f64) -> bool {
        let game = &mut *self.0;
        let entities = game.player_entity_collisions();
        let Some(player) = game.player.as_mut() else { return false; };
        let (vx, vz) = (player.velocity.x, player.velocity.z);
        let bounds = player
            .bounds()
            .move_by(Vec3::new(at[0] - player.position.x, 0.0, at[1] - player.position.z))
            .deflate(Vec3::new(shrink, 0.0, shrink))
            .move_by(Vec3::new(vx, -1.0, vz));
        let mut motion = horizon_client::motion::MotionWorld { entities: &entities, player, scene: &mut game.world, assets: &game.pack.assets, collision_binding: game.collision, resources: game.motion };
        motion.no_player_collision(bounds).unwrap_or(false)
    }
}

/// The local player for the bridge modes and the movement task.
fn body(game: &mut Game<'_>) -> Option<Body> {
    let on_ladder = on_ladder(game);
    let previous = game.previous_position;
    let player = game.player.as_ref()?;
    Some(Body {
        pos: [player.position.x, player.position.y, player.position.z],
        prev: [previous.x, previous.y, previous.z],
        motion: [player.velocity.x, player.velocity.y, player.velocity.z],
        yaw: game.yaw,
        pitch: game.pitch,
        on_ground: player.ground.on_ground,
        sneaking: player.shift_key_down || player.pose == horizon_client::pose::Pose::Crouching,
        sprinting: player.sprinting,
        flying: player.flying,
        eye_height: f64::from(player.dimensions.eye_height),
        on_ladder,
    })
}

/// The inventory as the bridge modes read it.
fn inventory(game: &Game<'_>) -> Inventory {
    let stack = |s: Option<&horizon_client::items::ItemStack>, slot| {
        s.map(|s| Stack { item: crate::game::vape_combat::item(game, Some(s), slot), count: s.count })
    };
    Inventory {
        slots: (0..36).map(|slot| stack(game.inventory.get(slot).and_then(Option::as_ref), slot)).collect(),
        offhand: stack(game.offhand.as_ref(), 40),
        selected: game.selected,
        creative: game.creative,
    }
}

/// `normalReachRayTrace` (`rayTraceUsingManagedRotation(3.0, 0, false)`, the
/// 1.20.6+ mouse-over of RayTraceUtil), as `RotationManager.onTick` leaves
/// it once it has turned the view, before the player moves. The block along
/// the block reach the host's own pick uses (4.5, 5 in creative); the 3.0
/// only bounds entities.
/// An entity on the ray before the block leaves no block hit (an entity hit
/// within 3, a miss past it). LAN guests count within the host's guest pick.
fn crosshair_block(game: &Game<'_>) -> Option<Hit> {
    let (eye, dir) = game.aim()?;
    let reach = game.reach();
    let hit = game.clip_ray(eye, dir, reach)?;
    let distance = (0..3).map(|i| (hit.point[i] - eye[i]).powi(2)).sum::<f64>().sqrt();
    let entity = game
        .mobs
        .iter()
        .filter(|m| m.alive() && Some(m.id) != game.riding)
        .filter_map(|m| crate::game::ray_box(eye, dir, reach, m.bounds()))
        .chain(game.guest_hit().map(|(_, d)| d))
        .any(|d| d < distance);
    if entity {
        return None;
    }
    let side = match hit.face {
        [0, -1, 0] => 0,
        [0, 1, 0] => 1,
        [0, 0, -1] => 2,
        [0, 0, 1] => 3,
        [-1, 0, 0] => 4,
        _ => 5,
    };
    Some(Hit { pos: hit.pos, side })
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
    // AutoTotem's blockMovementKeys (EventPrePlayerTick, HIGHEST): forward,
    // back, left, right, jump and sprint released; sneak stays.
    if TOTEM_HOLD.with(|h| h.get()) {
        for axis in [Axis::Forward, Axis::Back, Axis::Left, Axis::Right, Axis::Jump, Axis::Sprint] {
            out.push(Request { axis, owner: Owner::AutoTotem, priority: 5, pressed: false });
        }
    }
    if !again && JUMP.with(|j| j.take()) {
        out.push(Request { axis: Axis::Jump, owner: Owner::WindCharge, priority: 4, pressed: true });
    }
    let scaffold = active && with_client(&client, |runtime| runtime.enabled(ModuleId::Scaffold));
    if scaffold && game.player.is_some() {
        let edge = scaffold_edge(game);
        let held = crate::game::vape_combat::held(game);
        let Some(body) = body(game) else { return out; };
        let gui_open = with_client(&client, |runtime| runtime.gui.open);
        let requests = with_client(&client, |runtime| {
            let movement = runtime.movement();
            let forward = f32::from(u8::from(movement.user(Axis::Forward, view))) - f32::from(u8::from(movement.user(Axis::Back, view)));
            let sneak_held = movement.user(Axis::Sneak, view);
            let tick = scaffold::Tick {
                now_ms: runtime.now_ms(),
                no_screen: game.screen == crate::game::Screen::Playing && !gui_open,
                sneak_held,
                sneaking: body.sneaking,
                pitch: game.pitch,
                forward,
                on_ground: body.on_ground,
                on_ladder: body.on_ladder,
                edge,
                held,
            };
            runtime.scaffold_requests(&tick, &body, again)
        });
        out.extend(requests);
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
    // AutoTotem's silent open holds the movement keys (and attack/use) from
    // the open until one "Silent move delay" after its close.
    let totem_hold = with_client(&client, |runtime| runtime.auto_totem_tick(std::time::Instant::now()));
    TOTEM_HOLD.with(|h| h.set(totem_hold));
    let (gui_open, capturing) = with_client(&client, |runtime| (runtime.gui.open, runtime.gui.capturing()));
    // The shared rotation manager (`RotationManager.onTick`): a visible
    // controller turns the view; it waits while a screen is open.
    let no_screen = matches!(game.screen, crate::game::Screen::Playing) && !gui_open;
    let sensitivity = f64::from(game.settings.sensitivity);
    let body = body(game);
    let view = (game.yaw, game.pitch);
    if let Some(p) = game.player.as_ref() { let eye = [p.position.x, p.position.y + f64::from(p.dimensions.eye_height), p.position.z]; with_client(&client, |runtime| runtime.rotation_eye(eye)); }
    let rotated = with_client(&client, |runtime| match &body {
        Some(body) => runtime.rotation_tick_at(view, sensitivity, no_screen, body),
        None => runtime.rotation_tick(view, sensitivity, no_screen),
    });
    if let Some([yaw, pitch]) = rotated {
        game.yaw = yaw;
        game.pitch = pitch;
    }
    // The reference's bus dispatches `EventPriority.values()` from LOWEREST
    // up, so `RotationManager.onTick` (LOWEST) turns the view and refreshes
    // `normalReachRayTrace` before the bridge modes' pre-tick reads it.
    let body = self::body(game);
    scaffold_bridge_tick(game, &client, no_screen, body);
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
        player.vape_rs_set_inv_walk(keys.filter(|_| !totem_hold));
    }
    wind_charge_tick(game, &client);
}

/// Scaffold GodBridge/TellyBridge's pre-tick: the slot, the right click
/// (`use.onTick(1)`) and the movement task's keys.
fn scaffold_bridge_tick(game: &mut Game<'_>, client: &crate::game::vape_rs::core::Client, no_screen: bool, body: Option<Body>) {
    let Some(body) = body else { return; };
    let bridging = with_client(client, |runtime| {
        runtime.enabled(ModuleId::Scaffold) && scaffold::Options::read(runtime.settings()).mode != scaffold::Mode::Legit
    });
    if !bridging {
        return;
    }
    let view = crate::game::vape_input_delays::host_view(game);
    let user = with_client(client, |runtime| {
        let movement = runtime.movement();
        Axis::ALL.map(|axis| movement.user(axis, &view))
    });
    let mouse = game.mouse.accumulated();
    let tick = BridgeTick {
        no_screen,
        body,
        inventory: inventory(game),
        hit: crosshair_block(game),
        right_mouse: game.mouse_reading().1[1],
        mouse: [mouse.0 as i32, mouse.1 as i32],
        view: [game.yaw, game.pitch],
        user,
    };
    let actions = with_client(client, |runtime| runtime.scaffold_bridge_tick(tick, &mut HostWorld(game)));
    let alive = game.player.as_ref().is_some_and(|p| p.health.health() > 0.0);
    if let Some(slot) = actions.select.filter(|s| *s < 9) {
        if alive && game.title.is_none() {
            game.selected = slot;
        }
    }
    if actions.click && alive {
        crate::game::remote::vape_macros::use_once(game);
    }
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
