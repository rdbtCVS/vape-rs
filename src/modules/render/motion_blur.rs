// MotionBlur (`MotionBlur`, HUD group GAME): each presented frame, with the
// HUD and every menu drawn, is mixed with the previous blended frame, which
// becomes the next frame's history. The reference's per-frame order: copy
// the frame, seed the history on the first frame of a size, scale the blend
// by the camera's turn ("Velocity Adaptive") and by the frame rate ("FPS
// Modulate"), draw, keep the result. "Apply On Menu" off skips frames while
// the click GUI is open and "Apply On Game Menu" off while a game screen is;
// a skipped frame keeps the history and every rate. Off frees the textures,
// so the host's frame is untouched and the next enable seeds afresh.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::Value;

pub const ID: &str = "render.motion-blur";

fn described(mut d: Definition, text: &str) -> Definition {
    d.description = text.into();
    d
}

pub fn definitions() -> Vec<Definition> {
    vec![
        described(
            Definition::number(
                &format!("{ID}.blurriness"),
                "Blurriness",
                0.0,
                5.0,
                10.0,
                0.1,
            ),
            "How strong the motion blur trail is",
        ),
        described(
            Definition::boolean(
                &format!("{ID}.velocity-adaptive"),
                "Velocity Adaptive",
                true,
            ),
            "Scales the blur strength with camera movement speed",
        ),
        described(
            Definition::boolean(&format!("{ID}.smooth-blur"), "Smooth Blur", false),
            "Smooths the blur trail (may blur the game UI too)",
        ),
        described(
            Definition::boolean(&format!("{ID}.fps-modulate"), "FPS Modulate", true),
            "Reduces blur strength at low frame rates",
        ),
        described(
            Definition::boolean(&format!("{ID}.clear-color"), "Clear Color", false),
            "Renders a faded grayscale trail instead of RGB",
        ),
        described(
            Definition::boolean(&format!("{ID}.apply-on-menu"), "Apply On Menu", true),
            "Keep applying the blur while the Vape GUI is open",
        ),
        described(
            Definition::boolean(
                &format!("{ID}.apply-on-game-menu"),
                "Apply On Game Menu",
                true,
            ),
            "Keep applying the blur while a game menu (pause/inventory/chat) is open",
        ),
    ]
}

/// One frame's blend: the reference's shader uniforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pass {
    /// `min(curBlurriness * 2 / 11, 0.95)`, the history's mix weight.
    pub blurriness: f32,
    /// 0..=1 by the camera's turn; above zero the weight rises to
    /// `(1 + velocity) / 2` of `blurriness` and "Smooth Blur" spreads the
    /// history by `0.0006 * velocity` of the screen.
    pub velocity_factor: f32,
    pub smooth: bool,
    /// Off ("Clear Color"): the frame's red channel stands in for its colour.
    pub rgb: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    /// Disabled: free the textures; the frame is the host's.
    Off,
    /// Enabled, this frame left as drawn, the history kept.
    Skip,
    Draw(Pass),
}

/// What the frame shows, read before it is presented.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    /// The click GUI or the HUD editor (`!ClientSettings.isInputEnabled()`).
    pub gui_open: bool,
    /// A host screen (`currentScreen != null`): the title, pause, inventory,
    /// chat or a dialog.
    pub screen_open: bool,
    /// The camera's yaw and pitch in degrees, when a world is drawn.
    pub view: Option<[f32; 2]>,
    /// A monotonic clock (`System.nanoTime()`).
    pub now_nanos: u64,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    enabled: bool,
    velocity_factor: f32,
    cur_blurriness: f32,
    last_frame_nanos: u64,
    /// `lastViewX`/`lastViewY`, not reset by enabling.
    last_view: [f32; 2],
}

