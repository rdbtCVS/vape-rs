// AutoArmor (AutoArmor.java). Each pass walks the armor slots from helmet to
// boots and, for every slot with a better candidate, queues the removal of
// the equipped piece (shift-click, or pick it up and click outside the menu
// with "Drop equipped") followed by a shift-click of the candidate. Queued
// clicks then run one per "Delay" sample. The open timer is reset whenever
// the screen changes, while in combat and before closing an inventory this
// planner opened. Clicks resolve through the planner gate, so a click is
// only followed up once a snapshot shows its (possibly predicted) effect.
// With "Open inventory" and "Inventory only" both off and no screen shown,
// the clicks go to the player's menu (window 0) as the reference's do.
use super::super::super::inventory::{
    planner::{Gate, Outcome},
    types::{ArmorSlot, Click, Item, Request, Slot, SlotRole, Snapshot},
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub open_inventory: bool,
    pub inventory_only: bool,
    pub check_durability: bool,
    pub drop_equipped: bool,
    pub combat_check: bool,
    /// "Delay" bounds in milliseconds.
    pub delay: (u64, u64),
}
impl Default for Options {
    fn default() -> Self {
        Self {
            open_inventory: true,
            inventory_only: true,
            check_durability: true,
            drop_equipped: false,
            combat_check: false,
            delay: (100, 120),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    Request(Request),
    /// A click on the player's menu (window 0) while no screen is shown.
    SilentRequest(Request),
    OpenInventory,
    CloseInventory,
}
/// A queued click: a menu slot, or the outside of the menu (slot -999).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Target {
    Slot(u16),
    Outside,
}
#[derive(Debug)]
pub struct AutoArmor {
    gate: Gate,
    queue: VecDeque<(Target, Click)>,
    /// The last observed screen: whether one is open and its menu id.
    screen: (bool, u32),
    /// "openTimer": `None` until the first reset, which counts as elapsed.
    opened_at: Option<Instant>,
    /// `100 + Delay` before anything runs after an open-timer reset.
    open_ms: u64,
    /// `200 + Delay` before "Open inventory" presses the inventory key.
    press_ms: u64,
    /// "clickTimer" and the Delay sample the next queued click waits for.
    clicked_at: Option<Instant>,
    click_ms: u64,
    /// This planner opened the inventory that is shown.
    pressed: bool,
    /// Nothing is left to equip; close the inventory once the timer allows.
    closing: bool,
    seed: u64,
}
impl Default for AutoArmor {
    fn default() -> Self {
        Self {
            gate: Gate::default(),
            queue: VecDeque::new(),
            screen: (false, 0),
            opened_at: None,
            open_ms: 0,
            press_ms: 0,
            clicked_at: None,
            click_ms: 0,
            pressed: false,
            closing: false,
            seed: 0x2545_f491_4f6c_dd1d,
        }
    }
}
impl AutoArmor {
    /// Drops queued and pending clicks and the open/close latches (the
    /// reference's `onDisable`). The timers keep running, as the
    /// reference's do.
    pub fn cancel(&mut self) {
        self.gate.clear();
        self.queue.clear();
        self.pressed = false;
        self.closing = false;
    }
    /// Whether this planner opened the screen that is currently shown.
    pub fn opened(&self) -> bool {
        self.pressed
    }
    pub fn active(&self) -> bool {
        !self.queue.is_empty() || self.gate.pending().is_some() || self.pressed || self.closing
    }
    pub fn step(
        &mut self,
        snapshot: &Snapshot,
        options: Options,
        combat_blocked: bool,
    ) -> Vec<Effect> {
        let now = snapshot.observed;
        let screen = (snapshot.screen_open, snapshot.container_id);
        if screen != self.screen {
            self.screen = screen;
            self.reset_open(now, options.delay);
        }
        match self.gate.observe(snapshot) {
            Outcome::Waiting => return Vec::new(),
            Outcome::Failed(_) | Outcome::Invalidated => self.queue.clear(),
            Outcome::Idle | Outcome::Applied => {}
        }
        // Another container's menu is open (window id other than 0).
        if snapshot.container_id != 0 || snapshot.is_chest {
            return Vec::new();
        }
        let inventory = snapshot.is_player_inventory;
        if !inventory && (options.open_inventory || options.inventory_only) {
            self.queue.clear();
        }
        if !inventory && options.inventory_only {
            return Vec::new();
        }
        if !elapsed(self.opened_at, now, self.open_ms) {
            return Vec::new();
        }
        if let Some(&(target, click)) = self.queue.front() {
            if !elapsed(self.clicked_at, now, self.click_ms) {
                return Vec::new();
            }
            self.queue.pop_front();
            self.clicked_at = Some(now);
            self.click_ms = self.sample(options.delay);
            let request = match target {
                Target::Outside => Some(Request::outside(snapshot)),
                Target::Slot(index) => snapshot
                    .slot(index)
                    .map(|slot| Request::for_slot(snapshot, slot, click)),
            };
            return match request.and_then(|r| self.gate.issue(snapshot, r)) {
                Some(request) if snapshot.screen_open => vec![Effect::Request(request)],
                Some(request) => vec![Effect::SilentRequest(request)],
                None => {
                    self.queue.clear();
                    Vec::new()
                }
            };
        }
        if self.closing {
            // The reference stays latched while no screen is shown; the
            // latch is dropped instead once the inventory is gone
            // (docs/decisions.md).
            self.closing = false;
            self.pressed = false;
            return if snapshot.screen_open {
                vec![Effect::CloseInventory]
            } else {
                Vec::new()
            };
        }
        if options.combat_check && combat_blocked {
            self.reset_open(now, options.delay);
            return Vec::new();
        }
        // A stalled gate means the last click changed nothing; planning the
        // same clicks again would only repeat it.
        let stalled = self.gate.stalled(snapshot);
        for armor in [
            ArmorSlot::Head,
            ArmorSlot::Chest,
            ArmorSlot::Legs,
            ArmorSlot::Feet,
        ] {
            if stalled {
                break;
            }
            let Some(candidate) = best(snapshot, armor, options.check_durability) else {
                continue;
            };
            if !inventory && options.open_inventory {
                if elapsed(self.opened_at, now, self.press_ms) {
                    self.pressed = true;
                    return vec![Effect::OpenInventory];
                }
                return Vec::new();
            }
            if let Some(equipped) = equipped_slot(snapshot, armor).filter(|s| s.item.is_some()) {
                if options.drop_equipped {
                    self.queue
                        .push_back((Target::Slot(equipped.index), Click::Pickup));
                    self.queue.push_back((Target::Outside, Click::Outside));
                } else {
                    self.queue
                        .push_back((Target::Slot(equipped.index), Click::QuickMove));
                }
            }
            self.queue
                .push_back((Target::Slot(candidate.index), Click::QuickMove));
        }
        if self.pressed && self.queue.is_empty() {
            self.closing = true;
            self.reset_open(now, options.delay);
        }
        Vec::new()
    }
    fn reset_open(&mut self, now: Instant, delay: (u64, u64)) {
        self.opened_at = Some(now);
        self.open_ms = 100 + self.sample(delay);
        self.press_ms = 200 + self.sample(delay);
    }
    /// A uniform "Delay" sample. The reference draws a fresh sample on each
    /// tick it checks a timer; one sample per wait keeps the same bounds.
    fn sample(&mut self, (a, b): (u64, u64)) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        let (min, max) = (a.min(b), a.max(b));
        min + self.seed % (max - min + 1)
    }
}
fn elapsed(since: Option<Instant>, now: Instant, ms: u64) -> bool {
    since.is_none_or(|since| now.saturating_duration_since(since) >= Duration::from_millis(ms))
}
fn equipped_slot(snapshot: &Snapshot, armor: ArmorSlot) -> Option<&Slot> {
    snapshot.slots(|role| role == SlotRole::Armor(armor)).next()
}
/// The armor slot an item id names (`armorSlotFor`), checked boots first.
pub fn armor_slot_for(id: &str) -> Option<ArmorSlot> {
    let id = id.to_lowercase();
    [
        (ArmorSlot::Feet, &["boots"][..]),
        (ArmorSlot::Legs, &["pants", "leggings"]),
        (ArmorSlot::Chest, &["tunic", "chestplate"]),
        (ArmorSlot::Head, &["cap", "helmet"]),
    ]
    .into_iter()
    .find(|(_, names)| names.iter().any(|name| id.contains(name)))
    .map(|(slot, _)| slot)
}
/// `AutoArmor.scoreItem`: worn armor points, toughness and Protection level,
/// plus a tenth per level of Feather Falling, Fire Protection and Blast
/// Protection; a golden helmet loses 0.01. Items that cannot be worn as
/// armor score 0.
pub fn armor_score(item: &Item) -> f64 {
    if item.armor.is_none() {
        return 0.0;
    }
    let meta = &item.meta;
    let mut score = f64::from(meta.armor_points)
        + f64::from(meta.enchantment("protection"))
        + f64::from(meta.armor_toughness);
    for enchantment in ["feather_falling", "fire_protection", "blast_protection"] {
        score += f64::from(meta.enchantment(enchantment)) * 0.1;
    }
    if item.fingerprint.id == "minecraft:golden_helmet" {
        score -= 0.01;
    }
    score
}
/// `findBestArmorSlot`: the main inventory and then the hotbar (menu slots
/// 9..44), keeping the first strictly better score; with "Check durability"
/// an equal score with less damage also wins. An empty armor slot counts as
/// score 0 with damage 999.
fn best(snapshot: &Snapshot, armor: ArmorSlot, durability: bool) -> Option<&Slot> {
    let (mut score, mut damage) = equipped_slot(snapshot, armor)
        .and_then(|s| s.item.as_ref())
        .map_or((0.0, 999.0), |item| {
            (armor_score(item), f64::from(item.durability_used()))
        });
    let mut result = None;
    let candidates = snapshot
        .slots(|role| role == SlotRole::Main)
        .chain(snapshot.slots(|role| matches!(role, SlotRole::Hotbar(_))));
    for slot in candidates {
        let Some(item) = &slot.item else {
            continue;
        };
        if armor_slot_for(&item.fingerprint.id) != Some(armor) {
            continue;
        }
        let (candidate_score, candidate_damage) =
            (armor_score(item), f64::from(item.durability_used()));
        if candidate_score > score {
            score = candidate_score;
            damage = candidate_damage;
            result = Some(slot);
        } else if durability && candidate_score == score && candidate_damage < damage {
            damage = candidate_damage;
            result = Some(slot);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{rules::tests::item, session::tests, types::ItemClass};

    /// The shared test snapshot plus the four armor slots (menu 500..503).
    fn snapshot(open: bool, at: Instant) -> Snapshot {
        let mut s = tests::snapshot(open, at);
        s.slots[20].item = None;
        for (i, armor) in [
            ArmorSlot::Head,
            ArmorSlot::Chest,
            ArmorSlot::Legs,
            ArmorSlot::Feet,
        ]
        .into_iter()
        .enumerate()
        {
            s.slots.push(Slot {
                index: 500 + i as u16,
                item: None,
                role: SlotRole::Armor(armor),
            });
        }
        s
    }
    fn piece(id: &str, armor: ArmorSlot, points: f32) -> Item {
        let mut i = item(id.trim_start_matches("minecraft:"), ItemClass::Other, 0.0);
        i.armor = Some(armor);
        i.meta.armor_points = points;
        i
    }
    fn options() -> Options {
        Options {
            delay: (1, 1),
            ..Options::default()
        }
    }
    fn armor_index(s: &Snapshot, armor: ArmorSlot) -> u16 {
        equipped_slot(s, armor).unwrap().index
    }
    fn set(s: &mut Snapshot, index: u16, value: Option<Item>) {
        s.slots
            .iter_mut()
            .find(|slot| slot.index == index)
            .unwrap()
            .item = value;
    }

    #[test]
    fn names_and_scores_follow_the_reference() {
        assert_eq!(
            armor_slot_for("minecraft:diamond_boots"),
            Some(ArmorSlot::Feet)
        );
        assert_eq!(
            armor_slot_for("minecraft:turtle_helmet"),
            Some(ArmorSlot::Head)
        );
        assert_eq!(armor_slot_for("minecraft:elytra"), None);
        assert_eq!(armor_slot_for("minecraft:carved_pumpkin"), None);
        let mut iron = piece("minecraft:iron_chestplate", ArmorSlot::Chest, 6.0);
        iron.meta.enchantments = vec![
            ("minecraft:protection".into(), 2),
            ("minecraft:blast_protection".into(), 3),
        ];
        assert!((armor_score(&iron) - 8.3).abs() < 1e-9);
        let gold = piece("minecraft:golden_helmet", ArmorSlot::Head, 2.0);
        assert!((armor_score(&gold) - 1.99).abs() < 1e-9);
    }

    #[test]
    fn queues_every_slot_then_clicks_one_per_delay() {
        let t = Instant::now();
        let mut s = snapshot(true, t);
        let head = armor_index(&s, ArmorSlot::Head);
        set(
            &mut s,
            head,
            Some(piece("minecraft:leather_helmet", ArmorSlot::Head, 1.0)),
        );
        s.slots[9].item = Some(piece("minecraft:iron_helmet", ArmorSlot::Head, 2.0));
        s.slots[2].item = Some(piece("minecraft:iron_boots", ArmorSlot::Feet, 2.0));
        let mut armor = AutoArmor::default();
        // The screen change resets the open timer (100 + Delay).
        assert!(armor.step(&s, options(), false).is_empty());
        s.observed = t + Duration::from_millis(101);
        assert!(armor.step(&s, options(), false).is_empty());
        assert_eq!(
            armor.queue,
            [
                (Target::Slot(head), Click::QuickMove),
                (Target::Slot(9), Click::QuickMove),
                (Target::Slot(2), Click::QuickMove),
            ]
        );
        let Effect::Request(first) = &armor.step(&s, options(), false)[0] else {
            panic!("first click");
        };
        assert_eq!((first.slot, first.click), (head, Click::QuickMove));
        assert!(armor.active());
    }

    #[test]
    fn drop_equipped_picks_up_then_clicks_outside() {
        let t = Instant::now();
        let mut s = snapshot(true, t);
        let head = armor_index(&s, ArmorSlot::Head);
        set(
            &mut s,
            head,
            Some(piece("minecraft:leather_helmet", ArmorSlot::Head, 1.0)),
        );
        s.slots[9].item = Some(piece("minecraft:iron_helmet", ArmorSlot::Head, 2.0));
        let mut armor = AutoArmor::default();
        let o = Options {
            drop_equipped: true,
            ..options()
        };
        armor.step(&s, o, false);
        s.observed = t + Duration::from_millis(101);
        armor.step(&s, o, false);
        assert_eq!(
            armor.queue,
            [
                (Target::Slot(head), Click::Pickup),
                (Target::Outside, Click::Outside),
                (Target::Slot(9), Click::QuickMove),
            ]
        );
    }

    #[test]
    fn inventory_only_waits_and_open_inventory_needs_it_off() {
        let t = Instant::now();
        let mut s = snapshot(false, t);
        s.slots[9].item = Some(piece("minecraft:iron_helmet", ArmorSlot::Head, 2.0));
        let mut armor = AutoArmor::default();
        // Defaults: "Inventory only" keeps "Open inventory" from opening.
        s.observed = t + Duration::from_secs(1);
        assert!(armor.step(&s, options(), false).is_empty());
        let open = Options {
            inventory_only: false,
            ..options()
        };
        assert_eq!(armor.step(&s, open, false), vec![Effect::OpenInventory]);
        assert!(armor.opened());
        // Both off: the clicks go to window 0 without a screen.
        let mut screenless = AutoArmor::default();
        let off = Options {
            open_inventory: false,
            ..open
        };
        assert!(screenless.step(&s, off, false).is_empty());
        assert_eq!(screenless.queue, [(Target::Slot(9), Click::QuickMove)]);
        let Effect::SilentRequest(click) = &screenless.step(&s, off, false)[0] else {
            panic!("screenless click");
        };
        assert_eq!((click.slot, click.click), (9, Click::QuickMove));
        assert!(screenless.active() && !screenless.opened());
    }

    #[test]
    fn closes_what_it_opened_after_the_open_timer() {
        let t = Instant::now();
        let mut s = snapshot(true, t);
        let mut armor = AutoArmor {
            pressed: true,
            ..AutoArmor::default()
        };
        let open = Options {
            inventory_only: false,
            ..options()
        };
        // Nothing to equip: start closing and reset the open timer.
        armor.screen = (true, s.container_id);
        armor.step(&s, open, false);
        assert!(armor.closing);
        assert!(armor.step(&s, open, false).is_empty());
        s.observed = t + Duration::from_millis(101);
        assert_eq!(armor.step(&s, open, false), vec![Effect::CloseInventory]);
        assert!(!armor.active());
    }

    #[test]
    fn combat_and_durability_rules() {
        let t = Instant::now();
        let mut s = snapshot(true, t);
        let mut worn = piece("minecraft:iron_helmet", ArmorSlot::Head, 2.0);
        worn.fingerprint.damage = 50;
        let head = armor_index(&s, ArmorSlot::Head);
        set(&mut s, head, Some(worn));
        s.slots[9].item = Some(piece("minecraft:iron_helmet", ArmorSlot::Head, 2.0));
        let mut armor = AutoArmor::default();
        let combat = Options {
            combat_check: true,
            ..options()
        };
        armor.step(&s, combat, false);
        s.observed = t + Duration::from_millis(101);
        armor.step(&s, combat, true);
        assert!(armor.queue.is_empty());
        // Equal score, less damage: replaced only with "Check durability".
        s.observed = t + Duration::from_millis(300);
        let plain = Options {
            check_durability: false,
            ..options()
        };
        armor.step(&s, plain, false);
        assert!(armor.queue.is_empty());
        armor.step(&s, options(), false);
        assert_eq!(armor.queue.len(), 2);
    }
}
