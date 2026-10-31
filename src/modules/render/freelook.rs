// Freelook (reference `module/render/hud/FreeLookHudModule`, a GAME-group
// HUD module): while its bind is held (or toggled on), the camera turns with
// the mouse in third person and the player keeps its rotation, so it walks
// and aims where it faced. Enabling the module (the GUI) arms it; the bind
// activates the freelook, it never toggles the module.
//
// The reference's names are swapped by its obfuscation (its "pitch" fields
// hold the yaw, its "yaw" fields the pitch); here they are named for what
// they hold.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::{Value, json};

pub const ID: &str = "render.freelook";

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let mut custom =
        Definition::boolean(&key("custom-sensitivity"), "Use Custom Sensitivity", false);
    custom.description =
        "Enable to set a separate sensitivity from Minecraft using a slider".into();
    // `addValue(activationMode, startingPositionMode, customSensitivity,
    // sensitivity)`. The Perspective value is built but never added, so it
    // keeps its default: the view is always third person.
    vec![
        Definition::choice(
            &key("activate"),
            "Activate Freelook",
            &["Hold", "Toggle"],
            "Hold",
        ),
        Definition::choice(
            &key("starting-position"),
            "Starting Position",
            &["Forward", "Backward"],
            "Forward",
        ),
        custom,
        // `NumberValue.create(.., "#.#", "", 0.001, 0.5, 1.0)`: the default
        // increment, 0.01.
        Definition::number(&key("sensitivity"), "Sensitivity", 0.001, 0.5, 1.0, 0.01)
            .when(&key("custom-sensitivity"), json!(true)),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    Hold,
    Toggle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub activation: Activation,
    pub backward: bool,
    pub custom_sensitivity: bool,
    pub sensitivity: f64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            activation: Activation::Hold,
            backward: false,
            custom_sensitivity: false,
            sensitivity: 0.5,
        }
    }
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let choice = |k: &str| get(k).and_then(Value::as_str).unwrap_or("");
        Self {
            activation: if choice("activate") == "Toggle" {
                Activation::Toggle
            } else {
                Activation::Hold
            },
            backward: choice("starting-position") == "Backward",
            custom_sensitivity: get("custom-sensitivity")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            sensitivity: get("sensitivity").and_then(Value::as_f64).unwrap_or(0.5),
        }
    }
    /// `getSensitivity`: the slider, or the game's mouse sensitivity scaled
    /// as the reference writes it (`* 0.6 * 0.2 * 8`, not vanilla's cube).
    pub fn sensitivity(&self, game: f64) -> f64 {
        if self.custom_sensitivity {
            self.sensitivity
        } else {
            game * 0.6 * 0.2 * 8.0
        }
    }
}

/// The host's camera type (`GameSettings.thirdPersonView` 0, 1 and 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perspective {
    First,
    Back,
    Front,
}

/// One render frame (`EventPreRenderTick`).
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// The player's rotation, (yaw, pitch).
    pub view: (f32, f32),
    pub perspective: Perspective,
    /// A screen (or the click GUI) is open.
    pub screen: bool,
    /// The game's mouse sensitivity option.
    pub game_sensitivity: f64,
}

/// The camera to render from this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub perspective: Perspective,
}

/// One client tick (`EventPostTick`).
#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub screen: bool,
    pub player: bool,
    /// `areBoundInputsDown`: every bound input is held.
    pub bind_down: bool,
}

#[derive(Debug, Default)]
pub struct Freelook {
    /// The reference's `enabled`: the bind holds (or toggled) the freelook.
    engaged: bool,
    /// `active`: the first frame took the rotation to start from.
    active: bool,
    /// `originalPitch`, `originalYaw`: the player's (yaw, pitch) then.
    original: (f32, f32),
    /// `renderPitch`, `renderYaw`: the camera's (yaw, pitch).
    render: (f32, f32),
    /// `yawOffset`, `pitchOffset`.
    offset: (f32, f32),
    saved_perspective: Option<Perspective>,
    /// `onBindActivated`'s `POST_TICK_EXECUTOR.execute(toggleFreelook)`.
    toggle_requested: bool,
    /// The perspective to put back (`savedPerspective`), for the host.
    restore: Option<Perspective>,
    /// The mouse's movement since the last frame in the host's cursor units,
    /// right and down positive (the reference's `mouseDX`, `mouseDY` are
    /// `lastX - x` and `lastY - y`: their negation).
    pending: (f64, f64),
}

