// Runtime glue for combat part A: TargetFilter's team snapshot and
// HitSelect's click verdicts.
use super::super::modules::combat::hit_select;
use super::super::modules::utility::target_filter;
use super::settings::Definition;
use super::{ModuleId, Runtime};
use serde_json::json;

#[derive(Debug, Default)]
pub struct State {
    /// The player list and teams of the last client tick.
    teams: target_filter::Snapshot,
    hit_select: hit_select::HitSelect,
}

pub fn definitions() -> Vec<Definition> {
    let mut out = target_filter::definitions();
    out.extend(hit_select::definitions());
    out
}

impl Runtime {
    fn target_filter_options(&self) -> target_filter::Options {
        target_filter::Options::read(&self.settings, self.enabled(ModuleId::TargetFilter))
    }
    /// The host's player list and teams for this tick (`EventPreTick`).
    /// With "Teams by color" on, the detected own colour is shown.
    pub fn target_filter_tick(&mut self, snapshot: target_filter::Snapshot) {
        let options = self.target_filter_options();
        if options.teams_by_color
            && let Some(color) = target_filter::team_color(&snapshot, &snapshot.local)
        {
            let id = format!("{}.detected-color", target_filter::ID);
            // `setPersistenceSuppressed`: shown, not a user edit.
            let _ = self
                .settings
                .set(&id, json!(target_filter::detected_text(color)));
        }
        self.combat_extra.teams = snapshot;
    }
    /// Whether the host needs to build a team snapshot this tick.
    pub fn target_filter_active(&self) -> bool {
        let o = self.target_filter_options();
        o.teams_by_server || o.teams_by_color
    }
    pub(super) fn target_targets(
        &self,
        names: super::super::targeting::filter::NameLists,
    ) -> target_filter::Targets {
        target_filter::Targets::new(
            names,
            &self.target_filter_options(),
            &self.combat_extra.teams,
        )
    }
    pub(super) fn hit_select_options(&self) -> hit_select::Options {
        hit_select::Options::read(&self.settings)
    }
    fn hit_select_on(&self) -> bool {
        self.enabled(ModuleId::HitSelect)
    }
    /// HitSelect's `EventPreTick`.
    pub fn hit_select_tick(&mut self, view: &hit_select::View) {
        if self.hit_select_on() {
            let o = self.hit_select_options();
            let now = self.now_ms();
            self.combat_extra.hit_select.pre_tick(&o, view, now);
        }
    }
    /// HitSelect's `EventClickMouse`: an attack click about to start.
    pub fn hit_select_click(&mut self, view: &hit_select::View) -> hit_select::Click {
        if !self.hit_select_on() {
            return hit_select::Click::default();
        }
        let o = self.hit_select_options();
        let timing = hit_select::Timing {
            expected_hurt_ticks: self.outbound.expected_hurt_ticks(),
            average_hit_delay: self.outbound.average_hit_delay(),
            now_ms: self.now_ms(),
        };
        self.combat_extra.hit_select.click(&o, view, &timing)
    }
    /// An explosion or a velocity packet for the local player arrived.
    pub fn hit_select_velocity(&mut self) {
        if self.hit_select_on() {
            let o = self.hit_select_options();
            self.combat_extra.hit_select.velocity(&o);
        }
    }
    /// HitSelect holds back use, right clicks and block hitting.
    pub fn hit_select_cancel_use(&self) -> bool {
        self.hit_select_on()
            && self
                .combat_extra
                .hit_select
                .cancel_use(&self.hit_select_options())
    }
    /// LeftClicker's `isHitSelectActive`.
    pub fn hit_select_defers(&self) -> bool {
        self.hit_select_on()
            && self
                .combat_extra
                .hit_select
                .right_click_cancel_active(&self.hit_select_options())
    }
    pub(super) fn combat_extra_disabled(&mut self, module: ModuleId) {
        match module {
            ModuleId::TargetFilter => self.combat_extra.teams = Default::default(),
            ModuleId::HitSelect => self.combat_extra.hit_select = Default::default(),
            _ => {}
        }
    }
    pub(super) fn combat_extra_reset(&mut self) {
        self.combat_extra.teams = Default::default();
        self.combat_extra.hit_select = Default::default();
    }
}
