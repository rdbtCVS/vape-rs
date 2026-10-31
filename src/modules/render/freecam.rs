// Freecam (reference `module/render/Freecam` with
// `freecam/FreecamModernController`, the controller 26.3 runs): the camera
// leaves the player and flies with the movement keys while the player stays
// where it is, facing where it faced. The player keeps ticking with no
// movement input (the arrow keys with Move Fake), so the server sees it
// stand still. The legacy controller (packet-cancelling, pre-1.21.11) does
// not apply here.
//
// The camera is the module's own position and rotation: the host builds the
// camera entity from `camera()` instead of the player, sends every mouse turn
// to `turn()`, and sets the player's rotation to `body` each tick.
use super::super::super::core::movement_input::{Axis, Owner, Request};
use super::super::super::core::rotation_runtime::Shared;
use super::super::super::core::settings::{Definition, Settings};
use super::super::combat::rotation::Claimant;
use serde_json::{Value, json};

pub const ID: &str = "render.freecam";
/// Above every other movement owner: the reference cancels the movement
/// bindings' pressed state and writes the player's input itself.
pub const PRIORITY: u8 = 10;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    // `addValue(horizontalSpeed, verticalSpeed)`, then Allow Interacting
    // (before 1.21.11 only), then `addValue(spawnFake, moveFake)`.
    vec![
        Definition::number(&key("speed"), "Speed", 1.0, 3.0, 5.0, 0.1),
        Definition::number(&key("vertical-speed"), "Vertical Speed", 1.0, 3.0, 5.0, 0.1),
        Definition::boolean(&key("spawn-fake"), "Spawn Fake", true),
        Definition::boolean(&key("move-fake"), "Move Fake", false)
            .when(&key("spawn-fake"), json!(true)),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub speed: f64,
    pub vertical_speed: f64,
    pub spawn_fake: bool,
    pub move_fake: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            speed: 3.0,
            vertical_speed: 3.0,
            spawn_fake: true,
            move_fake: false,
        }
    }
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let number = |k: &str, d: f64| get(k).and_then(Value::as_f64).unwrap_or(d);
        let boolean = |k: &str, d: bool| get(k).and_then(Value::as_bool).unwrap_or(d);
        Self {
            speed: number("speed", 3.0),
            vertical_speed: number("vertical-speed", 3.0),
            spawn_fake: boolean("spawn-fake", true),
            move_fake: boolean("move-fake", false),
        }
    }
    /// Move Fake's effective value: it depends on Spawn Fake.
    pub fn moves_fake(&self) -> bool {
        self.spawn_fake && self.move_fake
    }
}

/// The movement bindings' physical states (`ClientSettings.isPhysicalKeyDown`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}

/// The raw arrow keys (`KeyboardInput.isKeyDown(37..=40)`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Arrows {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

/// One client tick of the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    /// The world and the player exist.
    pub player: bool,
    pub health: f32,
    /// Feet plus the eye height.
    pub eye: [f64; 3],
    /// The player's rotation.
    pub view: (f32, f32),
    pub no_screen: bool,
    pub keys: Keys,
    pub arrows: Arrows,
}

