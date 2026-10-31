// Hotbar use sequences for Throwpot, ThrowDebuff and AutoHeal (Legit): pick
// hotbar slots, then select each slot, press use, optionally drop the held
// bowl, wait, and finally restore the saved slot. Everything here is pure;
// the runtime owns the hotbar/use claims and the host applies the effects
// through the ordinary selected-slot and use-input paths.
use super::super::super::inventory::types::{Healing, Item, Snapshot};
use std::collections::VecDeque;

/// Press-to-release time of the reference's simulated key presses.
pub const PRESS_MS: u64 = 51;
/// One client tick; ThrowDebuff runs on `EventPreTick`.
pub const TICK_MS: u64 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PotKind {
    Both,
    Pots,
    Soup,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebuffMode {
    All,
    OneOfEach,
    First,
}
/// ThrowDebuff's toggles in reference order; each matches potions whose
/// display name contains it.
pub const DEBUFFS: [&str; 4] = ["Harming", "Weakness", "Poison", "Slowness"];

pub fn is_soup(item: &Item) -> bool {
    item.fingerprint.id == "minecraft:mushroom_stew"
}
pub fn is_bowl(item: &Item) -> bool {
    item.fingerprint.id == "minecraft:bowl"
}
/// `ItemSplashPotion.isSplashPotion`: splash, not lingering, potions.
fn is_splash(item: &Item) -> bool {
    item.fingerprint.id == "minecraft:splash_potion"
}
/// `ItemPotion` instances: drinkable, splash and lingering potions.
fn is_potion(item: &Item) -> bool {
    matches!(
        item.fingerprint.id.as_str(),
        "minecraft:potion" | "minecraft:splash_potion" | "minecraft:lingering_potion"
    )
}

/// One planned use: the hotbar slot and whether to drop the held item after
/// (Throwpot's "Throw bowls" for soup).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Use {
    pub slot: u8,
    pub drop_after: bool,
}

/// Throwpot's `collectHealingItems`. `order` is the hotbar scan order
/// (shuffled with "Random"). Soup is the mushroom-stew food (8 health) and
/// pots are splash potions with instant health or regeneration, healing
/// `4 * (amplifier + 1)` of their first effect. Dynamic skips any item that
/// would heal past max health; Single takes the first eligible item.
pub fn throwpot_plan(
    snapshot: &Snapshot,
    kind: PotKind,
    dynamic: bool,
    throw_bowls: bool,
    order: &[u8],
) -> Vec<Use> {
    let soup = matches!(kind, PotKind::Both | PotKind::Soup);
    let pots = matches!(kind, PotKind::Both | PotKind::Pots);
    let health = snapshot.vitals.health.floor();
    let max = snapshot.vitals.max_health;
    let mut queued = 0.0;
    let mut out = Vec::new();
    for &slot in order {
        let Some(item) = snapshot.hotbar(slot).and_then(|s| s.item.as_ref()) else {
            continue;
        };
        if soup && item.healing == Some(Healing::Soup) {
            let planned = Use {
                slot,
                drop_after: throw_bowls,
            };
            if !dynamic {
                out.push(planned);
                break;
            }
            if queued + 8.0 + health <= max {
                queued += 8.0;
                out.push(planned);
            }
            continue;
        }
        if !pots || !is_splash(item) || item.healing != Some(Healing::Potion) {
            continue;
        }
        let planned = Use {
            slot,
            drop_after: false,
        };
        if !dynamic {
            out.push(planned);
            break;
        }
        let amplifier = item.meta.effects.first().map_or(0, |e| e.amplifier.max(0));
        let heal = 4.0 * (amplifier + 1) as f32;
        if queued + heal + health <= max {
            queued += heal;
            out.push(planned);
        }
    }
    out
}

