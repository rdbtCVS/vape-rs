// KillAura (reference `KillAura`): swings at every valid target in swing
// range and attacks the ones in attack range by entity id, without turning
// the view, at the reference's randomized click rate. Timers use the caller's
// monotonic milliseconds. Show target boxes the current targets
// (`aura_render`).
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use super::super::super::targeting::SortMode;
use super::super::super::targeting::entities::{
    EntityClass, EntityView, PlayerView, int_yaw_difference,
};
use super::super::super::targeting::filter::{self, People, TargetFilter};
use super::click_timing::{Mode, Timing};
use serde_json::{Value, json};

pub const ID: &str = "combat.kill-aura";
pub const SORT_MODES: [&str; 5] = ["Distance", "Yaw", "Armor", "Threat", "Health"];
/// Show target's colours (ARGB): (255, 200, 112) at alpha 50 and
/// (255, 0, 0, 100), as both auras have them.
pub const TARGET_COLOR: u32 = 0x32ff_c870;
pub const ATTACK_COLOR: u32 = 0x64ff_0000;
/// The step the reference's range listeners keep between the two ranges.
pub const RANGE_GAP: f64 = 0.1;

pub fn sort_mode(name: &str) -> SortMode {
    match name {
        "Yaw" => SortMode::Yaw,
        "Armor" => SortMode::Armor,
        "Threat" => SortMode::Threat,
        "Health" => SortMode::Health,
        _ => SortMode::Distance,
    }
}

/// The reference comparators, all ascending. Armor and Threat order players
/// by their equipment scores and fall back to distance otherwise.
pub fn sort(targets: &mut [EntityView], player: &PlayerView, mode: SortMode) {
    let distance = |e: &EntityView| player.distance(e.tick_position);
    targets.sort_by(|a, b| {
        let key = |e: &EntityView| match mode {
            SortMode::Distance => distance(e),
            SortMode::Yaw => f64::from(int_yaw_difference(player, e.tick_position)),
            SortMode::Health => f64::from(e.health),
            SortMode::Armor | SortMode::Threat
                if a.class != EntityClass::Player || b.class != EntityClass::Player =>
            {
                distance(e)
            }
            SortMode::Armor => e.armor,
            SortMode::Threat => e.threat,
        };
        key(a).total_cmp(&key(b))
    });
}

pub(crate) fn range(settings: &Settings, key: &str, default: (f64, f64)) -> (f64, f64) {
    let v = settings.get(key).and_then(Value::as_array);
    let at = |i: usize, d: f64| {
        v.and_then(|v| v.get(i))
            .and_then(Value::as_f64)
            .unwrap_or(d)
    };
    (at(0, default.0), at(1, default.1))
}

pub(crate) fn color(value: Option<&Value>, default: u32) -> u32 {
    value.and_then(Value::as_u64).map_or(default, |c| c as u32)
}

/// The reference's range listeners: a swing range set below the attack
/// range comes back above it, an attack range set past the swing range
/// comes back below it. `changed` is the setting just set; the result is
/// the corrected (swing, attack) when one has to move.
pub fn couple_ranges(changed: &str, swing: f64, attack: f64) -> Option<(f64, f64)> {
    if swing >= attack {
        return None;
    }
    if changed == format!("{ID}.attack-range") {
        Some((swing, swing - RANGE_GAP))
    } else {
        Some((attack + RANGE_GAP, attack))
    }
}

