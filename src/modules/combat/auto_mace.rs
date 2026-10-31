// AutoMace (reference `AutoMace`): an attack (physical, or another module's
// synthetic attack) on the crosshair target switches to the best hotbar mace
// for the hit and the original slot comes back two ticks later. Manual takes
// the first mace of the chosen Density/Breach type (any mace when the hotbar
// has none); Auto takes the mace with the highest estimated damage for the
// fall distance and the target's armor. Stun slam hits a shielding player
// with an axe first and follows with the mace. Attack presses attack while
// falling onto a target. Aim (with `AutoMaceRotationController`) and the
// Elytra swaps simulate a copy of the local player's fall, which the host
// cannot do without side effects: their settings exist but do nothing.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit;
use super::super::super::targeting::filter::{self, TargetFilter};
use super::super::rng::Rng;
use super::trigger_bot::Cooldown;
use super::weapon_swap::{Crosshair, Op, Slot, Source, View, select};
use serde_json::{Value, json};

pub const ID: &str = "combat.auto-mace";

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let describe = |mut d: Definition, text: &str| {
        d.description = text.into();
        d
    };
    let mut out = filter::definitions(ID);
    out.extend([
        describe(
            Definition::boolean(&key("aim"), "Aim", false),
            "Aims at the nearest valid target while falling for a smash attack",
        ),
        describe(
            Definition::boolean(&key("silent-aim"), "Silent aim", false),
            "Uses Silent Aim system",
        )
        .when(&key("aim"), json!(true)),
        describe(
            Definition::number(&key("aim-range"), "Aim range", 2.0, 6.0, 10.0, 0.1),
            "Maximum horizontal distance to search for targets",
        )
        .when(&key("aim"), json!(true)),
        describe(
            Definition::boolean(&key("attack"), "Attack", false),
            "Automatically attacks valid mace targets",
        ),
        describe(
            Definition::new(
                &key("extra-delay"),
                "Extra delay",
                Kind::Range {
                    min: -20.0,
                    max: 20.0,
                    step: 0.1,
                },
                json!([0.0, 0.0]),
            ),
            "Extra delay after attack cooldown(in ticks)\nNegative values will attack before cooldown is complete",
        )
        .when(&key("attack"), json!(true)),
        describe(
            Definition::boolean(&key("auto-unequip-elytra"), "Auto unequip Elytra", false),
            "Equips a hotbar chestplate when your predicted fall can reach a mace target",
        ),
        describe(
            Definition::boolean(&key("re-equip-elytra"), "Re-equip Elytra", false),
            "Puts the Elytra back on after upward mace bounce movement is detected",
        )
        .when(&key("auto-unequip-elytra"), json!(true)),
        describe(
            Definition::boolean(&key("smash-only"), "Smash only", true),
            "Only swap to mace if will smash",
        ),
        describe(
            Definition::choice(
                &key("mace-selection"),
                "Mace selection",
                &["Manual", "Auto"],
                "Manual",
            ),
            "Manual uses Mace type. Auto chooses the best mace enchantment for your fall distance and target armor.",
        ),
        describe(
            Definition::choice(&key("mace-type"), "Mace type", &["Density", "Breach"], "Density"),
            "Selects which mace enchantment AutoMace should use. Bind this setting to cycle it in game.",
        )
        .when(&key("mace-selection"), json!("Manual")),
        describe(
            Definition::boolean(&key("stun-slam"), "Stun slam", false),
            "When holding an axe and attacking a shielded player:\nHits with axe first (breaks shield), then swaps to mace for a follow-up slam",
        ),
        describe(
            Definition::number(&key("chance"), "Chance", 0.0, 100.0, 100.0, 1.0),
            "Chance that Stun slam will trigger",
        )
        .when(&key("stun-slam"), json!(true)),
        Definition::boolean(&key("limit-items"), "Limit to items", false),
        Definition::new(
            &key("allowed-items"),
            "Allowed Items",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!(["swords"]),
        )
        .when(&key("limit-items"), json!(true)),
    ]);
    out
}

