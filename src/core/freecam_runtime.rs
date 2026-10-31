// Runtime side of Freecam: the module's state, its enable and disable, and
// the host's per-tick, per-frame and per-turn entries.
use super::super::modules::render::freecam::{self, Camera, Input, Options, Output};
use super::lifecycle::CleanupReason;
use super::rotation_runtime::Shared;
use super::settings::Definition;
use super::{ModuleId, Runtime};

pub fn definitions() -> Vec<Definition> {
    freecam::definitions()
}

impl Runtime {
    /// The host should gather the player and the keys this tick.
    pub fn freecam_wanted(&self) -> bool {
        self.session().in_world()
            && (self.enabled(ModuleId::Freecam) || self.freecam.disable_requested())
    }
    /// `onEnable` claims the rotation; `setEnabled(false)` leaves the
    /// restore to the next tick.
    pub(super) fn freecam_toggled(&mut self, module: ModuleId, enabled: bool) {
        if module != ModuleId::Freecam {
            return;
        }
        if enabled {
            let mut shared = Shared::of(&mut self.combat, &mut self.rotations);
            self.freecam.enable(&mut shared);
        } else {
            self.freecam.request_disable();
        }
    }
    /// Leaving the world, or dying, disables Freecam (`onTick` with no
    /// world or no health); the rest of the session keeps the camera.
    pub(super) fn freecam_session_reset(&mut self, reason: CleanupReason) {
        if !matches!(
            reason,
            CleanupReason::OpenWorld
                | CleanupReason::CloseWorld
                | CleanupReason::Connect
                | CleanupReason::Disconnect
                | CleanupReason::Death
        ) {
            return;
        }
        self.set_enabled(ModuleId::Freecam, false);
        let mut shared = Shared::of(&mut self.combat, &mut self.rotations);
        shared.release(super::super::modules::combat::rotation::Claimant::Freecam);
        self.freecam.reset();
    }
    /// One client tick, before the movement writer.
    pub fn freecam_tick(&mut self, input: &Input) -> Output {
        if self.enabled(ModuleId::Freecam) && (!input.player || input.health <= 0.0) {
            self.set_enabled(ModuleId::Freecam, false);
        }
        let options = Options::read(&self.settings);
        let mut shared = Shared::of(&mut self.combat, &mut self.rotations);
        self.freecam.tick(input, options, &mut shared)
    }
    /// The movement writer's requests while the camera is free.
    pub fn freecam_requests(&self) -> Vec<super::movement_input::Request> {
        self.freecam.requests()
    }
    /// The free camera this frame, if any.
    pub fn freecam_camera(&self) -> Option<Camera> {
        self.freecam.camera()
    }
    /// A view turn; true when the camera took it.
    pub fn freecam_turn(&mut self, xo: f64, yo: f64) -> bool {
        self.freecam.turn(xo, yo)
    }
    /// The player's body is drawn in first person.
    pub fn freecam_body(&self) -> bool {
        self.freecam.shows_body(Options::read(&self.settings))
    }
    /// The view bob is off.
    pub fn freecam_no_bob(&self) -> bool {
        self.freecam
            .suppresses_bobbing(Options::read(&self.settings))
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::modules::combat::rotation::Claimant;
    use super::super::super::modules::render::freecam::{Arrows, Keys};
    use super::super::lifecycle::Context;
    use super::*;

    fn context(in_world: bool) -> Context {
        Context {
            in_world,
            alive: true,
            interactive: true,
        }
    }
    fn in_world() -> Runtime {
        let mut runtime = Runtime::default();
        runtime.observe_context(context(true));
        runtime
    }
    fn input() -> Input {
        Input {
            player: true,
            health: 20.0,
            eye: [0.5, 101.62, 0.5],
            view: (0.0, 0.0),
            no_screen: true,
            keys: Keys::default(),
            arrows: Arrows::default(),
        }
    }

    #[test]
    fn toggling_runs_the_controller_and_the_deferred_disable() {
        let mut r = in_world();
        assert!(!r.freecam_wanted());
        r.set_enabled(ModuleId::Freecam, true);
        assert!(r.freecam_wanted());
        assert!(r.rotation().owned_by(Claimant::Freecam));
        r.freecam_tick(&input());
        assert!(r.freecam_camera().is_some());
        assert!(r.freecam_body());
        assert!(!r.freecam_no_bob());
        assert!(r.freecam_turn(100.0, 0.0));
        r.set_enabled(ModuleId::Freecam, false);
        assert!(r.freecam_wanted());
        let out = r.freecam_tick(&input());
        assert_eq!(out.rotation, Some((0.0, 0.0)));
        assert!(out.finished);
        assert!(!r.freecam_wanted());
        assert!(r.freecam_camera().is_none());
        assert!(!r.rotation().owned_by(Claimant::Freecam));
    }

    #[test]
    fn death_disables_the_module() {
        let mut r = in_world();
        r.set_enabled(ModuleId::Freecam, true);
        r.freecam_tick(&input());
        let mut dead = input();
        dead.health = 0.0;
        let out = r.freecam_tick(&dead);
        assert!(out.finished);
        assert!(!r.enabled(ModuleId::Freecam));
        assert!(r.freecam_camera().is_none());
    }

    #[test]
    fn leaving_the_world_disables_it_and_a_dimension_keeps_it() {
        let mut r = in_world();
        r.set_enabled(ModuleId::Freecam, true);
        r.freecam_tick(&input());
        r.session_change(CleanupReason::Dimension);
        assert!(r.enabled(ModuleId::Freecam));
        r.freecam_tick(&input());
        assert!(r.freecam_camera().is_some());
        r.observe_context(context(false));
        assert!(!r.enabled(ModuleId::Freecam));
        assert!(r.freecam_camera().is_none());
        assert!(!r.freecam_wanted());
    }
}
