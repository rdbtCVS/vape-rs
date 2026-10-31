// WindCharge (reference `WindCharge`, Category.UTILITY): on enable, with a
// wind charge in the hotbar and standing on the ground, it selects the
// charge, claims the shared rotation (priority 6) and turns the pitch down
// to 90 (the yaw is left alone), throws once the turn completes (or the
// managed pitch passes 80), jumps the next tick, then releases the rotation,
// restores the slot and disables itself. "Silent aim" uses the silent
// managed rotation (and may preempt a lower owner); otherwise a visible
// controller turns the camera.
use super::super::super::core::rotation_runtime::{Shared, Silent};
use super::super::super::core::settings::{Definition, Settings};
use super::super::combat::rotation::{Claimant, Fixed, Step, UNSET};
use serde_json::Value;

pub const ID: &str = "utility.wind-charge";

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::number(&format!("{ID}.aim-speed"), "Aim speed", 1.0, 7.0, 10.0, 1.0),
        Definition::boolean(&format!("{ID}.silent-aim"), "Silent aim", true),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub aim_speed: f64,
    pub silent: bool,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        Self {
            aim_speed: settings
                .get(&format!("{ID}.aim-speed"))
                .and_then(Value::as_f64)
                .unwrap_or(7.0),
            silent: settings
                .get(&format!("{ID}.silent-aim"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }
}

/// Host state for one pre-tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tick {
    /// The first hotbar slot holding a wind charge.
    pub charge_slot: Option<usize>,
    pub selected: usize,
    pub on_ground: bool,
    /// The camera rotation `[yaw, pitch]`.
    pub view: [f32; 2],
}

/// What the host does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Actions {
    pub select: Option<usize>,
    /// The throw's rotation (`getManagedYaw/Pitch`).
    pub rotation: Option<[f32; 2]>,
    pub silent: bool,
    /// Use the held item once, at `rotation`.
    pub throw: bool,
    pub jump: bool,
    /// The module turns itself off.
    pub disable: bool,
    /// Why it disabled at enable time.
    pub refused: Option<&'static str>,
}

const WHO: Claimant = Claimant::WindCharge;

#[derive(Debug, Default)]
pub struct WindCharge {
    stage: Option<Stage>,
    previous_slot: usize,
    /// `rotationController != null`.
    controller: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// State 0: select the charge and claim the rotation.
    Select,
    /// State 1: wait for the turn.
    Aim,
    /// State 2: jump.
    Jump,
    /// State -1: release, restore, disable.
    Finish,
}