/// Mace selection, with Manual's Mace type.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Selection {
    #[default]
    Density,
    Breach,
    Auto,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub filter: TargetFilter,
    pub attack: bool,
    pub extra_delay: (f64, f64),
    pub smash_only: bool,
    pub selection: Selection,
    pub stun_slam: bool,
    pub chance: f64,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
}
impl Options {
    /// Booleans are effective values; the modes, Chance and Extra delay are
    /// raw (`getValue`).
    pub fn read(settings: &Settings) -> Self {
        let id = |k: &str| format!("{ID}.{k}");
        let boolean = |k: &str| {
            settings
                .effective(&id(k))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };
        let text = |k: &str| settings.get(&id(k)).and_then(Value::as_str).unwrap_or("");
        let range = settings.get(&id("extra-delay")).and_then(Value::as_array);
        let at = |i: usize| {
            range
                .and_then(|v| v.get(i))
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
        };
        let selection = match (text("mace-selection"), text("mace-type")) {
            ("Auto", _) => Selection::Auto,
            (_, "Breach") => Selection::Breach,
            _ => Selection::Density,
        };
        Self {
            filter: TargetFilter::read(settings, ID),
            attack: boolean("attack"),
            extra_delay: (at(0), at(1)),
            smash_only: boolean("smash-only"),
            selection,
            stun_slam: boolean("stun-slam"),
            chance: settings
                .get(&id("chance"))
                .and_then(Value::as_f64)
                .unwrap_or(100.0),
            limit_items: boolean("limit-items"),
            allowed_items: settings
                .get(&id("allowed-items"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        }
    }

    /// `getSimpleSuffix`: "Auto", or the Mace type.
    pub fn suffix(&self) -> &'static str {
        match self.selection {
            Selection::Auto => "Auto",
            Selection::Density => "Density",
            Selection::Breach => "Breach",
        }
    }

    /// `canOperate`.
    fn can_operate(&self, view: &View) -> bool {
        !view.screen_open
            && (!self.limit_items || item_limit::matches(&self.allowed_items, &view.held().item))
    }
    /// `isConfiguredMace`.
    fn configured(&self, slot: &Slot) -> bool {
        slot.mace()
            && match self.selection {
                Selection::Auto => slot.density > 0 || slot.breach > 0,
                Selection::Density => slot.density > 0,
                Selection::Breach => slot.breach > 0,
            }
    }
    /// `hasConfiguredMaceInHotbar`.
    fn filter_by_enchantment(&self, view: &View) -> bool {
        view.hotbar.iter().take(9).any(|s| self.configured(s))
    }
    /// `scoreMace`: below zero is no use.
    fn score(&self, view: &View, slot: &Slot, filter: bool, target: Option<&Crosshair>) -> f64 {
        if self.smash_only && !view.will_smash || !slot.mace() {
            return -1.0;
        }
        if !filter {
            return 0.0;
        }
        match self.selection {
            Selection::Auto => automatic(view, slot.density, slot.breach, target),
            Selection::Density if slot.density > 0 => 0.0,
            Selection::Breach if slot.breach > 0 => 0.0,
            _ => -1.0,
        }
    }
    /// `findBestMaceSlot`: Auto takes the highest score, Manual the first.
    fn best_mace(
        &self,
        view: &View,
        target: Option<&Crosshair>,
        exclude_selected: bool,
    ) -> Option<usize> {
        let filter = self.filter_by_enchantment(view);
        let mut best = None;
        let mut best_score = -1.0;
        for (slot, stack) in view.hotbar.iter().enumerate().take(9) {
            if exclude_selected && slot == view.selected {
                continue;
            }
            let score = self.score(view, stack, filter, target);
            if score < 0.0 || self.selection == Selection::Auto && score <= best_score {
                continue;
            }
            if self.selection != Selection::Auto {
                return Some(slot);
            }
            best = Some(slot);
            best_score = score;
        }
        best
    }
    /// `isHeldMaceReady`.
    fn held_mace_ready(&self, view: &View, target: Option<&Crosshair>) -> bool {
        self.score(view, &view.held(), self.filter_by_enchantment(view), target) >= 0.0
    }
}

/// `scoreAutomaticMace`.
fn automatic(view: &View, density: i32, breach: i32, target: Option<&Crosshair>) -> f64 {
    if view.on_ground {
        return if breach > 0 { 0.0 } else { -1.0 };
    }
    let mut score = -1.0;
    if density > 0 {
        score = estimate(view.fall_distance, target, density, 0);
    }
    if breach > 0 {
        score = f64::max(score, estimate(view.fall_distance, target, 0, breach));
    }
    score
}

/// `estimateMaceDamage`: base damage, the smash bonus and Density, less the
/// target's armor (`CombatRules.getDamageAfterAbsorb`) lowered by Breach.
pub fn estimate(fall: f32, target: Option<&Crosshair>, density: i32, breach: i32) -> f64 {
    let damage = 6.0 + smash_bonus(fall) + 0.5 * f64::from(density) * f64::from(fall);
    let Some(target) = target else {
        return damage;
    };
    let (armor, toughness) = (target.armor, target.toughness);
    let effective = (armor - damage / (2.0 + toughness / 4.0)).clamp(armor * 0.2, 20.0);
    let reduction = (effective / 25.0 - 0.15 * f64::from(breach)).clamp(0.0, 1.0);
    damage * (1.0 - reduction)
}

/// `smashBonusDamage`: 4 per block for 3 blocks, 2 per block to 8, then 1.
fn smash_bonus(fall: f32) -> f64 {
    if fall <= 3.0 {
        4.0 * f64::from(fall)
    } else if fall <= 8.0 {
        12.0 + 2.0 * f64::from(fall - 3.0)
    } else {
        22.0 + f64::from(fall) - 8.0
    }
}

/// `findAxeSlot`: another hotbar slot with an axe.
fn axe_slot(view: &View) -> Option<usize> {
    (0..view.hotbar.len().min(9)).find(|&i| i != view.selected && view.hotbar[i].axe())
}

/// `requestSyntheticAttack(markInProgress)`: true when the press went through.
pub type Request<'a> = dyn FnMut(&mut View, &mut Vec<Op>, bool) -> bool + 'a;

#[derive(Clone, Debug)]
pub struct AutoMace {
    release_pending: bool,
    weapon_swap_active: bool,
    followup_pending: bool,
    original_slot: Option<usize>,
    mace_slot: Option<usize>,
    target_id: Option<i32>,
    original_attack_tick: i32,
    weapon_swap_ticks: i32,
    /// `inputTimer`: when an attack input last went down.
    input_at: Option<u64>,
    random: Rng,
}

impl AutoMace {
    pub fn new(seed: u64) -> Self {
        Self {
            release_pending: false,
            weapon_swap_active: false,
            followup_pending: false,
            original_slot: None,
            mace_slot: None,
            target_id: None,
            original_attack_tick: -1,
            weapon_swap_ticks: 0,
            input_at: None,
            random: Rng::new(seed),
        }
    }
    pub fn active(&self) -> bool {
        self.weapon_swap_active
    }
    /// `onDisable` (also a world change): a pending press is released; the
    /// slot is not restored.
    pub fn reset(&mut self, ops: &mut Vec<Op>) {
        if self.release_pending {
            ops.push(Op::Release);
        }
        let random = self.random.clone();
        *self = Self::new(1);
        self.random = random;
    }