pub(crate) fn strings(settings: &Settings, key: &str) -> Vec<String> {
    settings
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

pub(crate) fn aps_definition(key: &str) -> Definition {
    Definition::new(
        key,
        "Attacks per Second",
        Kind::Range {
            min: 1.0,
            max: 20.0,
            step: 1.0,
        },
        json!([6.0, 13.0]),
    )
}

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let mut out = filter::definitions(ID);
    out.extend([
        aps_definition(&key("aps")),
        Definition::number(&key("swing-range"), "Swing range", 0.0, 4.0, 6.0, 0.1),
        Definition::number(&key("attack-range"), "Attack range", 0.0, 3.5, 6.0, 0.1),
        Definition::number(&key("max-angle"), "Max angle", 1.0, 90.0, 360.0, 5.0),
        Definition::number(&key("max-targets"), "Max targets", 1.0, 1.0, 6.0, 1.0),
        Definition::choice(&key("target-mode"), "Target Mode", &SORT_MODES, "Distance"),
        Definition::boolean(&key("disable-on-death"), "Disable on death", false),
        Definition::boolean(&key("require-mouse"), "Require mouse down", false),
        Definition::boolean(&key("gui-check"), "GUI check", true),
        Definition::boolean(&key("show-target"), "Show target", false),
        Definition::new(
            &key("target-color"),
            "Target Color",
            Kind::Color,
            json!(TARGET_COLOR),
        )
        .when(&key("show-target"), json!(true)),
        Definition::new(
            &key("attack-color"),
            "Attack Color",
            Kind::Color,
            json!(ATTACK_COLOR),
        )
        .when(&key("show-target"), json!(true)),
        Definition::boolean(&key("limit-items"), "Limit to items", false),
        Definition::new(
            &key("allowed-items"),
            "Allowed Items",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!([]),
        )
        .when(&key("limit-items"), json!(true)),
        Definition::boolean(&key("perfect-swing"), "Perfect swing", false),
    ]);
    out
}

#[derive(Clone, Debug)]
pub struct Options {
    pub filter: TargetFilter,
    pub aps: (f64, f64),
    pub swing_range: f64,
    pub attack_range: f64,
    pub max_angle: f64,
    pub max_targets: usize,
    pub mode: SortMode,
    pub disable_on_death: bool,
    pub require_mouse: bool,
    pub gui_check: bool,
    pub show_target: bool,
    pub target_color: u32,
    pub attack_color: u32,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
    pub perfect_swing: bool,
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let boolean = |k: &str, d: bool| get(k).and_then(Value::as_bool).unwrap_or(d);
        let number = |k: &str, d: f64| get(k).and_then(Value::as_f64).unwrap_or(d);
        Self {
            filter: TargetFilter::read(settings, ID),
            aps: range(settings, &format!("{ID}.aps"), (6.0, 13.0)),
            swing_range: number("swing-range", 4.0),
            attack_range: number("attack-range", 3.5),
            max_angle: number("max-angle", 90.0),
            max_targets: number("max-targets", 1.0).max(1.0) as usize,
            mode: sort_mode(
                get("target-mode")
                    .and_then(Value::as_str)
                    .unwrap_or("Distance"),
            ),
            disable_on_death: boolean("disable-on-death", false),
            require_mouse: boolean("require-mouse", false),
            gui_check: boolean("gui-check", true),
            show_target: boolean("show-target", false),
            target_color: color(get("target-color"), TARGET_COLOR),
            attack_color: color(get("attack-color"), ATTACK_COLOR),
            limit_items: boolean("limit-items", false),
            allowed_items: strings(settings, &format!("{ID}.allowed-items")),
            perfect_swing: boolean("perfect-swing", false),
        }
    }
}

/// One client tick of host state.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    pub now_ms: u64,
    pub player: PlayerView,
    pub entities: Vec<EntityView>,
    pub attack_down: bool,
    pub use_down: bool,
    /// Using an item (blocking) this tick.
    pub using: bool,
    /// No screen is open.
    pub no_screen: bool,
    pub focused: bool,
    pub alive: bool,
    pub held: HeldItem,
    /// The attack strength is full (Perfect swing).
    pub cooldown_ready: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    /// Swing at each, attacking the ones whose flag is set.
    pub swings: Vec<(i32, bool)>,
    /// Release the use key (`setKeyBindState(use, false)`).
    pub release_use: bool,
    /// Use the held item again after the hits (re-block).
    pub reuse: bool,
    pub disable: bool,
}