impl Freelook {
    /// `isActive`.
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn engaged(&self) -> bool {
        self.engaged
    }
    /// `onBindActivated` (the module is enabled): Toggle flips the freelook
    /// after the tick; nothing behind a screen.
    pub fn bind_activated(&mut self, options: &Options, screen: bool) {
        if !screen && options.activation == Activation::Toggle {
            self.toggle_requested = true;
        }
    }
    /// The post tick: the queued toggle, then Hold's `onPostTick`.
    pub fn tick(&mut self, tick: &Tick, options: &Options) {
        if std::mem::take(&mut self.toggle_requested) {
            self.set_engaged(!self.engaged);
        }
        if options.activation != Activation::Hold || tick.screen || !tick.player {
            return;
        }
        self.set_engaged(tick.bind_down);
    }
    fn set_engaged(&mut self, engaged: bool) {
        if engaged && !self.engaged {
            self.pending = (0.0, 0.0);
        }
        self.engaged = engaged;
        if !engaged {
            // `active = false`, then `gameSettings.I(savedPerspective)` once.
            self.active = false;
            if let Some(perspective) = self.saved_perspective.take() {
                self.restore = Some(perspective);
            }
        }
    }
    /// The module was disabled or the world left: the freelook ends and the
    /// perspective goes back.
    pub fn release(&mut self) {
        self.toggle_requested = false;
        self.set_engaged(false);
    }
    /// The perspective the host must restore, once.
    pub fn take_restore(&mut self) -> Option<Perspective> {
        self.restore.take()
    }
    /// A view turn in cursor units: the freelook takes it while engaged and
    /// the player does not turn (`onPostMouseUpdate`'s `restorePlayerView`).
    pub fn turn(&mut self, dx: f64, dy: f64) -> bool {
        if !self.engaged {
            return false;
        }
        self.pending.0 += dx;
        self.pending.1 += dy;
        true
    }
    /// `onPreRenderTick`: the camera to render from, if engaged.
    pub fn frame(&mut self, frame: &Frame, options: &Options) -> Option<Camera> {
        if !self.engaged {
            return None;
        }
        if !self.active {
            self.original = frame.view;
            self.saved_perspective = Some(frame.perspective);
            self.offset = (if options.backward { 180.0 } else { 0.0 }, 0.0);
            self.render = self.original;
            self.active = true;
            return Some(Camera {
                yaw: self.render.0,
                pitch: self.render.1,
                perspective: frame.perspective,
            });
        }
        // The Perspective value is never added: always "3rd Person" (1).
        let perspective = Perspective::Back;
        let (dx, dy) = std::mem::take(&mut self.pending);
        let (mouse_dx, mouse_dy) = if frame.screen {
            self.offset = (0.0, 0.0);
            (0.0, 0.0)
        } else {
            (-dx, -dy)
        };
        let sensitivity = options.sensitivity(frame.game_sensitivity);
        let yaw_delta = mouse_dx * sensitivity * 0.15;
        let pitch_delta = mouse_dy * sensitivity * 0.15;
        let (yaw, pitch) = self.original;
        self.render.0 = (yaw_delta - f64::from(self.offset.0) + f64::from(yaw)) as f32;
        self.render.1 = clamp(
            (pitch_delta - f64::from(self.offset.1) + f64::from(pitch)) as f32,
            -90.0,
            90.0,
        );
        self.offset.0 = (yaw_delta + f64::from(self.offset.0)) as f32;
        self.offset.1 = (pitch_delta + f64::from(self.offset.1)) as f32;
        self.offset.1 = clamp(self.offset.1, -(90.0 - pitch), 90.0 + pitch);
        Some(Camera {
            yaw: self.render.0,
            pitch: self.render.1,
            perspective,
        })
    }
    /// The session ended: nothing carries over.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// `MathUtil.clamp`.
fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        Frame {
            view: (10.0, 20.0),
            perspective: Perspective::First,
            screen: false,
            game_sensitivity: 0.5,
        }
    }
    fn hold(down: bool) -> Tick {
        Tick {
            screen: false,
            player: true,
            bind_down: down,
        }
    }
    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }
    /// Engaged by Hold, the first frame taken.
    fn started(options: &Options) -> Freelook {
        let mut f = Freelook::default();
        f.tick(&hold(true), options);
        let first = f.frame(&frame(), options).unwrap();
        assert_eq!((first.yaw, first.pitch), (10.0, 20.0));
        assert_eq!(first.perspective, Perspective::First);
        f
    }
    /// Frames until the camera settles after one turn.
    fn settle(f: &mut Freelook, options: &Options) -> Camera {
        let mut last = None;
        for _ in 0..3 {
            last = f.frame(&frame(), options);
        }
        last.unwrap()
    }

    #[test]
    fn the_sensitivity_is_the_slider_or_the_reference_scaling() {
        let mut options = Options::default();
        assert!((options.sensitivity(0.5) - 0.48).abs() < 1e-12);
        options.custom_sensitivity = true;
        options.sensitivity = 0.3;
        assert_eq!(options.sensitivity(0.5), 0.3);
    }

    #[test]
    fn hold_turns_the_camera_not_the_player_and_restores_the_perspective() {
        let options = Options::default();
        let mut f = started(&options);
        assert!(f.active());
        // 200 cursor units right (a 30 degree vanilla turn at 0.5).
        assert!(f.turn(200.0, 0.0));
        // The frame of the movement jerks back by it before it settles.
        let jerk = f.frame(&frame(), &options).unwrap();
        assert!(close(jerk.yaw, 10.0 - 14.4), "{jerk:?}");
        assert_eq!(jerk.perspective, Perspective::Back);
        let settled = settle(&mut f, &options);
        assert!(close(settled.yaw, 10.0 + 14.4), "{settled:?}");
        assert!(close(settled.pitch, 20.0));
        assert_eq!(f.take_restore(), None);
        f.tick(&hold(false), &options);
        assert!(!f.engaged() && !f.active());
        assert_eq!(f.take_restore(), Some(Perspective::First));
        assert_eq!(f.take_restore(), None);
        assert!(!f.turn(1.0, 0.0));
        assert_eq!(f.frame(&frame(), &options), None);
    }

    #[test]
    fn custom_sensitivity_scales_the_turn() {
        let mut options = Options {
            custom_sensitivity: true,
            ..Options::default()
        };
        for (sensitivity, degrees) in [(0.5, 15.0), (1.0, 30.0)] {
            options.sensitivity = sensitivity;
            let mut f = started(&options);
            f.turn(200.0, 0.0);
            let camera = settle(&mut f, &options);
            assert!(close(camera.yaw, 10.0 + degrees), "{camera:?}");
        }
    }

    #[test]
    fn backward_starts_behind_and_pitch_follows_the_mouse_within_bounds() {
        let options = Options {
            backward: true,
            ..Options::default()
        };
        let mut f = started(&options);
        let camera = f.frame(&frame(), &options).unwrap();
        assert!(close(camera.yaw, 10.0 - 180.0));
        // Down raises the pitch; far down stops at 90.
        f.turn(0.0, 100.0);
        assert!(close(settle(&mut f, &options).pitch, 20.0 + 7.2));
        f.turn(0.0, 10_000.0);
        assert_eq!(settle(&mut f, &options).pitch, 90.0);
        // The offset is clamped too: coming back up starts at once.
        f.turn(0.0, -100.0);
        assert!(close(settle(&mut f, &options).pitch, 90.0 - 7.2));
    }

    #[test]
    fn toggle_flips_after_the_tick_and_ignores_hold_state() {
        let options = Options {
            activation: Activation::Toggle,
            ..Options::default()
        };
        let mut f = Freelook::default();
        f.bind_activated(&options, true);
        f.tick(&hold(false), &options);
        assert!(!f.engaged(), "nothing behind a screen");
        f.bind_activated(&options, false);
        assert!(!f.engaged(), "deferred to the post tick");
        f.tick(&hold(false), &options);
        assert!(f.engaged());
        f.frame(&frame(), &options);
        f.frame(&frame(), &options);
        f.tick(&hold(false), &options);
        assert!(f.engaged(), "the bind's state does not matter");
        f.bind_activated(&options, false);
        f.tick(&hold(false), &options);
        assert!(!f.engaged());
        assert_eq!(f.take_restore(), Some(Perspective::First));
    }

    #[test]
    fn a_screen_holds_the_state_and_recentres_the_camera() {
        let options = Options {
            backward: true,
            ..Options::default()
        };
        let mut f = started(&options);
        f.turn(200.0, 0.0);
        settle(&mut f, &options);
        // Hold's tick behind a screen changes nothing.
        f.tick(
            &Tick {
                screen: true,
                player: true,
                bind_down: false,
            },
            &options,
        );
        assert!(f.engaged());
        let screen = Frame {
            screen: true,
            ..frame()
        };
        let camera = f.frame(&screen, &options).unwrap();
        assert!(close(camera.yaw, 10.0) && close(camera.pitch, 20.0));
        // The offsets were reset: the start position is gone after it.
        let after = f.frame(&frame(), &options).unwrap();
        assert!(close(after.yaw, 10.0));
    }

    #[test]
    fn release_restores_once_and_a_new_hold_starts_over() {
        let options = Options::default();
        let mut f = started(&options);
        f.frame(&frame(), &options);
        f.release();
        assert_eq!(f.take_restore(), Some(Perspective::First));
        f.release();
        assert_eq!(f.take_restore(), None);
        // A turn made before the hold is not replayed into the next one.
        f.turn(500.0, 0.0);
        f.tick(&hold(true), &options);
        let start = Frame {
            view: (50.0, 0.0),
            perspective: Perspective::Front,
            ..frame()
        };
        let first = f.frame(&start, &options).unwrap();
        assert_eq!((first.yaw, first.pitch), (50.0, 0.0));
        let next = f.frame(&start, &options).unwrap();
        assert_eq!((next.yaw, next.perspective), (50.0, Perspective::Back));
        f.tick(&hold(false), &options);
        assert_eq!(f.take_restore(), Some(Perspective::Front));
    }
}