    /// `canHandleMaceAttack`: Stun slam can take a shielded player.
    pub fn can_handle_mace_attack(options: &Options, view: &View) -> bool {
        options.stun_slam && (view.held().axe() || axe_slot(view).is_some())
    }
    /// `hasReadyMace`: another hotbar mace suits the crosshair target.
    pub fn has_ready_mace(options: &Options, view: &View) -> bool {
        options
            .best_mace(view, mace_target(view).as_ref(), true)
            .is_some()
    }

    /// The attack mapping going down; `cancelled` by an earlier handler.
    /// True cancels the press.
    pub fn press(
        &mut self,
        options: &Options,
        view: &mut View,
        ops: &mut Vec<Op>,
        cancelled: bool,
    ) -> bool {
        self.input_at = Some(view.now_ms);
        !cancelled && self.handle_attack(options, view, ops)
    }
    /// `onSyntheticAttack`.
    pub fn synthetic(
        &mut self,
        options: &Options,
        source: Source,
        view: &mut View,
        ops: &mut Vec<Op>,
        cancelled: bool,
    ) -> bool {
        if matches!(
            source,
            Source::AutoMace(_) | Source::HitSwap | Source::ShieldBreaker
        ) {
            return false;
        }
        self.press(options, view, ops, cancelled)
    }
    /// `onPostAttack`: the axe hit of a stun slam landed.
    pub fn post_attack(
        &mut self,
        view: &mut View,
        ops: &mut Vec<Op>,
        target: i32,
        request: &mut Request,
    ) {
        if !self.weapon_swap_active || !self.followup_pending || self.target_id != Some(target) {
            return;
        }
        if !view.held().axe() {
            self.abort(view, ops);
            return;
        }
        self.complete_followup(view, ops, request);
    }

