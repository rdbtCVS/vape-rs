// ChestSteal (`world/ChestSteal`): quick-moves a chest's items into the
// player inventory, one click per click delay. The queue is built once per
// opened chest (`initializeStealQueue`) and never re-scanned while the chest
// stays open: every slot except blacklisted items and harmful splash
// potions, or, with "Best only", every non-gear item plus the chest's best
// piece of each gear kind when it beats what the player already carries
// (`ChestStealBestSlotTracker`). "Shuffle" shuffles the queue, then sorts it
// by grid distance from the left of the first row with the reference's
// fuzzy comparator (`ChestStealSlotDistanceComparator`). Far clicks add
// travel time to the click delay (`calculateSlotTravelDelay`). Without
// "Keep open" the chest closes when the queue is done or the inventory is
// full.
use super::super::super::inventory::{
    arrange::java_sort,
    planner::{Gate, Outcome},
    rules::{Category, weapon_damage},
    types::{ArmorSlot, Click, Item, Request, SlotRole, Snapshot},
};
use std::{cmp::Ordering, collections::BTreeSet};
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// "Check in menu", off by default.
    pub check_menu: bool,
    pub best_only: bool,
    pub keep_open: bool,
    pub shuffle: bool,
    pub blacklist: BTreeSet<String>,
}
#[derive(Clone, Debug)]
pub enum Effect {
    Request(Request),
    Close,
}
/// Snapshot index of chest slot 0.
const CHEST: u16 = 300;
/// One client tick: how soon an unconfirmed click or an emptied queue is
/// looked at again.
const TICK: u64 = 50;
/// The wait before the next step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Wait {
    /// The click delay plus this many milliseconds.
    Click(u64),
    Tick,
}
#[derive(Debug)]
pub struct ChestSteal {
    gate: Gate,
    /// `pendingSlots`: chest-relative slot indexes.
    queued: Vec<u16>,
    /// `queueInitialized`.
    initialized: bool,
    /// `previousSlot`; `None` is the reference's -1.
    previous: Option<u16>,
    /// `waitingToClose`.
    closing: bool,
    wait: Wait,
    seed: u64,
}
impl Default for ChestSteal {
    fn default() -> Self {
        Self {
            gate: Gate::default(),
            queued: Vec::new(),
            initialized: false,
            previous: None,
            closing: false,
            wait: Wait::Click(0),
            seed: 0,
        }
    }
}
impl ChestSteal {
    /// `resetState`.
    pub fn cancel(&mut self) {
        self.gate.clear();
        self.queued.clear();
        self.initialized = false;
        self.previous = None;
        self.closing = false;
        self.wait = Wait::Click(0);
    }
    pub fn active(&self) -> bool {
        self.gate.pending().is_some() || !self.queued.is_empty() || self.closing
    }
    /// The wait before the next step, given the click delay range.
    pub fn delay(&self, (min, max): (u64, u64)) -> (u64, u64) {
        match self.wait {
            Wait::Click(extra) => (min + extra, max + extra),
            Wait::Tick => (TICK, TICK),
        }
    }
    pub fn step(&mut self, snapshot: &Snapshot, options: &Options) -> Vec<Effect> {
        self.wait = Wait::Click(0);
        if self.gate.interrupted(snapshot)
            || !snapshot.screen_open
            || !snapshot.is_chest
            || options.check_menu && !snapshot.chest_title_standard
        {
            self.cancel();
            return Vec::new();
        }
        match self.gate.observe(snapshot) {
            Outcome::Waiting => {
                self.wait = Wait::Tick;
                return Vec::new();
            }
            Outcome::Invalidated => {
                self.cancel();
                return Vec::new();
            }
            // The quick-move moved nothing (no room for that stack). The
            // reference's click is spent the same way and the queue goes
            // on; each queued slot is clicked at most once, so dropping the
            // gate's stall memory cannot repeat a click.
            Outcome::Failed(_) => self.gate = Gate::default(),
            Outcome::Idle | Outcome::Applied => {}
        }
        if self.closing {
            self.cancel();
            return vec![Effect::Close];
        }
        if !options.keep_open && inventory_full(snapshot) {
            self.queued.clear();
            self.close();
            return Vec::new();
        }
        if !self.initialized {
            self.initialize(snapshot, options);
            match self.queued.first() {
                // The first click also waits out the action delay
                // (`actionDelayTimer`, click delay + 100).
                Some(&first) => self.wait = Wait::Click(travel(None, first).max(100)),
                None if !options.keep_open => self.close(),
                None => {}
            }
            return Vec::new();
        }
        if self.queued.is_empty() {
            self.previous = None;
            if !options.keep_open {
                self.close();
            }
            return Vec::new();
        }
        let slot = self.queued.remove(0);
        self.previous = Some(slot);
        self.wait = match self.queued.first() {
            Some(&next) => Wait::Click(travel(Some(slot), next)),
            None => Wait::Tick,
        };
        // A slot emptied since the queue was built costs its click delay
        // without a click, like the reference's `hasStack` check.
        let Some(source) = snapshot
            .slot(CHEST + slot)
            .filter(|s| s.role == SlotRole::Container && s.item.is_some())
        else {
            return Vec::new();
        };
        self.gate
            .issue(
                snapshot,
                Request::for_slot(snapshot, source, Click::QuickMove),
            )
            .map(Effect::Request)
            .into_iter()
            .collect()
    }
    /// `waitingToClose`: the close follows the click delay plus 200.
    fn close(&mut self) {
        self.closing = true;
        self.wait = Wait::Click(200);
    }
    fn initialize(&mut self, snapshot: &Snapshot, options: &Options) {
        self.queued.clear();
        let chest = snapshot
            .slots(|role| role == SlotRole::Container)
            .filter_map(|s| Some((s.index.checked_sub(CHEST)?, s.item.as_ref()?)));
        let blacklisted = |item: &Item| options.blacklist.contains(&item.fingerprint.id);
        if options.best_only {
            let mut tracker = Tracker::default();
            for (index, item) in chest {
                if blacklisted(item) || tracker.track(item, Some(index)) || item.harmful_splash {
                    continue;
                }
                self.queued.push(index);
            }
            // `scanPlayerInventory`: the player's own items, at index -1.
            for slot in snapshot.slots(|role| {
                matches!(
                    role,
                    SlotRole::Armor(_) | SlotRole::Main | SlotRole::Hotbar(_)
                )
            }) {
                if let Some(item) = &slot.item {
                    tracker.track(item, None);
                }
            }
            self.queued.extend(tracker.best());
        } else {
            self.queued.extend(
                chest
                    .filter(|(_, item)| !blacklisted(item) && !item.harmful_splash)
                    .map(|(index, _)| index),
            );
        }
        if options.shuffle {
            self.shuffle();
            let from = self.previous;
            java_sort(&mut self.queued, |a, b| compare(from, *a, *b));
        }
        self.initialized = true;
    }
    /// `Collections.shuffle`.
    fn shuffle(&mut self) {
        for n in (1..self.queued.len()).rev() {
            self.next_seed();
            self.queued.swap(n, self.seed as usize % (n + 1));
        }
    }
    fn next_seed(&mut self) {
        if self.seed == 0 {
            self.seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos() as u64)
                | 1;
        }
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
    }
}
/// `isInventoryFull`: no empty main or hotbar slot.
fn inventory_full(snapshot: &Snapshot) -> bool {
    !snapshot
        .slots(|role| matches!(role, SlotRole::Main | SlotRole::Hotbar(_)))
        .any(|s| s.item.is_none())
}

