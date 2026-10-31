// Runtime side of the combat-worker modules: the shared rotation claim and
// managed rotation, KillAura, SilentAura (with its clicker), HitFlick and
// KeepSprint.
use super::super::modules::combat::{
    hit_flick::{self, HitFlick},
    keep_sprint::{self, KeepSprint},
    kill_aura::{self, KillAura},
    reach,
    rotation::{Claim, Managed, UPDATES_PER_TICK},
    silent_aura::{self, Click, SilentAura},
};
use super::settings::Definition;
use super::{ModuleId, Runtime};

#[derive(Debug, Default)]
pub struct Combat {
    pub(super) claim: Claim,
    pub(super) managed: Managed,
    kill_aura: KillAura,
    silent_aura: SilentAura,
    keep_sprint: KeepSprint,
    hit_flick: HitFlick,
}

pub fn definitions() -> Vec<Definition> {
    kill_aura::definitions()
        .into_iter()
        .chain(silent_aura::definitions())
        .chain(keep_sprint::definitions())
        .chain(hit_flick::definitions())
        .collect()
}

/// One SilentAura tick's outcome for the host.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotation {
    pub click: Click,
    /// The managed rotation to send this tick instead of the view.
    pub sent: Option<(f32, f32)>,
    /// HitFlick's synthetic attack and packet flush.
    pub flick: hit_flick::Effects,
    /// HitFlick's forced forward and sprint keys changed.
    pub keys_changed: bool,
}

/// HitFlick's host inputs beyond SilentAura's tick.
#[derive(Clone, Debug, Default)]
pub struct Flick {
    /// The physical strafe keys (left, right).
    pub strafe: (bool, bool),
    /// Every snapshot entity's hurt time.
    pub hurt: Vec<(i32, i32)>,
    /// Reach's view of the player, for the managed-rotation ray.
    pub reach: reach::Player,
    pub creative: bool,
}

impl Runtime {
    /// The module was disabled.
    pub(super) fn combat_disabled(&mut self, module: ModuleId) {
        let c = &mut self.combat;
        match module {
            ModuleId::KillAura => c.kill_aura.reset(),
            ModuleId::KeepSprint => c.keep_sprint.reset(),
            // The managed rotation returns to the view before the claim is
            // released (the reference's deferred toggle-off).
            ModuleId::SilentAura => c.silent_aura.reset(&mut c.claim, &mut c.managed),
            ModuleId::HitFlick => {
                c.hit_flick.reset(&mut c.claim, &mut c.managed);
            }
            _ => {}
        }
    }
    pub(super) fn combat_setting_changed(&mut self, id: &str) {
        let c = &mut self.combat;
        if id.starts_with("combat.kill-aura.") {
            c.kill_aura.reset();
        }
        if id.starts_with("combat.keep-sprint.") {
            c.keep_sprint.reset();
        }
        if id.starts_with("combat.silent-aura.") {
            c.silent_aura.reset(&mut c.claim, &mut c.managed);
        }
        if id.starts_with("combat.hit-flick.") {
            c.hit_flick.reset(&mut c.claim, &mut c.managed);
        }
    }
    /// Session cleanup: nothing survives a world change, including the
    /// managed rotation.
    pub(super) fn combat_session_reset(&mut self) {
        let c = &mut self.combat;
        c.kill_aura.reset();
        c.keep_sprint.reset();
        c.silent_aura.reset(&mut c.claim, &mut c.managed);
        c.hit_flick.reset(&mut c.claim, &mut c.managed);
        c.hit_flick.yield_controller();
        c.managed.clear();
        c.claim.clear();
    }

    pub fn kill_aura_active(&self) -> bool {
        self.enabled(ModuleId::KillAura) && self.session().in_world()
    }
    pub fn kill_aura_tick(&mut self, tick: &kill_aura::Tick) -> kill_aura::Effects {
        let options = kill_aura::Options::read(&self.settings);
        let people = self.target_people();
        let effects = self.combat.kill_aura.tick(&options, tick, &people);
        if effects.disable {
            self.set_enabled(ModuleId::KillAura, false);
        }
        effects
    }

    pub fn silent_aura_wanted(&self) -> bool {
        ((self.enabled(ModuleId::SilentAura) || self.enabled(ModuleId::HitFlick))
            && self.session().in_world())
            || self.combat.managed.active
    }
    /// One tick: SilentAura's targeting and aim (while enabled), HitFlick's
    /// phases (its lowest-priority pre-tick), the managed rotation's 50
    /// updates driven by whichever controller is active, then the clicker.
    pub fn silent_aura_tick(&mut self, tick: &silent_aura::Tick, flick: &Flick) -> Rotation {
        let options = silent_aura::Options::read(&self.settings);
        let flick_options = hit_flick::Options::read(&self.settings);
        let people = self.target_people();
        let in_world = self.session().in_world();
        let enabled = self.enabled(ModuleId::SilentAura) && in_world;
        let flicking = self.enabled(ModuleId::HitFlick) && in_world;
        let c = &mut self.combat;
        let mut disable = false;
        if enabled {
            disable = c
                .silent_aura
                .tick(&options, tick, &people, &mut c.claim, &mut c.managed)
                .disable;
        }
        // SilentAura's controller replaced HitFlick's way back to the view.
        if c.silent_aura.controlling() {
            c.hit_flick.yield_controller();
        }
        let view = flick_view(tick, flick);
        let keys = c.hit_flick.forcing_keys();
        let mut effects = hit_flick::Effects::default();
        if flicking {
            effects =
                c.hit_flick
                    .tick(&flick_options, &view, &people, &mut c.claim, &mut c.managed);
        }
        let distance = c.silent_aura.distance_to_target(tick);
        let aura = &mut c.silent_aura;
        let hit_flick = &c.hit_flick;
        let player = (tick.player.yaw, tick.player.pitch);
        let returned = if hit_flick.controlling() {
            c.managed
                .tick_by(player, tick.sensitivity, UPDATES_PER_TICK, |m| {
                    hit_flick.step(&view, m)
                })
        } else {
            c.managed
                .tick(player, tick.sensitivity, UPDATES_PER_TICK, || {
                    aura.step(&options, distance)
                })
        };
        if returned {
            c.silent_aura.returned(&mut c.claim);
            c.hit_flick.yield_controller();
        }
        let click = if enabled && !disable && !c.hit_flick.controlling() {
            // Vanilla's entity interaction range for the click's ray.
            c.silent_aura
                .click(&options, tick, &c.managed, silent_aura::BASE_RANGE)
        } else {
            Click::None
        };
        let sent = c.managed.sent();
        let keys_changed = keys != c.hit_flick.forcing_keys();
        if disable {
            self.set_enabled(ModuleId::SilentAura, false);
        }
        Rotation {
            click,
            sent,
            flick: effects,
            keys_changed,
        }
    }