    /// `onTick` (EventPreTick).
    pub fn tick(
        &mut self,
        options: &Options,
        view: &mut View,
        ops: &mut Vec<Op>,
        cooldown: Cooldown,
        request: &mut Request,
    ) {
        let mut released = false;
        if self.release_pending {
            ops.push(Op::Release);
            self.release_pending = false;
            released = true;
        }
        if self.weapon_swap_active && self.followup_pending {
            if view.ticks_existed <= self.original_attack_tick {
                return;
            }
            if view.crosshair.map(|c| c.id) != self.target_id {
                self.abort(view, ops);
                return;
            }
            self.complete_followup(view, ops, request);
            return;
        }
        if self.weapon_swap_active {
            let ticks = self.weapon_swap_ticks;
            self.weapon_swap_ticks += 1;
            if ticks > 1 {
                if let Some(slot) = self.original_slot.take() {
                    select(view, ops, slot);
                }
                self.clear_weapon_swap();
            }
        }
        if !released {
            self.auto_attack(options, view, ops, cooldown, request);
        }
    }

    fn handle_attack(&mut self, options: &Options, view: &mut View, ops: &mut Vec<Op>) -> bool {
        if self.weapon_swap_active || !options.can_operate(view) {
            return false;
        }
        if view.fall_flying || view.using_item {
            return false;
        }
        let Some(target) = mace_target(view) else {
            return false;
        };
        if self.stun_slam(options, &target) {
            let Some(mace) = options.best_mace(view, Some(&target), false) else {
                return false;
            };
            if view.held().axe() {
                // The axe hit goes through; the mace follows it.
                self.begin_stun_slam(view, mace, target.id);
                return false;
            }
            if let Some(axe) = axe_slot(view) {
                let selected = view.selected;
                select(view, ops, axe);
                self.begin_stun_slam(view, mace, target.id);
                self.original_slot = Some(selected);
                return true;
            }
        }
        let Some(mace) = options.best_mace(view, Some(&target), true) else {
            return false;
        };
        self.original_slot = Some(view.selected);
        select(view, ops, mace);
        self.weapon_swap_active = true;
        self.weapon_swap_ticks = 0;
        false
    }

    /// `shouldStunSlam`: the chance rolls, and the target is a player whose
    /// shield has been up for at least 5 ticks.
    fn stun_slam(&mut self, options: &Options, target: &Crosshair) -> bool {
        let chance = options.chance;
        if !options.stun_slam
            || chance <= 0.0
            || chance < 100.0 && chance <= self.random.double() * 100.0
        {
            return false;
        }
        target
            .player
            .and_then(|p| p.shield)
            .is_some_and(|s| s.using && s.elapsed >= 5)
    }

    fn begin_stun_slam(&mut self, view: &View, mace: usize, target: i32) {
        self.original_slot = Some(view.selected);
        self.mace_slot = Some(mace);
        self.original_attack_tick = view.ticks_existed;
        self.target_id = Some(target);
        self.weapon_swap_active = true;
        self.followup_pending = true;
        self.weapon_swap_ticks = 0;
    }

