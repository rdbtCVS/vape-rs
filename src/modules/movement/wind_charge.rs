// WindCharge (reference `WindCharge`, Category.UTILITY): on enable, with a
// wind charge in the hotbar and standing on the ground, it selects the
// charge, turns the pitch down to 90 (the yaw is left alone), throws once the
// turn completes (or the pitch passes 80), jumps the next tick, then restores
// the slot and disables itself. "Silent aim" keeps the camera still: the
// throw carries the managed rotation instead.
use super::super::super::core::settings::{Definition, Settings};
use super::rotation::{Controller, UNSET};
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
    pub sensitivity: f32,
}

/// What the host does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Actions {
    pub select: Option<usize>,
    /// The rotation to use: the camera's own unless silent.
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

#[derive(Debug, Default)]
pub struct WindCharge {
    stage: Option<Stage>,
    previous_slot: usize,
    controller: Option<Controller>,
    managed: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Select,
    Aim,
    Jump,
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
    pub fn tick(&mut self, o: &Options, t: &Tick) -> Actions {
        let mut out = Actions {
            silent: o.silent,
            ..Default::default()
        };
        if self.stage.is_none() {
            // `onEnable`.
            match t.charge_slot {
                Some(slot) if t.on_ground => {
                    self.previous_slot = t.selected;
                    self.stage = Some(Stage::Select);
                    self.managed = t.view;
                    return self.run(o, t, slot, out);
                }
                Some(_) => out.refused = Some("you must be on the ground"),
                None => out.refused = Some("no wind charge in your hotbar"),
            }
            out.disable = true;
            return out;
        }
        let slot = t.charge_slot.unwrap_or(t.selected);
        self.run(o, t, slot, out)
    }

    fn run(&mut self, o: &Options, t: &Tick, slot: usize, mut out: Actions) -> Actions {
        match self.stage.unwrap_or(Stage::Finish) {
            Stage::Select => {
                out.select = Some(slot);
                let mut c = Controller::new([UNSET, 90.0], o.aim_speed as i32 as f32, o.silent);
                c.tolerance = 5.0;
                c.linear_acceleration = true;
                c.scale_axes = false;
                self.controller = Some(c);
                self.stage = Some(Stage::Aim);
                self.aim(o, t, &mut out);
            }
            Stage::Aim => self.aim(o, t, &mut out),
            Stage::Jump => {
                out.jump = true;
                self.stage = Some(Stage::Finish);
            }
            Stage::Finish => {
                out.select = Some(self.previous_slot);
                out.disable = true;
                self.reset();
            }
        }
        out
    }

    fn aim(&mut self, o: &Options, t: &Tick, out: &mut Actions) {
        let Some(c) = self.controller.as_mut() else {
            return;
        };
        let from = if o.silent { self.managed } else { t.view };
        let to = c.tick(from, t.sensitivity);
        self.managed = to;
        out.rotation = Some(to);
        if c.complete() || to[1] > 80.0 {
            out.throw = true;
            self.stage = Some(Stage::Jump);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(view: [f32; 2]) -> Tick {
        Tick {
            charge_slot: Some(4),
            selected: 1,
            on_ground: true,
            view,
            sensitivity: 0.5,
        }
    }

    #[test]
    fn selects_aims_down_throws_jumps_and_restores() {
        let o = Options {
            aim_speed: 7.0,
            silent: true,
        };
        let mut wc = WindCharge::default();
        let first = wc.tick(&o, &tick([30.0, 0.0]));
        assert_eq!(first.select, Some(4));
        let mut thrown = first.throw;
        let mut rotation = first.rotation.unwrap();
        let mut ticks = 1;
        while !thrown {
            let a = wc.tick(&o, &tick([30.0, 0.0]));
            rotation = a.rotation.unwrap();
            thrown = a.throw;
            ticks += 1;
            assert!(ticks < 10, "never threw");
        }
        assert_eq!(rotation[0], 30.0);
        assert!(rotation[1] > 80.0);
        let jump = wc.tick(&o, &tick([30.0, 0.0]));
        assert!(jump.jump && !jump.disable);
        let done = wc.tick(&o, &tick([30.0, 0.0]));
        assert_eq!(done.select, Some(1));
        assert!(done.disable);
    }

    #[test]
    fn refuses_without_a_charge_or_in_the_air() {
        let o = Options {
            aim_speed: 7.0,
            silent: false,
        };
        let mut wc = WindCharge::default();
        let mut t = tick([0.0, 0.0]);
        t.on_ground = false;
        assert!(wc.tick(&o, &t).disable);
        t.charge_slot = None;
        t.on_ground = true;
        let a = wc.tick(&o, &t);
        assert!(a.disable && a.refused.is_some());
    }
}