/// Gear kinds in `getBestSlotIndexes` order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Sword,
    Boots,
    Leggings,
    Chestplate,
    Helmet,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Bow,
}
const KINDS: usize = 10;
/// `ChestStealBestSlotTracker`: the best slot and score of each kind. A
/// slot of `None` is the reference's -1 (the player's own item, or none).
#[derive(Debug)]
struct Tracker([(Option<u16>, f64); KINDS]);
impl Default for Tracker {
    fn default() -> Self {
        Self([(None, -1.0); KINDS])
    }
}
impl Tracker {
    /// `trackCandidate`: false for anything that is not gear.
    fn track(&mut self, item: &Item, slot: Option<u16>) -> bool {
        let Some((kind, score)) = gear(item) else {
            return false;
        };
        // `ChestStealSlotScore.updateIfHigher`.
        let best = &mut self.0[kind as usize];
        if score > best.1 {
            *best = (slot, score);
        }
        true
    }
    /// `getBestSlotIndexes`: chest slots only.
    fn best(&self) -> impl Iterator<Item = u16> + '_ {
        self.0.iter().filter_map(|(slot, _)| *slot)
    }
}
/// The gear kind and its score (`ItemStackScoreUtil`), checked in the
/// tracker's order: armor (by equip slot), sword, bow, pickaxe, axe,
/// shovel, hoe.
fn gear(item: &Item) -> Option<(Kind, f64)> {
    let ench = |id: &str| f64::from(item.meta.enchantment(id));
    if let Some(slot) = item.armor {
        let kind = match slot {
            ArmorSlot::Head => Kind::Helmet,
            ArmorSlot::Chest => Kind::Chestplate,
            ArmorSlot::Legs => Kind::Leggings,
            ArmorSlot::Feet => Kind::Boots,
        };
        let score = f64::from(item.meta.armor_points)
            + ench("protection")
            + f64::from(item.meta.armor_toughness)
            + 0.1 * (ench("feather_falling") + ench("fire_protection") + ench("blast_protection"));
        return Some((kind, score));
    }
    let id = item.fingerprint.id.as_str();
    // The tool base: attack damage of items with a `tool` component.
    let tool = if item.meta.tool {
        f64::from(item.score)
    } else {
        0.0
    };
    let digger = || {
        if tool == 0.0 {
            return 0.0;
        }
        let efficiency = item.meta.enchantment("efficiency");
        let efficiency = if efficiency > 0 {
            f64::from(efficiency * efficiency + 1)
        } else {
            0.0
        };
        tool + efficiency + 0.1 * (ench("unbreaking") + ench("fortune") + ench("mending"))
    };
    Some(if id.contains("_sword") {
        (Kind::Sword, Category::WeaponBest.score(item)?)
    } else if id == "minecraft:bow" {
        (Kind::Bow, Category::BowBest.score(item)?)
    } else if id.contains("_pickaxe") {
        (Kind::Pickaxe, digger())
    } else if id.contains("_axe") {
        let score = if item.meta.tool {
            weapon_damage(item)
                + 0.1 * ench("efficiency")
                + 0.01 * (ench("unbreaking") + ench("fortune") + ench("smite"))
                + 0.01 * ench("bane_of_arthropods")
                + 0.1 * ench("mending")
        } else {
            0.0
        };
        (Kind::Axe, score)
    } else if id.contains("_shovel") {
        (Kind::Shovel, digger())
    } else if id.contains("_hoe") {
        let score = if tool == 0.0 {
            0.0
        } else {
            tool + 0.1 * (ench("unbreaking") + ench("mending"))
        };
        (Kind::Hoe, score)
    } else {
        return None;
    })
}