    /// `completeStunSlamFollowup`: the mace and a marked synthetic attack.
    fn complete_followup(&mut self, view: &mut View, ops: &mut Vec<Op>, request: &mut Request) {
        if let Some(mace) = self
            .mace_slot
            .filter(|&s| s < 9 && view.slot(s).is_some_and(Slot::mace))
        {
            select(view, ops, mace);
            ops.push(Op::Release);
            self.release_pending = request(view, ops, true);
        }
        self.followup_pending = false;
        self.mace_slot = None;
        self.original_attack_tick = -1;
        self.target_id = None;
    }

    /// `abortWeaponSwap`.
    fn abort(&mut self, view: &mut View, ops: &mut Vec<Op>) {
        if let Some(slot) = self.original_slot.take() {
            select(view, ops, slot);
        }
        self.clear_weapon_swap();
    }

    fn clear_weapon_swap(&mut self) {
        self.weapon_swap_active = false;
        self.weapon_swap_ticks = 0;
        self.followup_pending = false;
        self.mace_slot = None;
        self.original_attack_tick = -1;
        self.target_id = None;
    }

    /// `updateAutoAttack`.
    fn auto_attack(
        &mut self,
        options: &Options,
        view: &mut View,
        ops: &mut Vec<Op>,
        cooldown: Cooldown,
        request: &mut Request,
    ) {
        let falling = !view.on_ground && view.descending;
        let input_elapsed = self
            .input_at
            .is_none_or(|at| view.now_ms.saturating_sub(at) >= 50);
        if !options.attack
            || self.weapon_swap_active
            || self.release_pending
            || view.using_item
            || view.fall_flying
            || !falling
            || !input_elapsed
            || !(options.can_operate(view) && view.input_enabled)
        {
            return;
        }
        let Some(target) = mace_target(view) else {
            return;
        };
        let offset = -(self.random.range(options.extra_delay) as f32);
        let held_ready = options.held_mace_ready(view, Some(&target));
        let alternate_ready = options.best_mace(view, Some(&target), true).is_some();
        // `isAutoAttackReady`: a smash with a mace needs no cooldown.
        let ready = view.will_smash && (alternate_ready || held_ready) || cooldown.ready(offset);
        if !ready {
            return;
        }
        let stun_ready = options.stun_slam
            && options.best_mace(view, Some(&target), false).is_some()
            && axe_slot(view).is_some();
        if !held_ready && !alternate_ready && !stun_ready {
            return;
        }
        let cancelled = self.handle_attack(options, view, ops);
        if !cancelled && (held_ready || alternate_ready || self.followup_pending) {
            ops.push(Op::Release);
            self.release_pending = request(view, ops, false);
        }
    }
}

/// `getCrosshairTarget`: the crosshair entity AutoMace's filter accepts.
fn mace_target(view: &View) -> Option<Crosshair> {
    view.crosshair.filter(|c| c.mace_target)
}

#[cfg(test)]
mod tests {
    use super::super::super::super::inventory::item_limit::HeldItem;
    use super::super::weapon_swap::{Shield, Target};
    use super::*;

    fn slot(id: &str, density: i32, breach: i32) -> Slot {
        Slot {
            item: HeldItem {
                id: (!id.is_empty()).then(|| id.into()),
                ..HeldItem::default()
            },
            density,
            breach,
        }
    }
    /// Falling 4 blocks onto an unarmored mob, a sword held.
    fn view(hotbar: &[Slot]) -> View {
        let mut hotbar = hotbar.to_vec();
        hotbar.resize(9, Slot::default());
        View {
            hotbar,
            crosshair: Some(Crosshair {
                id: 7,
                mace_target: true,
                ..Crosshair::default()
            }),
            will_smash: true,
            descending: true,
            fall_distance: 4.0,
            ticks_existed: 100,
            input_enabled: true,
            now_ms: 1_000,
            ..View::default()
        }
    }
    fn shielded(v: &mut View, elapsed: i32) {
        v.crosshair.as_mut().unwrap().player = Some(Target {
            shield: Some(Shield {
                using: true,
                elapsed,
            }),
        });
    }
    fn options() -> Options {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s.validate_conditions().unwrap();
        Options::read(&s)
    }
    fn tick(m: &mut AutoMace, o: &Options, v: &mut View, ready: bool) -> Vec<Op> {
        let mut ops = Vec::new();
        let cooldown = Cooldown {
            ticks: if ready { 20.0 } else { 0.0 },
            delay: 12.5,
        };
        m.tick(o, v, &mut ops, cooldown, &mut |_, ops, mark| {
            ops.push(if mark { Op::Press } else { Op::Release });
            ops.push(Op::Press);
            true
        });
        v.ticks_existed += 1;
        v.now_ms += 50;
        ops
    }

