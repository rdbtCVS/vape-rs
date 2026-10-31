// Runtime glue for combat part A: TargetFilter's team snapshot,
// HitSelect's click verdicts and BlockHit's modes.
use super::super::modules::combat::{block_hit, hit_select};
use super::super::modules::utility::target_filter;
use super::super::targeting::entities::{EntityView, PlayerView};
use super::settings::Definition;
use super::{ModuleId, Runtime};
use serde_json::json;

#[derive(Debug, Default)]
pub struct State {
    /// The player list and teams of the last client tick.
    teams: target_filter::Snapshot,
    hit_select: hit_select::HitSelect,
    block_hit: block_hit::BlockHit,
}

pub fn definitions() -> Vec<Definition> {
    let mut out = target_filter::definitions();
    out.extend(hit_select::definitions());
    out.extend(block_hit::definitions());
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
    pub(super) fn block_hit_options(&self) -> block_hit::Options {
        block_hit::Options::read(&self.settings)
    }
    /// BlockHit's mode while it is enabled.
    pub fn block_hit_mode(&self) -> Option<block_hit::Mode> {
        self.enabled(ModuleId::BlockHit)
            .then(|| self.block_hit_options().mode)
    }
    /// The runtime's part of BlockHit's view: the targets, the clickers, the
    /// attack timing and the clock.
    fn block_hit_input(
        &self,
        o: &block_hit::Options,
        mut input: block_hit::Input,
        player: &PlayerView,
        entities: &[EntityView],
    ) -> block_hit::Input {
        let people = self.target_people();
        input.target = o.mode == block_hit::Mode::Lag
            && block_hit::find_target(player, entities, &people, o.angle, o.distance);
        input.default_target = o.mode == block_hit::Mode::Predict
            && block_hit::find_target(player, entities, &people, 90.0, 5.0);
        input.clicker = self.enabled(ModuleId::AutoClicker) || self.enabled(ModuleId::SilentAura);
        input.expected_hurt_ticks = self.outbound.expected_hurt_ticks();
        input.average_hit_delay = self.outbound.average_hit_delay();
        input.now_ms = self.now_ms();
        input
    }
    /// BlockHit's `EventPreTick`: the use key change, if any.
    pub fn block_hit_tick(
        &mut self,
        input: block_hit::Input,
        player: &PlayerView,
        entities: &[EntityView],
    ) -> Option<block_hit::Key> {
        self.block_hit_mode()?;
        let o = self.block_hit_options();
        if o.mode != block_hit::Mode::Lag && self.combat_extra.block_hit.claimed() {
            // Lag was left mid-cycle: the host sends what it held.
            self.combat_extra.block_hit.flushed();
        }
        let input = self.block_hit_input(&o, input, player, entities);
        self.combat_extra.block_hit.tick(&o, &input)
    }
    /// BlockHit's `EventMouseButton` for an attack or use press.
    pub fn block_hit_press(&mut self, input: block_hit::Input, attack: bool) -> block_hit::Mouse {
        if self.block_hit_mode().is_none() {
            return block_hit::Mouse::default();
        }
        let o = self.block_hit_options();
        let input = self.block_hit_input(&o, input, &PlayerView::default(), &[]);
        self.combat_extra.block_hit.press(&o, &input, attack)
    }
    /// A clicker asks at its click whether to block too
    /// (`shouldSimulateBlockHit` = `BlockHit.shouldBlock`).
    pub fn block_hit_simulate(&mut self, input: block_hit::Input) -> bool {
        if self.block_hit_mode().is_none() {
            return false;
        }
        let o = self.block_hit_options();
        let input = self.block_hit_input(&o, input, &PlayerView::default(), &[]);
        self.combat_extra.block_hit.should_block(&o, &input)
    }
    /// Predict: the local player took (or blocked) a hit.
    pub fn block_hit_damaged(&mut self) {
        if self.block_hit_mode().is_some() {
            let o = self.block_hit_options();
            let now = self.now_ms();
            self.combat_extra.block_hit.damaged(&o, now);
        }
    }
    /// Lag holds the release-use packet and what follows it.
    pub fn block_hit_buffering(&self) -> bool {
        self.combat_extra.block_hit.buffering()
    }
    /// The host sent what Lag held without a send verdict.
    pub fn block_hit_flushed(&mut self) {
        self.combat_extra.block_hit.flushed();
    }
    /// Lag's `findTarget(angle, distance)`, which ends the buffering when lost.
    pub fn block_hit_target(&self, player: &PlayerView, entities: &[EntityView]) -> bool {
        let o = self.block_hit_options();
        block_hit::find_target(player, entities, &self.target_people(), o.angle, o.distance)
    }
    /// Lag's `onPacketSend`.
    pub fn block_hit_send(
        &mut self,
        release_use: bool,
        target: bool,
        sword: bool,
    ) -> block_hit::Send {
        if self.block_hit_mode().is_none() {
            return block_hit::Send::Pass;
        }
        let o = self.block_hit_options();
        let now = self.now_ms();
        self.combat_extra
            .block_hit
            .send(&o, release_use, target, sword, now)
    }
    /// BlockHit's detailed suffix.
    pub(super) fn block_hit_suffix(&self) -> String {
        self.block_hit_options()
            .suffix(self.combat_extra.block_hit.buffering())
    }
    /// FakeLag's Repel mode yields to Lag's primary-action claim
    /// (`controlClaim.isClaimed()`: flush and hold nothing).
    pub(super) fn fake_lag_yields(&self) -> bool {
        self.combat_extra.block_hit.claimed()
            && self.enabled(ModuleId::FakeLag)
            && self
                .settings
                .get("world.fake-lag.mode")
                .and_then(serde_json::Value::as_str)
                == Some("Repel")
    }
    pub(super) fn combat_extra_disabled(&mut self, module: ModuleId) {
        match module {
            ModuleId::TargetFilter => self.combat_extra.teams = Default::default(),
            ModuleId::HitSelect => self.combat_extra.hit_select = Default::default(),
            // The host sends what Lag held once it stops buffering.
            ModuleId::BlockHit => self.combat_extra.block_hit.flushed(),
            _ => {}
        }
    }
    pub(super) fn combat_extra_reset(&mut self) {
        self.combat_extra.teams = Default::default();
        self.combat_extra.hit_select = Default::default();
        self.combat_extra.block_hit = Default::default();
    }
}
