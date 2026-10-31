// Runtime side of AntiFireball: its state next to the shared rotation claim
// and SilentAura's managed rotation (the silent controller's start and the
// claim's returning case).
use super::super::modules::combat::anti_fireball::{self, Effects, Tick};
use super::{ModuleId, Runtime};

/// A block clip from one point to another: where the segment hits a block.
pub type Clip<'a> = dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]> + 'a;

impl Runtime {
    pub fn anti_fireball_wanted(&self) -> bool {
        self.enabled(ModuleId::AntiFireball) && self.session().in_world()
    }

    /// `onTick` and the rotation manager's updates for its controller.
    pub fn anti_fireball_tick(&mut self, tick: &mut Tick, clip: &mut Clip) -> Effects {
        let options = anti_fireball::Options::read(&self.settings);
        let managed = &self.combat.managed;
        tick.managed = if managed.active { managed.sent() } else { None };
        tick.managed_returning = managed.relative;
        self.anti_fireball
            .tick(&options, tick, &mut self.combat.claim, clip)
    }

    /// `setEnabled`: a disable while the silent controller runs only flips
    /// `toggledOff`. True when the toggle was taken here.
    pub(super) fn anti_fireball_set_enabled(&mut self, enabled: bool) -> bool {
        self.anti_fireball.defer_disable(enabled)
    }

    /// A real disable releases the target, movement, rotation and claim.
    pub(super) fn anti_fireball_disabled(&mut self, module: ModuleId) {
        if module == ModuleId::AntiFireball {
            self.anti_fireball.reset(&mut self.combat.claim);
        }
    }
    pub(super) fn anti_fireball_session_reset(&mut self) {
        self.anti_fireball.reset(&mut self.combat.claim);
    }

    /// Sprint's `movementLocked`.
    pub fn anti_fireball_locked(&self) -> bool {
        self.anti_fireball.movement_locked()
    }
    /// The forward, back, left and right keys held up, given the user's.
    pub fn anti_fireball_held_up(&mut self, user: [bool; 4]) -> [bool; 4] {
        self.anti_fireball.held_up(user)
    }
    /// The rotation the crosshair is evaluated at: the installed silent
    /// controller's, else SilentAura's managed one, else the view.
    pub fn anti_fireball_aim(&self, view: (f32, f32)) -> (f32, f32) {
        if let Some(rotation) = self
            .anti_fireball
            .controller()
            .and_then(|c| c.rendered(view))
        {
            return rotation;
        }
        let managed = &self.combat.managed;
        if managed.active {
            managed.sent().unwrap_or(view)
        } else {
            view
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::modules::combat::anti_fireball::{Aabb, Fireball, Me};
    use super::super::lifecycle::Context;
    use super::*;

    #[test]
    fn deferred_disable_keeps_the_module_on_until_the_rotation_returns() {
        let mut runtime = Runtime::default();
        runtime.observe_context(Context {
            in_world: true,
            alive: true,
            interactive: true,
        });
        runtime
            .settings
            .set("combat.anti-fireball.silent-aim", serde_json::json!(true))
            .unwrap();
        runtime.set_enabled(ModuleId::AntiFireball, true);
        assert!(runtime.anti_fireball_wanted());
        let mut tick = Tick {
            me: Me {
                id: 1,
                position: [0.0, 64.0, 0.0],
                previous: [0.0, 64.0, 0.0],
                eye_height: 1.62,
                bounds: Aabb {
                    min: [-0.3, 64.0, -0.3],
                    max: [0.3, 65.8, 0.3],
                },
                yaw: 15.0,
                pitch: 0.0,
            },
            fireballs: vec![Fireball {
                id: 4,
                position: [0.0, 64.6, 6.0],
                previous: [0.0, 64.6, 6.3],
                velocity: [0.0, 0.0, -0.3],
                acceleration: 0.1,
                age: 10,
                width: 1.0,
                height: 1.0,
            }],
            focused: true,
            sensitivity: 0.5,
            ..Tick::default()
        };
        let fx = runtime.anti_fireball_tick(&mut tick, &mut |_, _| None);
        let sent = fx.sent.flatten().unwrap();
        assert_eq!(runtime.anti_fireball_aim((15.0, 0.0)), sent);
        runtime.set_enabled(ModuleId::AntiFireball, false);
        assert!(runtime.enabled(ModuleId::AntiFireball));
        for _ in 0..40 {
            runtime.anti_fireball_tick(&mut tick, &mut |_, _| None);
        }
        assert_eq!(runtime.anti_fireball_aim((15.0, 0.0)), (15.0, 0.0));
        runtime.set_enabled(ModuleId::AntiFireball, false);
        assert!(!runtime.enabled(ModuleId::AntiFireball));
        assert_eq!(runtime.combat.claim.owner(), None);
    }
}