    #[test]
    fn defaults_and_suffix_match_the_reference() {
        let o = options();
        assert!(!o.attack && o.smash_only && !o.stun_slam && !o.limit_items);
        assert_eq!(
            (o.selection, o.chance, o.extra_delay),
            (Selection::Density, 100.0, (0.0, 0.0))
        );
        assert_eq!(o.suffix(), "Density");
        assert_eq!(
            Options {
                selection: Selection::Auto,
                ..o.clone()
            }
            .suffix(),
            "Auto"
        );
        assert_eq!(
            Options {
                selection: Selection::Breach,
                ..o
            }
            .suffix(),
            "Breach"
        );
        let ids: Vec<_> = definitions().into_iter().map(|d| d.id).collect();
        let at = |k: &str| ids.iter().position(|i| *i == format!("{ID}.{k}")).unwrap();
        assert!(at("targets.players") < at("aim") && at("aim") < at("attack"));
        assert!(at("smash-only") < at("mace-selection") && at("stun-slam") < at("chance"));
    }

    #[test]
    fn a_falling_hit_takes_the_configured_mace_and_swaps_back() {
        let o = options();
        let mut v = view(&[
            slot("diamond_sword", 0, 0),
            slot("mace", 0, 3),
            slot("mace", 2, 0),
        ]);
        let mut m = AutoMace::new(3);
        assert!(!m.press(&o, &mut v, &mut Vec::new(), false));
        assert_eq!(v.selected, 2);
        assert!(tick(&mut m, &o, &mut v, false).is_empty());
        assert!(tick(&mut m, &o, &mut v, false).is_empty());
        assert_eq!(tick(&mut m, &o, &mut v, false), vec![Op::Select(0)]);
        assert!(!m.active());
        // Breach type; then no Breach mace at all: any mace will do.
        let breach = Options {
            selection: Selection::Breach,
            ..o.clone()
        };
        assert!(!m.press(&breach, &mut v, &mut Vec::new(), false));
        assert_eq!(v.selected, 1);
        let mut v = view(&[slot("diamond_sword", 0, 0), slot("mace", 0, 0)]);
        let mut m = AutoMace::new(3);
        m.press(&breach, &mut v, &mut Vec::new(), false);
        assert_eq!(v.selected, 1);
    }

    #[test]
    fn smash_only_screens_limits_and_cancelled_presses_do_nothing() {
        let o = options();
        let hotbar = [slot("diamond_sword", 0, 0), slot("mace", 1, 0)];
        let mut m = AutoMace::new(3);
        let mut v = view(&hotbar);
        v.will_smash = false;
        m.press(&o, &mut v, &mut Vec::new(), false);
        assert_eq!(v.selected, 0);
        let mut v = view(&hotbar);
        v.screen_open = true;
        m.press(&o, &mut v, &mut Vec::new(), false);
        let mut v = view(&hotbar);
        m.press(&o, &mut v, &mut Vec::new(), true);
        let limited = Options {
            limit_items: true,
            allowed_items: vec!["axes".into()],
            ..o.clone()
        };
        m.press(&limited, &mut v, &mut Vec::new(), false);
        v.crosshair.as_mut().unwrap().mace_target = false;
        m.press(&o, &mut v, &mut Vec::new(), false);
        assert_eq!(v.selected, 0);
        for source in [
            Source::AutoMace(false),
            Source::HitSwap,
            Source::ShieldBreaker,
        ] {
            m.synthetic(&o, source, &mut view(&hotbar), &mut Vec::new(), false);
        }
        assert!(!m.active());
        let mut v = view(&hotbar);
        m.synthetic(&o, Source::TriggerBot, &mut v, &mut Vec::new(), false);
        assert_eq!(v.selected, 1);
    }

