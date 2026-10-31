// Runtime side of Freelook: its bind (which engages the freelook, never the
// module), and the host's per-tick, per-frame and per-turn entries.
use super::super::input::{BindingMode, KeyAction};
use super::super::modules::render::freelook::{self, Camera, Frame, Options, Perspective, Tick};
use super::lifecycle::CleanupReason;
use super::settings::Definition;
use super::{ModuleId, Runtime};
use serde_json::Value;

pub fn definitions() -> Vec<Definition> {
    freelook::definitions()
}

impl Runtime {
    fn freelook_required(&self) -> usize {
        self.settings
            .get(&format!("{}.binds", ModuleId::Freelook.id()))
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    }
    /// Freelook's bind: `onBindActivated` (a completed press) asks the
    /// module, which never toggles itself; the held keys stay tracked for
    /// Hold's `areBoundInputsDown`.
    pub(super) fn freelook_key(
        &mut self,
        key: &str,
        action: KeyAction,
        accepts_input: bool,
    ) -> bool {
        let required = self.freelook_required();
        let bind = &mut self
            .modules
            .get_mut(&ModuleId::Freelook)
            .expect("registered module")
            .binding;
        // Toggle from off reports every completed fresh press as Some(true).
        let pressed = bind.apply(
            key,
            action,
            BindingMode::Toggle,
            false,
            accepts_input,
            required,
        );
        let complete = bind.complete(required);
        if pressed == Some(true) {
            if self.enabled(ModuleId::Freelook) {
                let options = Options::read(&self.settings);
                // The bind only reaches a module with no screen open.
                self.freelook.bind_activated(&options, false);
            }
            return true;
        }
        accepts_input && action != KeyAction::Release && complete
    }
    /// Disabling the module ends the freelook (its events stop) and puts the
    /// perspective back.
    pub(super) fn freelook_toggled(&mut self, module: ModuleId, enabled: bool) {
        if module == ModuleId::Freelook && !enabled {
            self.freelook.release();
        }
    }
    /// Leaving the world ends the freelook; the perspective is still put
    /// back by the host.
    pub(super) fn freelook_session_reset(&mut self, reason: CleanupReason) {
        if matches!(
            reason,
            CleanupReason::OpenWorld
                | CleanupReason::CloseWorld
                | CleanupReason::Connect
                | CleanupReason::Disconnect
        ) {
            self.freelook.release();
        }
    }
    /// The post tick (`EventPostTick`), while the module is enabled.
    pub fn freelook_tick(&mut self, screen: bool, player: bool) {
        if !self.enabled(ModuleId::Freelook) {
            return;
        }
        let bind_down = self
            .modules
            .get(&ModuleId::Freelook)
            .expect("registered module")
            .binding
            .complete(self.freelook_required());
        let options = Options::read(&self.settings);
        self.freelook.tick(
            &Tick {
                screen,
                player,
                bind_down,
            },
            &options,
        );
    }
    /// The perspective the host must put back, once.
    pub fn freelook_restore(&mut self) -> Option<Perspective> {
        self.freelook.take_restore()
    }
    /// The camera to render from this frame, if the freelook is engaged.
    pub fn freelook_frame(&mut self, frame: &Frame) -> Option<Camera> {
        if !self.enabled(ModuleId::Freelook) {
            return None;
        }
        let options = Options::read(&self.settings);
        self.freelook.frame(frame, &options)
    }
    /// A view turn in cursor units; true when the freelook took it.
    pub fn freelook_turn(&mut self, dx: f64, dy: f64) -> bool {
        self.enabled(ModuleId::Freelook) && self.freelook.turn(dx, dy)
    }
}

#[cfg(test)]
mod tests {
    use super::super::lifecycle::Context;
    use super::*;
    use serde_json::json;

    fn runtime(activation: &str) -> Runtime {
        let mut runtime = Runtime::default();
        runtime.observe_context(Context {
            in_world: true,
            alive: true,
            interactive: true,
        });
        runtime
            .settings
            .set("render.freelook.binds", json!(["KeyV"]))
            .unwrap();
        runtime
            .settings
            .set("render.freelook.activate", json!(activation))
            .unwrap();
        runtime
    }
    fn frame() -> Frame {
        Frame {
            view: (10.0, 20.0),
            perspective: Perspective::First,
            screen: false,
            game_sensitivity: 0.5,
        }
    }

    #[test]
    fn the_bind_engages_hold_without_toggling_the_module() {
        let mut r = runtime("Hold");
        // Disabled: the bind is consumed but nothing engages.
        assert!(r.freelook_key("KeyV", KeyAction::Press, true));
        r.freelook_tick(false, true);
        assert!(!r.enabled(ModuleId::Freelook));
        assert_eq!(r.freelook_frame(&frame()), None);
        assert!(!r.freelook_key("KeyV", KeyAction::Release, true));
        r.set_enabled(ModuleId::Freelook, true);
        r.freelook_key("KeyV", KeyAction::Press, true);
        r.freelook_tick(false, true);
        assert!(r.enabled(ModuleId::Freelook));
        assert!(r.freelook_frame(&frame()).is_some());
        assert!(r.freelook_turn(10.0, 0.0));
        r.freelook_key("KeyV", KeyAction::Release, true);
        r.freelook_tick(false, true);
        assert_eq!(r.freelook_frame(&frame()), None);
        assert_eq!(r.freelook_restore(), Some(Perspective::First));
        assert!(r.enabled(ModuleId::Freelook));
    }

    #[test]
    fn toggle_flips_on_presses_and_disabling_restores() {
        let mut r = runtime("Toggle");
        r.set_enabled(ModuleId::Freelook, true);
        r.freelook_key("KeyV", KeyAction::Press, true);
        r.freelook_key("KeyV", KeyAction::Release, true);
        r.freelook_tick(false, true);
        assert!(r.freelook_frame(&frame()).is_some());
        // A repeat is no new press.
        r.freelook_key("KeyV", KeyAction::Press, true);
        r.freelook_key("KeyV", KeyAction::Repeat, true);
        r.freelook_tick(false, true);
        assert_eq!(r.freelook_frame(&frame()), None);
        r.freelook_key("KeyV", KeyAction::Release, true);
        r.freelook_key("KeyV", KeyAction::Press, true);
        r.freelook_tick(false, true);
        r.freelook_frame(&frame());
        r.freelook_restore();
        r.set_enabled(ModuleId::Freelook, false);
        assert_eq!(r.freelook_restore(), Some(Perspective::First));
        assert!(!r.freelook_turn(1.0, 0.0));
        assert_eq!(r.freelook_frame(&frame()), None);
    }

    #[test]
    fn leaving_the_world_or_focus_ends_the_freelook() {
        let mut r = runtime("Hold");
        r.set_enabled(ModuleId::Freelook, true);
        r.freelook_key("KeyV", KeyAction::Press, true);
        r.freelook_tick(false, true);
        r.freelook_frame(&frame());
        r.freelook_session_reset(CleanupReason::CloseWorld);
        assert_eq!(r.freelook_restore(), Some(Perspective::First));
        r.freelook_tick(false, true);
        r.freelook_frame(&frame());
        // Focus loss lets go of the bind: Hold disengages on the next tick.
        r.freelook_key("", KeyAction::FocusLost, false);
        r.freelook_tick(false, true);
        assert_eq!(r.freelook_restore(), Some(Perspective::First));
    }
}