/// ThrowDebuff's `collectDebuffs`: potions (drinkable, splash or lingering)
/// whose lowercase display name contains an enabled debuff's name, in
/// hotbar order.
pub fn debuff_plan(snapshot: &Snapshot, mode: DebuffMode, enabled: [bool; 4]) -> Vec<Use> {
    let mut queued = [false; 4];
    let mut out = Vec::new();
    for slot in 0..9 {
        let Some(item) = snapshot.hotbar(slot).and_then(|s| s.item.as_ref()) else {
            continue;
        };
        if !is_potion(item) {
            continue;
        }
        let name = item.meta.hover_name.to_lowercase();
        let found = DEBUFFS.iter().enumerate().position(|(i, label)| {
            enabled[i]
                && !(mode == DebuffMode::OneOfEach && queued[i])
                && name.contains(&label.to_lowercase())
        });
        let Some(index) = found else {
            continue;
        };
        queued[index] = true;
        out.push(Use {
            slot,
            drop_after: false,
        });
        if mode == DebuffMode::First {
            break;
        }
    }
    out
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Select(u8),
    /// Press use (one activation).
    Use,
    /// Drop one held item.
    Drop,
    Wait(u64),
}

/// Effects the host applies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Select(u8),
    Use,
    Drop,
}

/// Throwpot's `selectHotbarSlot`: with "Scroll" it selects the current slot
/// and each slot on the way to `to`, every one followed by the scroll delay;
/// without it a single selection.
fn walk(out: &mut VecDeque<Action>, from: u8, to: u8, scroll: Option<u64>) {
    let Some(delay) = scroll else {
        out.push_back(Action::Select(to));
        return;
    };
    let mut current = from;
    loop {
        out.push_back(Action::Select(current));
        out.push_back(Action::Wait(delay));
        match to.cmp(&current) {
            std::cmp::Ordering::Greater => current += 1,
            std::cmp::Ordering::Less => current -= 1,
            std::cmp::Ordering::Equal => break,
        }
    }
}

/// Throwpot's `onScheduledAction`: for each item select, use, (drop), then
/// a fresh Delay sample; finally the saved slot. Items are stably sorted by
/// their distance to the saved slot (`ItemDataComparator`), so equally
/// distant items keep their scan order.
pub fn sequence(
    mut uses: Vec<Use>,
    saved: u8,
    mut delay: impl FnMut() -> u64,
    scroll: Option<u64>,
) -> VecDeque<Action> {
    uses.sort_by_key(|u| u.slot.abs_diff(saved));
    let mut out = VecDeque::new();
    let mut current = saved;
    for planned in uses {
        walk(&mut out, current, planned.slot, scroll);
        current = planned.slot;
        out.push_back(Action::Use);
        out.push_back(Action::Wait(PRESS_MS));
        if planned.drop_after {
            out.push_back(Action::Drop);
            out.push_back(Action::Wait(PRESS_MS));
        }
        out.push_back(Action::Wait(delay()));
    }
    walk(&mut out, current, saved, scroll);
    out
}

/// ThrowDebuff's tick loop (`onTick`), laid out ahead of time one client
/// tick apart. Each tick either releases the previous press, or, once a
/// fresh Delay sample has elapsed since the last press, moves toward the
/// next item and presses use when there; with the queue empty it moves back
/// to the saved slot and stops. Items are thrown in hotbar order. With
/// "Scroll" each tick steps one slot, once the scroll delay has passed
/// since the last arrival (`selectHotbarSlotIncrementally`). Both timers
/// start elapsed, as they do on a first use in the reference.
pub fn debuff_sequence(
    uses: Vec<Use>,
    saved: u8,
    mut delay: impl FnMut() -> u64,
    scroll: Option<u64>,
) -> VecDeque<Action> {
    struct Ticks {
        out: VecDeque<Action>,
        current: u8,
        scroll: Option<u64>,
        arrived: Option<u64>,
    }
    impl Ticks {
        fn select(&mut self, target: u8, now: u64) -> bool {
            let Some(delay) = self.scroll else {
                self.out.push_back(Action::Select(target));
                self.current = target;
                return true;
            };
            if !since(self.arrived, now, delay) {
                return false;
            }
            match target.cmp(&self.current) {
                std::cmp::Ordering::Equal => {
                    self.arrived = Some(now);
                    return true;
                }
                std::cmp::Ordering::Greater => self.current += 1,
                std::cmp::Ordering::Less => self.current -= 1,
            }
            self.out.push_back(Action::Select(self.current));
            false
        }
    }
    fn since(at: Option<u64>, now: u64, ms: u64) -> bool {
        at.is_none_or(|at| now - at >= ms)
    }
    let mut ticks = Ticks {
        out: VecDeque::new(),
        current: saved,
        scroll,
        arrived: None,
    };
    let mut queue: VecDeque<u8> = uses.into_iter().map(|u| u.slot).collect();
    let (mut pressed, mut throwing) = (None, false);
    // Bounded: every tick either advances the scroll, waits out a timer of
    // at most a few hundred milliseconds or consumes the queue.
    for tick in 0..10_000u64 {
        let now = tick * TICK_MS;
        if throwing {
            throwing = false;
        } else if let Some(&slot) = queue.front() {
            if since(pressed, now, delay()) && ticks.select(slot, now) {
                ticks.out.push_back(Action::Use);
                throwing = true;
                pressed = Some(now);
                queue.pop_front();
            }
        } else if ticks.select(saved, now) {
            break;
        }
        ticks.out.push_back(Action::Wait(TICK_MS));
    }
    ticks.out
}