fn flag(settings: &Settings, key: &str, default: bool) -> bool {
    settings
        .get(&format!("{ID}.{key}"))
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

/// The reference's velocity target for a turn of `delta` degrees in a frame.
pub fn velocity_target(delta: f32) -> f32 {
    (delta * 10.0 - 1.0).clamp(0.0, 15.0) / 15.0
}

/// The weight "FPS Modulate" leaves of `blurriness` after a frame of
/// `elapsed` nanoseconds: `(min(fps, 1000) / 1000) ^ 0.2`.
pub fn fps_attenuation(elapsed: u64) -> f32 {
    let fps = 1.0e9_f32 / elapsed as f32;
    (fps.clamp(0.0, 1000.0) / 1000.0).powf(0.2)
}

/// The yaw change from `from` to `to` over the shorter way round, so a
/// wrapped heading is the turn the camera made.
fn turn(from: f32, to: f32) -> f32 {
    let d = (to - from).rem_euclid(360.0);
    if d > 180.0 { d - 360.0 } else { d }
}

impl State {
    pub fn frame(&mut self, enabled: bool, settings: &Settings, frame: Frame) -> Request {
        let blurriness = settings
            .get(&format!("{ID}.blurriness"))
            .and_then(Value::as_f64)
            .unwrap_or(5.0) as f32;
        if !enabled {
            self.enabled = false;
            return Request::Off;
        }
        if !self.enabled {
            // onEnable.
            self.enabled = true;
            self.velocity_factor = 0.0;
            self.cur_blurriness = blurriness;
            self.last_frame_nanos = 0;
        }
        if !flag(settings, "apply-on-menu", true) && frame.gui_open
            || !flag(settings, "apply-on-game-menu", true) && frame.screen_open
        {
            return Request::Skip;
        }
        if flag(settings, "velocity-adaptive", true) {
            let view = frame.view.unwrap_or(self.last_view);
            let delta =
                (view[1] - self.last_view[1]).abs() + turn(self.last_view[0], view[0]).abs();
            self.last_view = view;
            if delta > 0.01 {
                self.velocity_factor = velocity_target(delta);
            }
        } else {
            self.velocity_factor = 1.0;
        }
        if flag(settings, "fps-modulate", true) {
            self.cur_blurriness = if self.last_frame_nanos != 0 {
                blurriness * fps_attenuation(frame.now_nanos.saturating_sub(self.last_frame_nanos))
            } else {
                blurriness
            };
            self.last_frame_nanos = frame.now_nanos;
        } else {
            self.cur_blurriness = blurriness;
        }
        Request::Draw(Pass {
            blurriness: (self.cur_blurriness * 2.0 / 11.0).min(0.95),
            velocity_factor: self.velocity_factor,
            smooth: flag(settings, "smooth-blur", false),
            rgb: !flag(settings, "clear-color", false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn settings(pairs: &[(&str, Value)]) -> Settings {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        for (k, v) in pairs {
            s.set(&format!("{ID}.{k}"), v.clone()).unwrap();
        }
        s
    }

    fn at(now_nanos: u64, view: [f32; 2]) -> Frame {
        Frame {
            view: Some(view),
            now_nanos,
            ..Frame::default()
        }
    }

    fn draw(r: Request) -> Pass {
        match r {
            Request::Draw(p) => p,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn definitions_match_the_reference() {
        let d = definitions();
        let labels: Vec<_> = d.iter().map(|d| d.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Blurriness",
                "Velocity Adaptive",
                "Smooth Blur",
                "FPS Modulate",
                "Clear Color",
                "Apply On Menu",
                "Apply On Game Menu"
            ]
        );
        assert_eq!(
            d.iter().map(|d| d.default.clone()).collect::<Vec<_>>(),
            [
                json!(5.0),
                json!(true),
                json!(false),
                json!(true),
                json!(false),
                json!(true),
                json!(true)
            ]
        );
        assert!(d.iter().all(|d| !d.description.is_empty()));
    }

    #[test]
    fn off_frees_and_first_frame_uses_the_full_blurriness() {
        let s = settings(&[]);
        let mut state = State::default();
        assert_eq!(state.frame(false, &s, at(1, [0.0, 0.0])), Request::Off);
        let p = draw(state.frame(true, &s, at(1_000, [0.0, 0.0])));
        // 5 * 2 / 11, no turn yet.
        assert!((p.blurriness - 10.0 / 11.0).abs() < 1e-6);
        assert_eq!(p.velocity_factor, 0.0);
        assert!(p.rgb && !p.smooth);
    }

    #[test]
    fn the_weight_is_clamped_to_095() {
        let s = settings(&[("blurriness", json!(10.0)), ("fps-modulate", json!(false))]);
        let p = draw(State::default().frame(true, &s, at(1, [0.0, 0.0])));
        assert_eq!(p.blurriness, 0.95);
    }

    #[test]
    fn fps_modulation_scales_by_the_fifth_root_of_the_rate() {
        let s = settings(&[]);
        let mut state = State::default();
        state.frame(true, &s, at(1_000_000, [0.0, 0.0]));
        // 100 fps: 5 * 0.1^0.2.
        let p = draw(state.frame(true, &s, at(11_000_000, [0.0, 0.0])));
        let cur = 5.0 * 0.1_f32.powf(0.2);
        assert!((p.blurriness - cur * 2.0 / 11.0).abs() < 1e-6);
        // Above 1000 fps the weight is whole.
        assert_eq!(fps_attenuation(1), 1.0);
        assert_eq!(fps_attenuation(0), 1.0);
    }

    #[test]
    fn velocity_follows_the_turn_and_holds_when_still() {
        let s = settings(&[("fps-modulate", json!(false))]);
        let mut state = State::default();
        state.frame(true, &s, at(1, [0.0, 0.0]));
        // 1 degree: (10 - 1) / 15.
        let p = draw(state.frame(true, &s, at(2, [1.0, 0.0])));
        assert!((p.velocity_factor - 0.6).abs() < 1e-6);
        // Still: the factor holds.
        let p = draw(state.frame(true, &s, at(3, [1.0, 0.0])));
        assert!((p.velocity_factor - 0.6).abs() < 1e-6);
        // A wrapped heading is a small turn, pitch counts too.
        let p = draw(state.frame(true, &s, at(4, [-358.5, 0.5])));
        assert!((p.velocity_factor - velocity_target(1.0)).abs() < 1e-5);
        // A big turn saturates.
        let p = draw(state.frame(true, &s, at(5, [-300.0, 0.5])));
        assert_eq!(p.velocity_factor, 1.0);
        // Off: always whole.
        let s = settings(&[("velocity-adaptive", json!(false))]);
        assert_eq!(
            draw(State::default().frame(true, &s, at(1, [0.0, 0.0]))).velocity_factor,
            1.0
        );
    }

    #[test]
    fn menus_skip_only_when_their_option_is_off() {
        let gui = Frame {
            gui_open: true,
            ..at(1, [0.0, 0.0])
        };
        let screen = Frame {
            screen_open: true,
            ..at(1, [0.0, 0.0])
        };
        let on = settings(&[]);
        assert!(matches!(
            State::default().frame(true, &on, gui),
            Request::Draw(_)
        ));
        assert!(matches!(
            State::default().frame(true, &on, screen),
            Request::Draw(_)
        ));
        let off = settings(&[
            ("apply-on-menu", json!(false)),
            ("apply-on-game-menu", json!(false)),
        ]);
        assert_eq!(State::default().frame(true, &off, gui), Request::Skip);
        assert_eq!(State::default().frame(true, &off, screen), Request::Skip);
    }

    #[test]
    fn skipped_frames_keep_the_clock_and_view() {
        let s = settings(&[("apply-on-menu", json!(false))]);
        let mut state = State::default();
        state.frame(true, &s, at(1_000_000, [0.0, 0.0]));
        let skipped = Frame {
            gui_open: true,
            ..at(500_000_000, [90.0, 0.0])
        };
        assert_eq!(state.frame(true, &s, skipped), Request::Skip);
        // The next drawn frame measures from the last drawn one.
        let p = draw(state.frame(true, &s, at(1_001_000_000, [0.0, 0.0])));
        assert!((p.blurriness - 5.0 * 0.001_f32.powf(0.2) * 2.0 / 11.0).abs() < 1e-6);
        assert_eq!(p.velocity_factor, 0.0);
    }

    #[test]
    fn re_enabling_resets_the_rates() {
        let s = settings(&[]);
        let mut state = State::default();
        state.frame(true, &s, at(1, [0.0, 0.0]));
        draw(state.frame(true, &s, at(2, [5.0, 0.0])));
        assert_eq!(state.frame(false, &s, at(3, [5.0, 0.0])), Request::Off);
        let p = draw(state.frame(true, &s, at(1_000_000_000, [5.0, 0.0])));
        assert_eq!(p.velocity_factor, 0.0);
        assert!((p.blurriness - 10.0 / 11.0).abs() < 1e-6);
    }

    #[test]
    fn clear_color_and_smooth_reach_the_pass() {
        let s = settings(&[("clear-color", json!(true)), ("smooth-blur", json!(true))]);
        let p = draw(State::default().frame(true, &s, at(1, [0.0, 0.0])));
        assert!(!p.rgb && p.smooth);
    }
}