    fn hit_flick_on(&self) -> bool {
        self.enabled(ModuleId::HitFlick) && self.session().in_world()
    }
    fn hit_flick_start(&mut self, view: &hit_flick::View<'_>, target: i32) -> bool {
        let options = hit_flick::Options::read(&self.settings);
        let people = self.target_people();
        let start = hit_flick::Start {
            hurt_time: view
                .hurt
                .iter()
                .find(|(id, _)| *id == target)
                .map_or(0, |(_, hurt)| *hurt),
            expected_hurt_ticks: self.outbound.expected_hurt_ticks(),
            primary_claimed: self.block_hit_claimed(),
        };
        let c = &mut self.combat;
        let started = c.hit_flick.start(
            &options,
            view,
            target,
            start,
            &people,
            &mut c.claim,
            &mut c.managed,
        );
        if started && c.silent_aura.controlling() {
            c.silent_aura.yield_controller();
        }
        started
    }
    /// HitFlick's `handleAttackInput` and `interceptAttackRequest` (the
    /// physical and synthetic attack cancellation adapters): true cancels
    /// the attack. A flick in progress cancels every attack; otherwise one
    /// whose managed-rotation ray (Reach's range, HitBoxes' expansion) hits
    /// a living entity starts a flick instead.
    pub fn hit_flick_intercept(&mut self, tick: &silent_aura::Tick, flick: &Flick) -> bool {
        if !self.hit_flick_on() {
            return false;
        }
        if self.combat.hit_flick.mode() != hit_flick::Mode::Idle {
            return true;
        }
        let options = hit_flick::Options::read(&self.settings);
        let view = flick_view(tick, flick);
        if !self.combat.hit_flick.can_start(&options, &view) {
            return false;
        }
        let (reach, expand) = self
            .pick_range(flick.reach)
            .unwrap_or((if flick.creative { 5.0 } else { reach::BASE }, 0.0));
        let rotation = self
            .combat
            .managed
            .sent()
            .unwrap_or((tick.player.yaw, tick.player.pitch));
        match hit_flick::ray(&view, rotation, reach, expand) {
            Some(target) => self.hit_flick_start(&view, target),
            None => false,
        }
    }
    /// HitFlick's `onClickMouse`: SilentAura's click on its own target
    /// starts a flick instead (true cancels the click).
    pub fn hit_flick_click(
        &mut self,
        tick: &silent_aura::Tick,
        flick: &Flick,
        target: i32,
    ) -> bool {
        if !self.hit_flick_on()
            || self.combat.hit_flick.mode() != hit_flick::Mode::Idle
            || self.combat.silent_aura.target() != Some(target)
        {
            return false;
        }
        let view = flick_view(tick, flick);
        self.hit_flick_start(&view, target)
    }
    /// HitFlick's Blink holds outgoing packets.
    pub fn hit_flick_blinking(&self) -> bool {
        self.combat.hit_flick.blinking()
    }
    /// The held packets went out with the attack.
    pub fn hit_flick_flushed(&mut self) {
        self.combat.hit_flick.flushed();
    }
    /// `EventPrePlayerTick`: drop sprinting once after a flick started.
    pub fn hit_flick_cancel_sprint(&mut self) -> bool {
        self.combat.hit_flick.take_cancel_sprint()
    }
    /// The forward and sprint keys HitFlick forces while attacking.
    pub fn hit_flick_keys(&self) -> bool {
        self.hit_flick_on() && self.combat.hit_flick.forcing_keys()
    }
    pub(super) fn hit_flick_claimed(&self) -> bool {
        self.combat.hit_flick.claim_held()
    }
    pub(super) fn hit_flick_suffix(&self) -> String {
        hit_flick::Options::read(&self.settings)
            .suffix(self.combat.hit_flick.mode() != hit_flick::Mode::Idle)
    }

    pub fn keep_sprint_hit(&mut self, hit: keep_sprint::Hit) -> Option<keep_sprint::Keep> {
        if !self.enabled(ModuleId::KeepSprint) {
            return None;
        }
        let options = keep_sprint::Options::read(&self.settings);
        self.combat.keep_sprint.post_attack(&options, hit)
    }
}

fn flick_view<'a>(tick: &'a silent_aura::Tick, flick: &'a Flick) -> hit_flick::View<'a> {
    hit_flick::View {
        now_ms: tick.now_ms,
        player: &tick.player,
        eye_height: tick.eye_height,
        entities: &tick.entities,
        no_screen: tick.no_screen,
        held: &tick.held,
        strafe: flick.strafe,
        hurt: &flick.hurt,
    }
}