    #[test]
    fn auto_selection_estimates_the_damage() {
        let armored = Crosshair {
            armor: 20.0,
            toughness: 12.0,
            mace_target: true,
            ..Crosshair::default()
        };
        // A long fall favours Density, a short one Breach against armor.
        assert!((estimate(10.0, Some(&armored), 5, 0) - 35.2).abs() < 1e-9);
        assert!((estimate(10.0, Some(&armored), 0, 4) - 30.0).abs() < 1e-9);
        assert!((estimate(2.0, Some(&armored), 5, 0) - 6.688).abs() < 1e-9);
        assert!((estimate(2.0, Some(&armored), 0, 4) - 12.768).abs() < 1e-9);
        assert_eq!(estimate(9.0, None, 0, 0), 6.0 + 23.0);
        let o = Options {
            selection: Selection::Auto,
            ..options()
        };
        let hotbar = [
            slot("diamond_sword", 0, 0),
            slot("mace", 5, 0),
            slot("mace", 0, 4),
        ];
        let mut v = view(&hotbar);
        *v.crosshair.as_mut().unwrap() = Crosshair { id: 7, ..armored };
        v.fall_distance = 10.0;
        assert_eq!(o.best_mace(&v, mace_target(&v).as_ref(), true), Some(1));
        v.fall_distance = 2.0;
        assert_eq!(o.best_mace(&v, mace_target(&v).as_ref(), true), Some(2));
        // On the ground (Smash only off) only Breach scores.
        let ground = Options {
            smash_only: false,
            ..o
        };
        v.on_ground = true;
        assert_eq!(
            ground.best_mace(&v, mace_target(&v).as_ref(), true),
            Some(2)
        );
    }

    #[test]
    fn stun_slam_with_an_axe_in_hand_follows_the_hit_with_the_mace() {
        let o = Options {
            stun_slam: true,
            ..options()
        };
        let mut v = view(&[slot("iron_axe", 0, 0), slot("mace", 1, 0)]);
        shielded(&mut v, 5);
        let mut m = AutoMace::new(3);
        assert!(!m.press(&o, &mut v, &mut Vec::new(), false));
        assert_eq!(v.selected, 0);
        // Another entity's hit is not the follow-up.
        let mut ops = Vec::new();
        let mut request = |_: &mut View, ops: &mut Vec<Op>, mark: bool| {
            assert!(mark);
            ops.push(Op::Press);
            true
        };
        m.post_attack(&mut v, &mut ops, 8, &mut request);
        assert!(ops.is_empty());
        m.post_attack(&mut v, &mut ops, 7, &mut request);
        assert_eq!(ops, vec![Op::Select(1), Op::Release, Op::Press]);
        // The press is released, then the axe slot returns.
        assert_eq!(tick(&mut m, &o, &mut v, false), vec![Op::Release]);
        assert!(tick(&mut m, &o, &mut v, false).is_empty());
        assert_eq!(tick(&mut m, &o, &mut v, false), vec![Op::Select(0)]);
    }