#[derive(Debug)]
pub struct KillAura {
    timing: Timing,
    last_click_ms: Option<u64>,
    /// `targets`: what the last update chose, for Show target.
    shown: Vec<i32>,
    /// `pauseTicks`: the tick after the GUI check paused is skipped too.
    pause: u8,
}
impl Default for KillAura {
    fn default() -> Self {
        Self::new(0x006b_6175_7261)
    }
}
impl KillAura {
    pub fn new(seed: u64) -> Self {
        Self {
            timing: Timing::new(seed),
            last_click_ms: None,
            shown: Vec::new(),
            pause: 0,
        }
    }
    pub fn reset(&mut self) {
        self.last_click_ms = None;
        self.shown.clear();
        self.pause = 0;
    }
    /// The targets Show target boxes, best first.
    pub fn shown(&self) -> &[i32] {
        &self.shown
    }
    /// `RandomClickDelayValue.hasClickDelayElapsed`: the delay is re-rolled
    /// on every check; the reference's span quirk always picks the maximum.
    pub(crate) fn click_ready(
        timing: &mut Timing,
        last: Option<u64>,
        now: u64,
        aps: (f64, f64),
    ) -> bool {
        let max = aps.1.max(aps.0).clamp(1.0, 20.0) as u32;
        let delay = timing.delay(max, max, Mode::Extra);
        last.is_none_or(|last| now.saturating_sub(last) >= delay)
    }
    pub fn targets(options: &Options, tick: &Tick, people: &impl People) -> Vec<EntityView> {
        let mut out: Vec<EntityView> = tick
            .entities
            .iter()
            .filter(|e| {
                e.alive()
                    && !e.vehicle
                    && tick.player.distance(e.tick_position) < options.swing_range
                    && f64::from(int_yaw_difference(&tick.player, e.tick_position))
                        <= options.max_angle / 2.0
                    && !(e.class == EntityClass::Player && people.friend(&e.name))
                    && options.filter.valid(e, people)
            })
            .cloned()
            .collect();
        sort(&mut out, &tick.player, options.mode);
        out.truncate(options.max_targets);
        out
    }
    pub fn tick(&mut self, options: &Options, tick: &Tick, people: &impl People) -> Effects {
        let mut effects = Effects::default();
        if !tick.alive {
            effects.disable = options.disable_on_death;
            return effects;
        }
        if self.pause > 0 {
            self.pause -= 1;
            return effects;
        }
        if options.gui_check && !tick.no_screen {
            self.pause = 1;
            return effects;
        }
        if options.require_mouse && !tick.attack_down {
            self.shown.clear();
            return effects;
        }
        // Limit to items is part of target validity: no targets at all.
        let targets =
            if options.limit_items && !item_limit::matches(&options.allowed_items, &tick.held) {
                Vec::new()
            } else {
                Self::targets(options, tick, people)
            };
        self.shown = targets.iter().map(|e| e.handle.id).collect();
        if targets.is_empty() || !tick.focused {
            return effects;
        }
        let ready = if options.perfect_swing {
            tick.cooldown_ready
        } else {
            Self::click_ready(
                &mut self.timing,
                self.last_click_ms,
                tick.now_ms,
                options.aps,
            )
        };
        if !ready {
            return effects;
        }
        if tick.use_down && !tick.using {
            effects.release_use = true;
            return effects;
        }
        for target in &targets {
            let attack = tick.player.distance(target.tick_position) <= options.attack_range;
            effects.swings.push((target.handle.id, attack));
        }
        effects.reuse = tick.using;
        self.last_click_ms = Some(tick.now_ms);
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::targeting::filter::NameLists;
    use super::super::super::super::targeting::filter::tests::entity as base;
    use super::*;

    pub(crate) fn entity(id: i32, at: [f64; 3], health: f32) -> EntityView {
        let mut e = base(EntityClass::Monster, "", at);
        e.handle.id = id;
        e.health = health;
        e
    }

    fn options() -> Options {
        let mut settings = Settings::default();
        for d in definitions() {
            settings.define(d).unwrap();
        }
        settings
            .set(&format!("{ID}.targets.mobs"), json!(true))
            .unwrap();
        Options::read(&settings)
    }

    fn tick(now: u64, entities: Vec<EntityView>) -> Tick {
        Tick {
            now_ms: now,
            entities,
            no_screen: true,
            focused: true,
            alive: true,
            ..Tick::default()
        }
    }

    #[test]
    fn swings_in_swing_range_and_attacks_in_attack_range() {
        let mut aura = KillAura::new(7);
        let mut o = options();
        o.max_targets = 3;
        let near = entity(1, [0.0, 0.0, 3.0], 20.0);
        let far = entity(2, [0.0, 0.0, 3.8], 20.0);
        let behind = entity(3, [0.1, 0.0, -2.0], 20.0);
        let effects = aura.tick(
            &o,
            &tick(1000, vec![far, near, behind]),
            &NameLists::default(),
        );
        assert_eq!(effects.swings, vec![(1, true), (2, false)]);
        // The click timer holds the next swing back.
        assert!(
            aura.tick(
                &o,
                &tick(1010, vec![entity(1, [0.0, 0.0, 3.0], 20.0)]),
                &NameLists::default()
            )
            .swings
            .is_empty()
        );
        assert!(
            !aura
                .tick(
                    &o,
                    &tick(1300, vec![entity(1, [0.0, 0.0, 3.0], 20.0)]),
                    &NameLists::default()
                )
                .swings
                .is_empty()
        );
    }

    #[test]
    fn sort_modes_order_ascending() {
        let player = PlayerView::default();
        let mut list = vec![
            entity(1, [0.0, 0.0, 3.0], 5.0),
            entity(2, [0.0, 0.0, 2.0], 9.0),
        ];
        sort(&mut list, &player, SortMode::Health);
        assert_eq!(list[0].handle.id, 1);
        sort(&mut list, &player, SortMode::Distance);
        assert_eq!(list[0].handle.id, 2);
        // Non-players fall back to distance for Armor.
        list[1].armor = 10.0;
        sort(&mut list, &player, SortMode::Armor);
        assert_eq!(list[0].handle.id, 2);
    }

    #[test]
    fn requires_mouse_and_gui_check_and_releases_use() {
        let mut aura = KillAura::new(3);
        let mut o = options();
        o.require_mouse = true;
        let t = tick(1000, vec![entity(1, [0.0, 0.0, 2.0], 20.0)]);
        assert!(aura.tick(&o, &t, &NameLists::default()).swings.is_empty());
        let t = Tick {
            attack_down: true,
            use_down: true,
            ..t
        };
        assert!(aura.tick(&o, &t, &NameLists::default()).release_use);
        let t = Tick {
            no_screen: false,
            ..t
        };
        assert_eq!(aura.tick(&o, &t, &NameLists::default()), Effects::default());
    }

    #[test]
    fn range_listeners_keep_the_swing_range_above_the_attack_range() {
        // Swing set below the attack range comes back one step above it.
        assert_eq!(
            couple_ranges(&format!("{ID}.swing-range"), 3.0, 3.5),
            Some((3.6, 3.5))
        );
        // Attack set past the swing range comes back one step below it.
        let (swing, attack) = couple_ranges(&format!("{ID}.attack-range"), 4.0, 4.5).unwrap();
        assert!(swing == 4.0 && (attack - 3.9).abs() < 1e-9);
        // `onAttackTick` (any other id) raises the swing range.
        let (swing, _) = couple_ranges("", 3.0, 3.5).unwrap();
        assert!((swing - 3.6).abs() < 1e-9);
        assert_eq!(couple_ranges("", 4.0, 3.5), None);
        assert_eq!(couple_ranges("", 3.5, 3.5), None);
    }

    #[test]
    fn show_target_keeps_the_chosen_targets_and_the_gui_check_pauses_a_tick() {
        let mut aura = KillAura::new(5);
        let mut o = options();
        assert!(!o.show_target);
        assert_eq!(
            (o.target_color, o.attack_color),
            (TARGET_COLOR, ATTACK_COLOR)
        );
        o.max_targets = 2;
        o.mode = SortMode::Health;
        let entities = vec![
            entity(1, [0.0, 0.0, 2.0], 18.0),
            entity(2, [0.0, 0.0, 3.0], 4.0),
            entity(3, [0.0, 0.0, 3.5], 10.0),
        ];
        aura.tick(&o, &tick(1000, entities.clone()), &NameLists::default());
        assert_eq!(aura.shown(), [2, 3]);
        // A screen pauses this tick and the next; targets stay shown.
        let screen = Tick {
            no_screen: false,
            ..tick(1100, entities.clone())
        };
        assert_eq!(
            aura.tick(&o, &screen, &NameLists::default()),
            Effects::default()
        );
        let after = tick(1150, entities.clone());
        assert_eq!(
            aura.tick(&o, &after, &NameLists::default()),
            Effects::default()
        );
        assert!(
            !aura
                .tick(&o, &tick(1500, entities), &NameLists::default())
                .swings
                .is_empty()
        );
        // Require mouse down with the button up shows nothing.
        o.require_mouse = true;
        aura.tick(
            &o,
            &tick(2000, vec![entity(1, [0.0, 0.0, 2.0], 9.0)]),
            &NameLists::default(),
        );
        assert!(aura.shown().is_empty());
        aura.reset();
        assert!(aura.shown().is_empty());
    }
}