/// A chest slot's grid cell. Java's `-1 % 9` is -1 and `(-1 - -1) / 9` is
/// 0, so the reference's "no previous slot" sits left of the first row.
fn cell(slot: Option<u16>) -> (f64, f64) {
    slot.map_or((-1.0, 0.0), |s| (f64::from(s % 9), f64::from(s / 9)))
}
fn distance(a: Option<u16>, b: u16) -> f64 {
    let ((ax, ay), (bx, by)) = (cell(a), cell(Some(b)));
    ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt()
}
/// `calculateSlotTravelDelay`: nothing under 3 cells, else 30 ms per cell.
fn travel(from: Option<u16>, to: u16) -> u64 {
    let d = distance(from, to);
    if d < 3.0 { 0 } else { (d * 30.0) as u64 }
}
/// `ChestStealSlotDistanceComparator`: the second operand's distance gets
/// +1.0, so slots within one cell of each other never compare equal and the
/// order is not consistent; [`java_sort`] reproduces what Java does with it.
fn compare(from: Option<u16>, a: u16, b: u16) -> Ordering {
    distance(from, a)
        .partial_cmp(&(distance(from, b) + 1.0))
        .unwrap_or(Ordering::Equal)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        planner::{DEADLINE, tests::item},
        types::{ItemClass, Slot},
    };

    fn slot(index: u16, role: SlotRole, item: Option<Item>) -> Slot {
        Slot { index, item, role }
    }
    /// A chest holding `items` and a player inventory with `free` empty
    /// main slots (the rest hold dirt).
    fn chest(items: Vec<Option<Item>>, free: usize) -> Snapshot {
        let mut slots: Vec<Slot> = (0..36u16)
            .map(|n| {
                let role = if n < 9 {
                    SlotRole::Hotbar(n as u8)
                } else {
                    SlotRole::Main
                };
                let empty = n >= 36 - free as u16;
                slot(n, role, (!empty).then(|| item("minecraft:dirt", 64)))
            })
            .collect();
        for (i, part) in [
            ArmorSlot::Head,
            ArmorSlot::Chest,
            ArmorSlot::Legs,
            ArmorSlot::Feet,
        ]
        .into_iter()
        .enumerate()
        {
            slots.push(slot(500 + i as u16, SlotRole::Armor(part), None));
        }
        slots.extend(
            items
                .into_iter()
                .enumerate()
                .map(|(i, item)| slot(CHEST + i as u16, SlotRole::Container, item)),
        );
        Snapshot {
            session: 1,
            container_id: 3,
            revision: 5,
            slots,
            carried: None,
            screen_open: true,
            is_player_inventory: false,
            is_chest: true,
            chest_title_standard: true,
            vitals: Default::default(),
            observed: std::time::Instant::now(),
        }
    }
    fn stone() -> Option<Item> {
        Some(item("minecraft:stone", 64))
    }
    fn sword(id: &str, damage: f32) -> Item {
        let mut out = item(id, 1);
        out.class = ItemClass::Sword;
        out.score = damage;
        out.meta.tool = true;
        out
    }
    fn armor(id: &str, part: ArmorSlot, points: f32, toughness: f32) -> Item {
        let mut out = item(id, 1);
        out.armor = Some(part);
        out.meta.armor_points = points;
        out.meta.armor_toughness = toughness;
        out
    }
    /// The player takes the stack: the chest slot empties.
    fn take(s: &mut Snapshot, index: u16) {
        s.slots.iter_mut().find(|x| x.index == index).unwrap().item = None;
        s.revision += 1;
    }
    /// Runs the queue to its end, taking each clicked stack; returns the
    /// clicked slots and whether the chest was closed.
    fn drain(steal: &mut ChestSteal, s: &mut Snapshot, options: &Options) -> (Vec<u16>, bool) {
        let mut clicked = Vec::new();
        for _ in 0..80 {
            match steal.step(s, options).as_slice() {
                [Effect::Request(r)] => {
                    clicked.push(r.slot);
                    take(s, r.slot);
                }
                [Effect::Close] => return (clicked, true),
                _ if !steal.active() => return (clicked, false),
                _ => {}
            }
        }
        panic!("no end: {clicked:?}");
    }

    #[test]
    fn takes_everything_but_blacklisted_and_harmful_then_closes() {
        let mut potion = item("minecraft:splash_potion", 1);
        potion.harmful_splash = true;
        let options = Options {
            blacklist: ["minecraft:stick".to_owned()].into(),
            ..Options::default()
        };
        let mut s = chest(
            vec![
                stone(),
                Some(item("minecraft:stick", 3)),
                None,
                Some(potion),
                Some(sword("minecraft:wooden_sword", 3.0)),
            ],
            9,
        );
        let mut steal = ChestSteal::default();
        assert_eq!(drain(&mut steal, &mut s, &options), (vec![300, 304], true));
        assert!(!steal.active());
    }

    #[test]
    fn best_only_takes_non_gear_and_only_upgrades() {
        // The player holds a diamond sword and wears an iron helmet; the
        // chest has a worse sword, a better helmet and blocks.
        let mut s = chest(
            vec![
                Some(sword("minecraft:stone_sword", 4.0)),
                Some(armor("minecraft:diamond_helmet", ArmorSlot::Head, 3.0, 2.0)),
                Some(item("minecraft:cobblestone", 32)),
                Some(armor("minecraft:golden_boots", ArmorSlot::Feet, 1.0, 0.0)),
            ],
            9,
        );
        s.slots[0].item = Some(sword("minecraft:diamond_sword", 6.0));
        s.slots[36].item = Some(armor("minecraft:iron_helmet", ArmorSlot::Head, 2.0, 0.0));
        let options = Options {
            best_only: true,
            ..Options::default()
        };
        let mut steal = ChestSteal::default();
        // Non-gear first, then the best of each kind in tracker order
        // (boots before helmet); the stone sword stays.
        assert_eq!(
            drain(&mut steal, &mut s, &options),
            (vec![302, 303, 301], true)
        );
        assert!(s.slot(300).unwrap().item.is_some());
    }

    #[test]
    fn tracker_order_ties_and_scores() {
        let mut t = Tracker::default();
        let bow = item("minecraft:bow", 1);
        let pick = {
            let mut p = item("minecraft:iron_pickaxe", 1);
            p.score = 3.0;
            p.meta.tool = true;
            p.meta.enchantments = vec![("minecraft:efficiency".into(), 2)];
            p
        };
        let hoe = item("minecraft:iron_hoe", 1);
        assert!(!t.track(&item("minecraft:cobblestone", 1), Some(0)));
        for (n, gear) in [bow, pick.clone(), hoe, sword("minecraft:iron_sword", 5.0)]
            .iter()
            .enumerate()
        {
            assert!(t.track(gear, Some(n as u16)));
        }
        // An equal player item does not displace the chest's (strict >).
        t.track(&sword("minecraft:iron_sword", 5.0), None);
        assert_eq!(t.best().collect::<Vec<_>>(), vec![3, 1, 2, 0]);
        // Pickaxe: 3 damage + efficiency 2·2+1.
        assert_eq!(gear(&pick), Some((Kind::Pickaxe, 8.0)));
        // A hoe's zero attack damage scores 0, still above the -1 start.
        assert_eq!(gear(&item("minecraft:iron_hoe", 1)), Some((Kind::Hoe, 0.0)));
        // A better player sword clears the chest's slot.
        t.track(&sword("minecraft:netherite_sword", 7.0), None);
        assert_eq!(t.best().collect::<Vec<_>>(), vec![1, 2, 0]);
    }

    #[test]
    fn travel_delay_and_fuzzy_distance_sort() {
        assert_eq!(travel(None, 0), 0);
        assert_eq!(travel(None, 1), 0);
        assert_eq!(travel(None, 2), 90);
        // (0,0) to (8,2): √68 cells.
        assert_eq!(travel(Some(0), 26), 247);
        let sort = |mut list: Vec<u16>| {
            java_sort(&mut list, |a, b| compare(None, *a, *b));
            list
        };
        assert_eq!(sort(vec![0, 26, 13]), vec![0, 13, 26]);
        assert_eq!(sort(vec![2, 1, 0]), vec![0, 1, 2]);
        // Within one cell the comparator calls every pair "less": a
        // two-slot run is reversed whatever its order.
        assert_eq!(sort(vec![1, 10]), vec![10, 1]);
        assert_eq!(sort(vec![10, 1]), vec![1, 10]);
    }

    #[test]
    fn shuffle_keeps_every_slot_and_starts_near_the_corner() {
        let options = Options {
            shuffle: true,
            ..Options::default()
        };
        let mut orders = BTreeSet::new();
        for seed in 1..200 {
            let mut s = chest((0..27).map(|_| stone()).collect(), 27);
            let mut steal = ChestSteal {
                seed,
                ..ChestSteal::default()
            };
            let (clicked, closed) = drain(&mut steal, &mut s, &options);
            assert!(closed);
            let mut sorted = clicked.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, (300..327).collect::<Vec<_>>());
            // The first click is within two cells of the top-left corner.
            assert!(distance(None, clicked[0] - CHEST) <= 2.0, "{clicked:?}");
            orders.insert(clicked);
        }
        // The order is random, not one fixed permutation.
        assert!(orders.len() > 150, "{}", orders.len());
    }

    #[test]
    fn first_click_waits_the_action_delay_and_far_clicks_add_travel() {
        let mut s = chest(
            vec![stone(), None, None, None, None, None, None, None, stone()],
            9,
        );
        let options = Options::default();
        let mut steal = ChestSteal::default();
        assert!(steal.step(&s, &options).is_empty());
        assert_eq!(steal.delay((150, 200)), (250, 300));
        let effects = steal.step(&s, &options);
        let [Effect::Request(r)] = effects.as_slice() else {
            panic!("click")
        };
        assert_eq!(r.slot, 300);
        // Slot 0 to slot 8: 8 cells.
        assert_eq!(steal.delay((150, 200)), (390, 440));
        take(&mut s, 300);
        assert_eq!(steal.step(&s, &options).len(), 1);
        // The queue is empty: one tick to notice, then the close waits.
        assert_eq!(steal.delay((150, 200)), (TICK, TICK));
        take(&mut s, 308);
        assert!(steal.step(&s, &options).is_empty());
        assert_eq!(steal.delay((150, 200)), (350, 400));
        assert!(matches!(
            steal.step(&s, &options).as_slice(),
            [Effect::Close]
        ));
    }

    #[test]
    fn full_inventory_closes_without_keep_open() {
        let mut steal = ChestSteal::default();
        let s = chest(vec![stone(), stone()], 0);
        assert!(steal.step(&s, &Options::default()).is_empty());
        assert!(steal.active());
        assert!(matches!(
            steal.step(&s, &Options::default()).as_slice(),
            [Effect::Close]
        ));
        assert!(!steal.active());
    }

    #[test]
    fn keep_open_clicks_past_a_failed_slot_and_never_rescans() {
        let options = Options {
            keep_open: true,
            ..Options::default()
        };
        let mut steal = ChestSteal::default();
        let mut s = chest(vec![stone(), stone(), None], 0);
        assert!(steal.step(&s, &options).is_empty());
        assert!(
            matches!(steal.step(&s, &options).as_slice(), [Effect::Request(r)] if r.slot == 300)
        );
        // No room: the click times out and the next slot is still tried.
        s.observed += DEADLINE;
        assert!(
            matches!(steal.step(&s, &options).as_slice(), [Effect::Request(r)] if r.slot == 301)
        );
        s.observed += DEADLINE;
        assert!(steal.step(&s, &options).is_empty());
        assert!(steal.step(&s, &options).is_empty());
        assert!(!steal.active());
        // A new stack in the open chest is not picked up...
        s.slots.last_mut().unwrap().item = stone();
        s.slots[35].item = None;
        s.revision += 1;
        for _ in 0..3 {
            assert!(steal.step(&s, &options).is_empty());
        }
        // ...until the chest is opened again.
        s.screen_open = false;
        assert!(steal.step(&s, &options).is_empty());
        s.screen_open = true;
        s.container_id += 1;
        assert!(steal.step(&s, &options).is_empty());
        assert!(matches!(
            steal.step(&s, &options).as_slice(),
            [Effect::Request(_)]
        ));
    }

    #[test]
    fn check_in_menu_is_off_by_default() {
        let mut s = chest(vec![stone()], 9);
        s.chest_title_standard = false;
        let mut steal = ChestSteal::default();
        assert!(steal.step(&s, &Options::default()).is_empty());
        assert!(matches!(
            steal.step(&s, &Options::default()).as_slice(),
            [Effect::Request(_)]
        ));
        let strict = Options {
            check_menu: true,
            ..Options::default()
        };
        assert!(steal.step(&s, &strict).is_empty());
        assert!(!steal.active());
    }
}