    #[test]
    fn stun_slam_without_an_axe_cancels_swaps_and_slams_next_tick() {
        let o = Options {
            stun_slam: true,
            ..options()
        };
        let mut v = view(&[
            slot("diamond_sword", 0, 0),
            slot("mace", 1, 0),
            slot("stone_axe", 0, 0),
        ]);
        shielded(&mut v, 6);
        let mut m = AutoMace::new(3);
        let mut ops = Vec::new();
        assert!(m.press(&o, &mut v, &mut ops, false));
        assert_eq!(ops, vec![Op::Select(2)]);
        // The attack tick waits; the next one, still on the target, slams.
        assert!(tick(&mut m, &o, &mut v, false).is_empty());
        assert_eq!(
            tick(&mut m, &o, &mut v, false),
            vec![Op::Select(1), Op::Release, Op::Press, Op::Press]
        );
        // Off the target instead: the original slot returns.
        let mut v = view(&[
            slot("diamond_sword", 0, 0),
            slot("mace", 1, 0),
            slot("stone_axe", 0, 0),
        ]);
        shielded(&mut v, 6);
        let mut m = AutoMace::new(3);
        assert!(m.press(&o, &mut v, &mut Vec::new(), false));
        v.crosshair = None;
        v.ticks_existed += 1;
        assert_eq!(tick(&mut m, &o, &mut v, false), vec![Op::Select(0)]);
        assert!(!m.active());
        // A shield up for 4 ticks, or a 0% chance, is a plain mace swap.
        for (elapsed, chance) in [(4, 100.0), (8, 0.0)] {
            let o = Options {
                chance,
                ..o.clone()
            };
            let mut v = view(&[
                slot("diamond_sword", 0, 0),
                slot("mace", 1, 0),
                slot("stone_axe", 0, 0),
            ]);
            shielded(&mut v, elapsed);
            assert!(!AutoMace::new(3).press(&o, &mut v, &mut Vec::new(), false));
            assert_eq!(v.selected, 1);
        }
        assert!(AutoMace::can_handle_mace_attack(&o, &v));
        assert!(!AutoMace::can_handle_mace_attack(&options(), &v));
    }

    #[test]
    fn attack_presses_while_falling_onto_a_target() {
        let o = Options {
            attack: true,
            ..options()
        };
        let hotbar = [slot("diamond_sword", 0, 0), slot("mace", 1, 0)];
        let mut m = AutoMace::new(3);
        // A smash needs no cooldown: swap, release, press.
        let mut v = view(&hotbar);
        assert_eq!(
            tick(&mut m, &o, &mut v, false),
            vec![Op::Select(1), Op::Release, Op::Release, Op::Press]
        );
        assert!(AutoMace::has_ready_mace(&o, &view(&hotbar)));
        // The pending press is released on the next tick, which attacks nothing.
        assert_eq!(tick(&mut m, &o, &mut v, false), vec![Op::Release]);
        // Not falling, on the ground, or a press under 50 ms ago: nothing.
        let mut m = AutoMace::new(3);
        let mut v = view(&hotbar);
        v.descending = false;
        assert!(tick(&mut m, &o, &mut v, true).is_empty());
        let mut v = view(&hotbar);
        v.input_enabled = false;
        assert!(tick(&mut m, &o, &mut v, true).is_empty());
        let mut v = view(&[slot("diamond_sword", 0, 0)]);
        assert!(tick(&mut m, &o, &mut v, true).is_empty());
        let mut v = view(&hotbar);
        m.press(&o, &mut v, &mut Vec::new(), true);
        v.now_ms += 49;
        let mut ops = Vec::new();
        m.tick(&o, &mut v, &mut ops, Cooldown::default(), &mut |_, _, _| {
            true
        });
        assert!(ops.is_empty());
        // Without a smash a held mace waits for the cooldown.
        let mut m = AutoMace::new(3);
        let mut v = view(&[slot("mace", 1, 0)]);
        v.will_smash = false;
        let smashless = Options {
            smash_only: false,
            ..o
        };
        assert!(tick(&mut m, &smashless, &mut v, false).is_empty());
        assert_eq!(
            tick(&mut m, &smashless, &mut v, true),
            vec![Op::Release, Op::Release, Op::Press]
        );
    }

    #[test]
    fn disabling_releases_a_pending_press() {
        let o = Options {
            attack: true,
            ..options()
        };
        let mut m = AutoMace::new(3);
        let mut v = view(&[slot("diamond_sword", 0, 0), slot("mace", 1, 0)]);
        tick(&mut m, &o, &mut v, false);
        let mut ops = Vec::new();
        m.reset(&mut ops);
        assert_eq!(ops, vec![Op::Release]);
        assert!(!m.active());
    }
}