/// The movement input the player ticks with while the camera is free
/// (`applyMovementInput`): the arrow keys with Move Fake, never a jump, a
/// sneak or a sprint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BodyKeys {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
}
impl BodyKeys {
    /// The movement writer's requests for the tick: the seven movement keys
    /// released (`onKeyBindingState`), the arrows' directions pressed.
    pub fn requests(self) -> Vec<Request> {
        Axis::ALL
            .into_iter()
            .map(|axis| Request {
                axis,
                owner: Owner::Freecam,
                priority: PRIORITY,
                pressed: match axis {
                    Axis::Forward => self.forward,
                    Axis::Back => self.back,
                    Axis::Left => self.left,
                    Axis::Right => self.right,
                    _ => false,
                },
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Output {
    /// The player's rotation: the body while the camera is free, the saved
    /// rotation on the disable (`restorePlayerView`).
    pub rotation: Option<(f32, f32)>,
    /// The player's movement input this tick, while the camera is free.
    pub body_keys: Option<BodyKeys>,
    /// `completeDisable` ran.
    pub finished: bool,
}

/// The free camera to render from this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub previous: [f64; 3],
    pub position: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Freecam {
    active: bool,
    initialization_pending: bool,
    disable_requested: bool,
    saved: (f32, f32),
    body: (f32, f32),
    camera: (f32, f32),
    position: [f64; 3],
    previous: [f64; 3],
    /// The player's input from the last tick, for the movement writer.
    body_keys: Option<BodyKeys>,
}

impl Freecam {
    /// `onEnable`: claim the rotation (priority 100); the controller
    /// captures the player on its next tick (the reference captures at
    /// once, but the runtime sees the player only in the tick).
    pub fn enable(&mut self, shared: &mut Shared<'_>) {
        self.active = false;
        self.disable_requested = false;
        self.initialization_pending = true;
        shared.claim(Claimant::Freecam, false);
    }
    /// `setEnabled(false)`: the disable completes on the next tick.
    pub fn request_disable(&mut self) {
        self.disable_requested = true;
    }
    pub fn disable_requested(&self) -> bool {
        self.disable_requested
    }
    pub fn active(&self) -> bool {
        self.active
    }
    /// A world change: nothing survives; the rotation claim goes with the
    /// session's.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// `onTick` (`EventPreTick`).
    pub fn tick(&mut self, input: &Input, options: Options, shared: &mut Shared<'_>) -> Output {
        if !input.player || input.health <= 0.0 {
            self.disable_requested = true;
        }
        if self.disable_requested {
            // restorePlayerView, then completeDisable and onDisable.
            let rotation = (self.active && input.player).then_some(self.saved);
            shared.release(Claimant::Freecam);
            self.reset();
            return Output {
                rotation,
                body_keys: None,
                finished: true,
            };
        }
        shared.claim(Claimant::Freecam, false);
        if self.initialization_pending || !self.active {
            self.capture(input);
        }
        self.handle_movement(input, options);
        let moves = options.moves_fake() && input.no_screen;
        let a = input.arrows;
        self.body_keys = Some(BodyKeys {
            forward: moves && a.up,
            back: moves && a.down,
            left: moves && a.left,
            right: moves && a.right,
        });
        Output {
            rotation: Some(self.body),
            body_keys: self.body_keys,
            finished: false,
        }
    }
    /// The movement writer's requests this tick (none once the camera is
    /// back on the player).
    pub fn requests(&self) -> Vec<Request> {
        self.body_keys.map(BodyKeys::requests).unwrap_or_default()
    }
    /// `captureState`.
    fn capture(&mut self, input: &Input) {
        self.saved = input.view;
        self.body = input.view;
        self.camera = input.view;
        self.position = input.eye;
        self.previous = input.eye;
        self.active = true;
        self.initialization_pending = false;
    }
    /// `handleMovement`: the physical movement keys fly the camera, the
    /// sprint key doubles both speeds; nothing moves with a screen open.
    fn handle_movement(&mut self, input: &Input, options: Options) {
        self.previous = self.position;
        if !input.no_screen {
            return;
        }
        let k = input.keys;
        let axis = |plus: bool, minus: bool| f64::from(u8::from(plus)) - f64::from(u8::from(minus));
        let (mut forward, mut strafe) = (axis(k.forward, k.back), axis(k.left, k.right));
        let vertical = axis(k.jump, k.sneak);
        let sprint = if k.sprint { 2.0 } else { 1.0 };
        let magnitude = (forward * forward + strafe * strafe).sqrt();
        if magnitude > 0.0 {
            forward /= magnitude;
            strafe /= magnitude;
            let speed = options.speed / 5.0 * sprint;
            let yaw = f64::from(self.camera.0 + 90.0).to_radians();
            self.position[0] += (forward * yaw.cos() + strafe * yaw.sin()) * speed;
            self.position[2] += (forward * yaw.sin() - strafe * yaw.cos()) * speed;
        }
        if vertical != 0.0 {
            self.position[1] += vertical * (options.vertical_speed / 5.0 * 0.42 * sprint);
        }
    }
    /// The camera to render from, while it is free
    /// (`updateRenderViewPosition` and `applyCameraToPlayer`).
    pub fn camera(&self) -> Option<Camera> {
        self.active.then_some(Camera {
            previous: self.previous,
            position: self.position,
            yaw: self.camera.0,
            pitch: self.camera.1,
        })
    }
    /// A mouse turn (`Entity.turn` degrees before its 0.15 scale): the
    /// camera turns, the player does not (`updateMouseLook`). False when the
    /// camera is not free and the player should turn.
    pub fn turn(&mut self, xo: f64, yo: f64) -> bool {
        if !self.active {
            return false;
        }
        self.camera.0 += (xo * 0.15) as f32;
        self.camera.1 = (self.camera.1 + (yo * 0.15) as f32).clamp(-90.0, 90.0);
        true
    }
    /// Spawn Fake draws the player where it stands (the reference's
    /// `EntityOtherPlayerMP` copy of it).
    pub fn shows_body(&self, options: Options) -> bool {
        self.active && options.spawn_fake
    }
    /// `suppressViewBobbing`: only while the arrow keys move the player.
    pub fn suppresses_bobbing(&self, options: Options) -> bool {
        self.active && options.moves_fake()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::rotation_runtime::Rotations;
    use super::super::super::combat::rotation::{Claim, Managed};
    use super::*;

    struct World {
        claim: Claim,
        managed: Managed,
        rotations: Rotations,
    }
    impl World {
        fn new() -> Self {
            Self {
                claim: Claim::default(),
                managed: Managed::default(),
                rotations: Rotations::default(),
            }
        }
        fn shared(&mut self) -> Shared<'_> {
            Shared {
                claim: &mut self.claim,
                managed: &mut self.managed,
                rotations: &mut self.rotations,
            }
        }
    }

    fn input() -> Input {
        Input {
            player: true,
            health: 20.0,
            eye: [0.5, 101.62, 0.5],
            view: (30.0, 10.0),
            no_screen: true,
            keys: Keys::default(),
            arrows: Arrows::default(),
        }
    }

    fn started(world: &mut World) -> Freecam {
        let mut f = Freecam::default();
        f.enable(&mut world.shared());
        f.tick(&input(), Options::default(), &mut world.shared());
        f
    }

    #[test]
    fn settings_match_the_reference() {
        let defs = definitions();
        let ids: Vec<_> = defs.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "render.freecam.speed",
                "render.freecam.vertical-speed",
                "render.freecam.spawn-fake",
                "render.freecam.move-fake"
            ]
        );
        let options = Options::read(&Settings::default());
        assert_eq!(options, Options::default());
        assert!(!options.moves_fake());
        let only_move = Options {
            spawn_fake: false,
            move_fake: true,
            ..Options::default()
        };
        assert!(!only_move.moves_fake());
    }

    #[test]
    fn enable_claims_the_rotation_and_the_tick_captures_the_player() {
        let mut world = World::new();
        let mut f = Freecam::default();
        f.enable(&mut world.shared());
        assert!(world.claim.owned_by(Claimant::Freecam));
        assert_eq!(f.camera(), None);
        let out = f.tick(&input(), Options::default(), &mut world.shared());
        assert_eq!(out.rotation, Some((30.0, 10.0)));
        assert_eq!(out.body_keys, Some(BodyKeys::default()));
        let camera = f.camera().unwrap();
        assert_eq!(camera.position, [0.5, 101.62, 0.5]);
        assert_eq!(camera.previous, camera.position);
        assert_eq!((camera.yaw, camera.pitch), (30.0, 10.0));
    }

    #[test]
    fn freecam_outranks_and_blocks_the_other_rotation_owners() {
        let mut world = World::new();
        assert!(world.shared().claim(Claimant::Scaffold, false));
        let mut f = Freecam::default();
        f.enable(&mut world.shared());
        // Queued behind the current owner, it blocks it until released.
        assert!(world.shared().blocked_for(Claimant::Scaffold));
        world.shared().release(Claimant::Scaffold);
        assert!(world.claim.owned_by(Claimant::Freecam));
        f.tick(&input(), Options::default(), &mut world.shared());
        assert!(world.shared().blocked_for(Claimant::SilentAura));
    }

    #[test]
    fn movement_follows_the_camera_yaw_and_the_speeds() {
        let mut world = World::new();
        let mut f = started(&mut world);
        // Yaw 30: forward is (-sin 30, cos 30) at Speed 3 / 5 = 0.6 a tick.
        let mut i = input();
        i.keys.forward = true;
        f.tick(&i, Options::default(), &mut world.shared());
        let c = f.camera().unwrap();
        let yaw = 30f64.to_radians();
        assert!((c.position[0] - (0.5 - yaw.sin() * 0.6)).abs() < 1e-9);
        assert!((c.position[2] - (0.5 + yaw.cos() * 0.6)).abs() < 1e-9);
        assert_eq!(c.previous, [0.5, 101.62, 0.5]);
        // A diagonal is normalized; the sprint key doubles it.
        let start = c.position;
        i.keys.left = true;
        i.keys.sprint = true;
        f.tick(&i, Options::default(), &mut world.shared());
        let c = f.camera().unwrap();
        let moved =
            ((c.position[0] - start[0]).powi(2) + (c.position[2] - start[2]).powi(2)).sqrt();
        assert!((moved - 1.2).abs() < 1e-9);
        // Jump rises Vertical Speed / 5 * 0.42, sneak falls; both cancel.
        let mut i = input();
        i.keys.jump = true;
        let y = c.position[1];
        f.tick(&i, Options::default(), &mut world.shared());
        assert!((f.camera().unwrap().position[1] - (y + 0.6 * 0.42)).abs() < 1e-9);
        i.keys.sneak = true;
        let y = f.camera().unwrap().position[1];
        f.tick(&i, Options::default(), &mut world.shared());
        assert_eq!(f.camera().unwrap().position[1], y);
    }

    #[test]
    fn a_screen_holds_the_camera() {
        let mut world = World::new();
        let mut f = started(&mut world);
        let mut i = input();
        i.keys.forward = true;
        i.no_screen = false;
        i.arrows.up = true;
        let out = f.tick(
            &i,
            Options {
                move_fake: true,
                ..Options::default()
            },
            &mut world.shared(),
        );
        assert_eq!(f.camera().unwrap().position, [0.5, 101.62, 0.5]);
        assert_eq!(out.body_keys, Some(BodyKeys::default()));
    }

    #[test]
    fn mouse_turns_the_camera_not_the_player() {
        let mut world = World::new();
        let mut f = Freecam::default();
        assert!(!f.turn(10.0, 10.0));
        f.enable(&mut world.shared());
        assert!(!f.turn(10.0, 10.0));
        f.tick(&input(), Options::default(), &mut world.shared());
        assert!(f.turn(100.0, 1000.0));
        let c = f.camera().unwrap();
        assert_eq!((c.yaw, c.pitch), (45.0, 90.0));
        let out = f.tick(&input(), Options::default(), &mut world.shared());
        assert_eq!(out.rotation, Some((30.0, 10.0)));
        // The camera's yaw steers the flight.
        let mut i = input();
        i.keys.forward = true;
        let before = f.camera().unwrap().position;
        f.tick(&i, Options::default(), &mut world.shared());
        let c = f.camera().unwrap();
        let yaw = 45f64.to_radians();
        assert!((c.position[0] - (before[0] - yaw.sin() * 0.6)).abs() < 1e-9);
    }

    #[test]
    fn move_fake_moves_the_player_with_the_arrow_keys_only() {
        let mut world = World::new();
        let mut f = started(&mut world);
        let mut i = input();
        i.arrows.up = true;
        i.arrows.left = true;
        i.keys.forward = true;
        let out = f.tick(&i, Options::default(), &mut world.shared());
        assert_eq!(out.body_keys, Some(BodyKeys::default()));
        let options = Options {
            move_fake: true,
            ..Options::default()
        };
        let out = f.tick(&i, options, &mut world.shared());
        assert_eq!(
            out.body_keys,
            Some(BodyKeys {
                forward: true,
                left: true,
                ..BodyKeys::default()
            })
        );
        assert!(f.suppresses_bobbing(options));
        assert!(!f.suppresses_bobbing(Options::default()));
        assert!(f.shows_body(Options::default()));
        assert!(!f.shows_body(Options {
            spawn_fake: false,
            ..Options::default()
        }));
    }

    #[test]
    fn the_movement_keys_are_released_above_every_other_owner() {
        let mut world = World::new();
        let mut f = Freecam::default();
        assert!(f.requests().is_empty());
        f.enable(&mut world.shared());
        f.tick(&input(), Options::default(), &mut world.shared());
        let requests = f.requests();
        assert_eq!(requests.len(), 7);
        assert!(
            requests
                .iter()
                .all(|r| !r.pressed && r.owner == Owner::Freecam)
        );
        let mut overrides = super::super::super::super::core::movement_input::Overrides::default();
        overrides.request(Request {
            axis: Axis::Sprint,
            owner: Owner::Sprint,
            priority: 4,
            pressed: true,
        });
        for request in requests {
            overrides.request(request);
        }
        assert!(!overrides.resolve(Axis::Sprint, true));
        assert!(!overrides.resolve(Axis::Forward, true));
        // Move Fake: the up arrow walks the player forward.
        let mut i = input();
        i.arrows.up = true;
        let options = Options {
            move_fake: true,
            ..Options::default()
        };
        f.tick(&i, options, &mut world.shared());
        let pressed: Vec<_> = f
            .requests()
            .into_iter()
            .filter(|r| r.pressed)
            .map(|r| r.axis)
            .collect();
        assert_eq!(pressed, [Axis::Forward]);
        f.request_disable();
        f.tick(&i, options, &mut world.shared());
        assert!(f.requests().is_empty());
    }

    #[test]
    fn disable_restores_the_saved_rotation_and_releases_the_claim() {
        let mut world = World::new();
        let mut f = started(&mut world);
        f.turn(200.0, 0.0);
        f.request_disable();
        // The camera stays free until the tick completes the disable.
        assert!(f.camera().is_some());
        let out = f.tick(&input(), Options::default(), &mut world.shared());
        assert_eq!(out.rotation, Some((30.0, 10.0)));
        assert!(out.finished);
        assert_eq!(f.camera(), None);
        assert!(!world.claim.owned_by(Claimant::Freecam));
        assert!(!f.turn(1.0, 1.0));
    }

    #[test]
    fn death_or_no_player_requests_the_disable() {
        let mut world = World::new();
        let mut f = started(&mut world);
        let mut i = input();
        i.health = 0.0;
        let out = f.tick(&i, Options::default(), &mut world.shared());
        assert!(out.finished);
        assert_eq!(out.rotation, Some((30.0, 10.0)));
        let mut f = started(&mut world);
        let mut i = input();
        i.player = false;
        let out = f.tick(&i, Options::default(), &mut world.shared());
        assert!(out.finished);
        assert_eq!(out.rotation, None);
    }
}