impl WindCharge {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// `onDisable`'s slot restore, when a throw was in progress.
    pub fn restore_slot(&self) -> Option<usize> {
        self.stage.map(|_| self.previous_slot)
    }
    /// `releaseRotation`, from `onDisable` too.
    pub fn release(&mut self, rotation: &mut Shared) {
        if self.controller {
            rotation.release(WHO);
        }
        self.controller = false;
    }
    pub fn tick(&mut self, o: &Options, t: &Tick, rotation: &mut Shared) -> Actions {
        let mut out = Actions {
            silent: o.silent,
            ..Default::default()
        };
        if self.stage.is_none() {
            // `onEnable`.
            match t.charge_slot {
                Some(_) if t.on_ground => {
                    self.previous_slot = t.selected;
                    self.stage = Some(Stage::Select);
                }
                Some(_) => out.refused = Some("you must be on the ground"),
                None => out.refused = Some("no wind charge in your hotbar"),
            }
            if self.stage.is_none() {
                out.disable = true;
                return out;
            }
        }
        let view = (t.view[0], t.view[1]);
        match self.stage.unwrap_or(Stage::Finish) {
            Stage::Select => {
                let Some(slot) = t.charge_slot.filter(|_| t.on_ground) else {
                    return out;
                };
                out.select = Some(slot);
                if !rotation.claim(WHO, o.silent) {
                    return out;
                }
                let speed = o.aim_speed as i32;
                if o.silent {
                    // `AdaptiveRotationController(-999, 90)`.
                    rotation.silent(
                        WHO,
                        view,
                        Silent {
                            target: (UNSET, 90.0),
                            step: Step {
                                counts: f64::from(speed) * 0.25,
                                clamp: false,
                                proportional: false,
                                linear: true,
                            },
                            tolerance: 5.0,
                        },
                    );
                } else {
                    let mut fixed = Fixed::new([UNSET, 90.0], speed as f32);
                    fixed.scale_axes = false;
                    fixed.linear_acceleration = true;
                    fixed.tolerance = 5.0;
                    rotation.visible(WHO, fixed);
                }
                self.controller = true;
                self.stage = Some(Stage::Aim);
            }
            Stage::Aim if !self.controller => self.stage = Some(Stage::Finish),
            Stage::Aim => {
                let managed = rotation.managed(view);
                if rotation.complete(WHO) || managed.1 > 80.0 {
                    out.throw = true;
                    out.rotation = Some([managed.0, managed.1]);
                    self.stage = Some(Stage::Jump);
                }
            }
            Stage::Jump => {
                out.jump = true;
                self.stage = Some(Stage::Finish);
            }
            Stage::Finish => {
                self.release(rotation);
                out.select = Some(self.previous_slot);
                out.disable = true;
                self.reset();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::Runtime;
    use super::*;

    fn tick(view: [f32; 2]) -> Tick {
        Tick {
            charge_slot: Some(4),
            selected: 1,
            on_ground: true,
            view,
        }
    }

    fn run(wc: &mut WindCharge, o: &Options, r: &mut Runtime, view: [f32; 2]) -> Actions {
        // The rotation manager ticks before the module (client tick HEAD).
        let turned = r.rotation_tick((view[0], view[1]), 0.5, true);
        let mut t = tick(turned.unwrap_or(view));
        t.view = turned.unwrap_or(view);
        wc.tick(o, &t, &mut r.rotation())
    }

    #[test]
    fn selects_aims_down_throws_jumps_and_restores() {
        for silent in [true, false] {
            let o = Options {
                aim_speed: 7.0,
                silent,
            };
            let mut r = Runtime::default();
            let mut wc = WindCharge::default();
            let mut view = [30.0, 0.0];
            let first = run(&mut wc, &o, &mut r, view);
            assert_eq!(first.select, Some(4));
            assert!(r.rotation().owned_by(WHO));
            let mut ticks = 1;
            let thrown = loop {
                if let Some(turned) = r.rotation_tick((view[0], view[1]), 0.5, true) {
                    view = turned;
                }
                let a = wc.tick(&o, &tick(view), &mut r.rotation());
                ticks += 1;
                assert!(ticks < 10, "never threw");
                if a.throw {
                    break a;
                }
            };
            let rotation = thrown.rotation.unwrap();
            assert_eq!(rotation[0], 30.0);
            assert!(rotation[1] > 80.0);
            // Silent aim leaves the camera; visible aim turned it.
            assert_eq!(view[1] > 80.0, !silent);
            let jump = run(&mut wc, &o, &mut r, view);
            assert!(jump.jump && !jump.disable);
            let done = run(&mut wc, &o, &mut r, view);
            assert_eq!(done.select, Some(1));
            assert!(done.disable);
            assert_eq!(r.rotation().claim.owner(), None);
        }
    }

    #[test]
    fn waits_for_a_higher_owner_and_preempts_only_when_silent() {
        let mut r = Runtime::default();
        assert!(r.rotation().claim(Claimant::Mlg, false));
        let mut wc = WindCharge::default();
        let o = Options {
            aim_speed: 7.0,
            silent: true,
        };
        // Mlg outranks WindCharge: it keeps selecting and waits.
        for _ in 0..3 {
            let a = run(&mut wc, &o, &mut r, [0.0, 0.0]);
            assert_eq!(a.select, Some(4));
            assert!(!a.throw && !a.disable);
        }
        r.rotation().release(Claimant::Mlg);
        let a = run(&mut wc, &o, &mut r, [0.0, 0.0]);
        assert_eq!(a.select, Some(4));
        assert!(r.rotation().owned_by(WHO));
        // SilentAura (5) cannot take it back.
        assert!(!r.rotation().claim(Claimant::SilentAura, true));
    }

    #[test]
    fn refuses_without_a_charge_or_in_the_air() {
        let o = Options {
            aim_speed: 7.0,
            silent: false,
        };
        let mut r = Runtime::default();
        let mut wc = WindCharge::default();
        let mut t = tick([0.0, 0.0]);
        t.on_ground = false;
        assert!(wc.tick(&o, &t, &mut r.rotation()).disable);
        t.charge_slot = None;
        t.on_ground = true;
        let a = wc.tick(&o, &t, &mut r.rotation());
        assert!(a.disable && a.refused.is_some());
    }
}