/// Plays a sequence against a millisecond clock.
#[derive(Debug, Default)]
pub struct Sequencer {
    queue: VecDeque<Action>,
    due: u64,
    /// The slot the sequence last selected.
    pub expected: Option<u8>,
}
impl Sequencer {
    pub fn start(&mut self, queue: VecDeque<Action>, now: u64) {
        self.queue = queue;
        self.due = now;
        self.expected = None;
    }
    pub fn active(&self) -> bool {
        !self.queue.is_empty()
    }
    pub fn cancel(&mut self) {
        self.queue.clear();
        self.expected = None;
    }
    /// Effects due at `now`, stopping at the next wait.
    pub fn tick(&mut self, now: u64) -> Vec<Effect> {
        let mut out = Vec::new();
        while now >= self.due {
            let Some(action) = self.queue.pop_front() else {
                break;
            };
            match action {
                Action::Wait(ms) => self.due = now + ms,
                Action::Select(slot) => {
                    self.expected = Some(slot);
                    out.push(Effect::Select(slot));
                }
                Action::Use => out.push(Effect::Use),
                Action::Drop => out.push(Effect::Drop),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        rules::tests::item,
        session::tests::snapshot,
        types::{ItemClass, PotionEffect},
    };

    fn pot(amplifier: i32, effect: &str) -> Item {
        let mut i = item("splash_potion", ItemClass::Other, 0.0);
        i.meta.effects.push(PotionEffect {
            id: effect.into(),
            amplifier,
            duration: 1,
        });
        if matches!(effect, "instant_health" | "regeneration") {
            i.healing = Some(Healing::Potion);
        }
        i
    }
    fn stew() -> Item {
        let mut i = item("mushroom_stew", ItemClass::Other, 0.0);
        i.healing = Some(Healing::Soup);
        i
    }
    fn named(id: &str, name: &str) -> Item {
        let mut i = item(id, ItemClass::Other, 0.0);
        i.meta.hover_name = name.into();
        i
    }
    #[test]
    fn dynamic_throwpot_stops_before_overhealing() {
        let mut s = snapshot(false, std::time::Instant::now());
        s.vitals.health = 9.5;
        s.vitals.max_health = 20.0;
        s.slots[1].item = Some(stew());
        s.slots[2].item = Some(pot(0, "instant_health"));
        s.slots[3].item = Some(pot(1, "instant_health"));
        s.slots[4].item = Some(pot(0, "poison"));
        s.slots[5].item = Some(pot(0, "regeneration"));
        s.slots[6].item = Some(item("beetroot_soup", ItemClass::Other, 0.0));
        let order: Vec<u8> = (0..9).collect();
        let plan = throwpot_plan(&s, PotKind::Both, true, true, &order);
        // 9 + 8 = 17; +4 = 21 and +8 = 25 overheal and are skipped, the
        // poison never counts, and so would regeneration's +4.
        assert_eq!(
            plan,
            vec![Use {
                slot: 1,
                drop_after: true
            }]
        );
        // Pots only: 9 + 4 = 13, +8 = 21 skipped, regeneration +4 = 17.
        let pots = throwpot_plan(&s, PotKind::Pots, true, true, &order);
        assert_eq!(pots.iter().map(|u| u.slot).collect::<Vec<_>>(), vec![2, 5]);
        let single = throwpot_plan(&s, PotKind::Pots, false, true, &order);
        assert_eq!(
            single,
            vec![Use {
                slot: 2,
                drop_after: false
            }]
        );
        // Single takes soup without a health check.
        s.vitals.health = 20.0;
        let soup = throwpot_plan(&s, PotKind::Soup, false, false, &order);
        assert_eq!(
            soup,
            vec![Use {
                slot: 1,
                drop_after: false
            }]
        );
        assert!(throwpot_plan(&s, PotKind::Soup, true, false, &order).is_empty());
    }
    #[test]
    fn throwpot_sequence_keeps_ties_and_scrolls_from_the_current_slot() {
        let uses = [5, 3].map(|slot| Use {
            slot,
            drop_after: false,
        });
        let seq = sequence(uses.to_vec(), 4, || 90, None);
        assert_eq!(
            seq.iter().copied().collect::<Vec<_>>(),
            vec![
                Action::Select(5),
                Action::Use,
                Action::Wait(PRESS_MS),
                Action::Wait(90),
                Action::Select(3),
                Action::Use,
                Action::Wait(PRESS_MS),
                Action::Wait(90),
                Action::Select(4),
            ]
        );
        let scrolled = sequence(uses[1..].to_vec(), 1, || 0, Some(10));
        assert_eq!(
            scrolled.iter().take(6).copied().collect::<Vec<_>>(),
            vec![
                Action::Select(1),
                Action::Wait(10),
                Action::Select(2),
                Action::Wait(10),
                Action::Select(3),
                Action::Wait(10),
            ]
        );
    }
    #[test]
    fn debuffs_match_display_names_in_each_mode() {
        let mut s = snapshot(false, std::time::Instant::now());
        s.slots[0].item = Some(named("splash_potion", "Splash Potion of Poison"));
        s.slots[1].item = Some(named("lingering_potion", "Lingering Potion of Poison"));
        s.slots[2].item = Some(named("potion", "Potion of Weakness"));
        s.slots[3].item = Some(named("splash_potion", "Splash Potion of Healing"));
        s.slots[5].item = Some(named("splash_potion", "Splash Potion of Harming"));
        s.slots[6].item = Some(named("stone", "Harming stone"));
        let slots = |mode, enabled| {
            debuff_plan(&s, mode, enabled)
                .iter()
                .map(|u| u.slot)
                .collect::<Vec<_>>()
        };
        assert_eq!(slots(DebuffMode::OneOfEach, [true; 4]), vec![0, 2, 5]);
        assert_eq!(slots(DebuffMode::All, [true; 4]), vec![0, 1, 2, 5]);
        assert_eq!(slots(DebuffMode::First, [true; 4]), vec![0]);
        assert_eq!(slots(DebuffMode::First, [true, true, false, true]), vec![2]);
    }
    #[test]
    fn debuff_ticks_press_release_and_restore_in_hotbar_order() {
        let uses = [5, 0].map(|slot| Use {
            slot,
            drop_after: false,
        });
        let mut seq = Sequencer::default();
        seq.start(debuff_sequence(uses.to_vec(), 4, || 70, None), 0);
        let mut effects = Vec::new();
        for ms in 0..1000 {
            effects.extend(seq.tick(ms).into_iter().map(|e| (ms, e)));
        }
        // Press at 0, release at 50; 70 ms have passed at 100 (tick 2).
        assert_eq!(
            effects,
            vec![
                (0, Effect::Select(5)),
                (0, Effect::Use),
                (100, Effect::Select(0)),
                (100, Effect::Use),
                (200, Effect::Select(4)),
            ]
        );
        assert!(!seq.active());
        // Scroll: one step per tick, arrival is seen the tick after, and the
        // next walk waits the scroll delay from that arrival.
        let scrolled: Vec<_> = debuff_sequence(uses[1..].to_vec(), 2, || 0, Some(120))
            .into_iter()
            .collect();
        assert_eq!(
            scrolled,
            vec![
                Action::Select(1),
                Action::Wait(TICK_MS),
                Action::Select(0),
                Action::Wait(TICK_MS),
                Action::Use,
                // Release, then 100 of the 120 ms since arriving.
                Action::Wait(TICK_MS),
                Action::Wait(TICK_MS),
                Action::Wait(TICK_MS),
                Action::Select(1),
                Action::Wait(TICK_MS),
                Action::Select(2),
                Action::Wait(TICK_MS),
            ]
        );
    }
}
