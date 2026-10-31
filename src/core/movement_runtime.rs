// Runtime state and entry points for the movement/world modules in
// `modules::movement` (Scaffold Legit, InvWalk, WindCharge) and the NoFall
// jump-key detail. Kept apart from core/mod.rs so the shared runtime only
// carries one field and one toggle call for them.
use super::super::actions::{Lease, Owner, Resource};
use super::super::modules::blatant::no_fall;
use super::super::modules::movement::{inv_walk, scaffold, step, wind_charge};
use super::{ModuleId, Runtime};

const WIND_CHARGE_OWNER: Owner = Owner("utility.wind-charge");

#[derive(Debug, Default)]
pub struct State {
    legit: scaffold::Legit,
    wind_charge: wind_charge::WindCharge,
    wind_lease: Option<Lease>,
    /// A slot to restore once WindCharge was turned off mid-throw.
    wind_restore: Option<usize>,
    /// NoFall released the jump key; it stays released until let go.
    jump_released: bool,
    step: step::Step,
    /// Step's 0.45 timer runs until this time.
    step_timer_until: Option<u64>,
}

impl Runtime {
    /// Called by `set_enabled` after the module's flag changed.
    pub(super) fn movement_toggled(&mut self, module: ModuleId, enabled: bool) {
        match module {
            ModuleId::Scaffold if enabled => {
                let options = scaffold::Options::read(&self.settings);
                if options.mode != scaffold::Mode::Legit {
                    self.notify(format!(
                        "Scaffold: the {:?} mode is not ported yet; only Legit works",
                        options.mode
                    ));
                }
                self.world_movement.legit.enable(&options);
            }
            ModuleId::Scaffold => self.world_movement.legit.reset(),
            ModuleId::WindCharge if !enabled => {
                let state = &mut self.world_movement;
                state.wind_restore = state.wind_charge.restore_slot();
                state.wind_charge.reset();
                if let Some(lease) = state.wind_lease.take() {
                    self.actions.release(lease);
                }
            }
            ModuleId::NoFall if !enabled => self.world_movement.jump_released = false,
            ModuleId::Step => self.world_movement.step.reset(),
            _ => {}
        }
    }

    /// Scaffold Legit's sneak for this pre-entity-update.
    pub fn scaffold_sneak(&mut self, tick: &scaffold::Tick) -> Option<scaffold::Sneak> {
        if !self.enabled(ModuleId::Scaffold) || !self.session.in_world() {
            return None;
        }
        let options = scaffold::Options::read(&self.settings);
        if options.mode != scaffold::Mode::Legit {
            return None;
        }
        self.world_movement.legit.tick(&options, tick)
    }

    /// InvWalk's options when it hands the physical keys over behind `screen`.
    pub fn inv_walk(&self, screen: inv_walk::Screen) -> Option<inv_walk::Options> {
        if !self.enabled(ModuleId::InvWalk) || !self.session.in_world() {
            return None;
        }
        let options = inv_walk::Options::read(&self.settings);
        inv_walk::active(&options, screen).then_some(options)
    }

    /// WindCharge's pre-tick; a pending slot restore after a manual disable
    /// comes back as a plain `select`.
    pub fn wind_charge_tick(&mut self, tick: &wind_charge::Tick) -> wind_charge::Actions {
        if let Some(slot) = self.world_movement.wind_restore.take() {
            return wind_charge::Actions {
                select: Some(slot),
                ..Default::default()
            };
        }
        if !self.enabled(ModuleId::WindCharge) || !self.session.in_world() {
            return Default::default();
        }
        let epoch = self.session.epoch();
        let lease = match self.world_movement.wind_lease {
            Some(lease) if self.actions.owns(lease, epoch) => Some(lease),
            _ => self
                .actions
                .acquire(Resource::Hotbar, WIND_CHARGE_OWNER, epoch, 90),
        };
        let Some(lease) = lease else {
            // Another module holds the hotbar this tick.
            return Default::default();
        };
        self.world_movement.wind_lease = Some(lease);
        let options = wind_charge::Options::read(&self.settings);
        let actions = self.world_movement.wind_charge.tick(&options, tick);
        if let Some(reason) = actions.refused {
            self.notify(format!("WindCharge: {reason}"));
        }
        if actions.disable {
            // The final stage already restored the slot.
            self.world_movement.wind_charge.reset();
            self.set_enabled(ModuleId::WindCharge, false);
            self.world_movement.wind_restore = None;
        }
        actions
    }

    /// NoFall Normal's claim this motion packet, with the jump key held: the
    /// reference releases the jump binding, which stays up until re-pressed.
    pub fn no_fall_claimed(&mut self, jump_held: bool) {
        if jump_held
            && self.enabled(ModuleId::NoFall)
            && no_fall::Mode::read(&self.settings) == no_fall::Mode::Normal
        {
            self.world_movement.jump_released = true;
        }
    }

    /// Whether NoFall keeps jump released this tick (`user_jump`: the
    /// user's own jump state).
    pub fn no_fall_jump_released(&mut self, user_jump: bool) -> bool {
        if !user_jump || !self.enabled(ModuleId::NoFall) {
            self.world_movement.jump_released = false;
        }
        self.world_movement.jump_released
    }

    /// Step at the position send: after a recognised step, Speed's stage and
    /// the timer; returns the reference's intermediate positions, which the
    /// host does not send (one position per client tick on 26.3).
    pub fn step_motion(&mut self, sample: step::Sample) -> Vec<[f64; 3]> {
        if !self.enabled(ModuleId::Step) || !self.session.in_world() {
            self.world_movement.step.reset();
            return Vec::new();
        }
        let Some(stepped) = self.world_movement.step.observe(sample) else {
            return Vec::new();
        };
        self.speed.set_stage(-4);
        self.world_movement.step_timer_until = Some(self.now_ms() + step::TIMER_MS);
        stepped.packets
    }

    /// Step's timer override, while it runs.
    pub(super) fn step_timer(&self) -> Option<f32> {
        let until = self.world_movement.step_timer_until?;
        (self.now_ms() < until && self.session.in_world()).then_some(step::TIMER)
    }
}
