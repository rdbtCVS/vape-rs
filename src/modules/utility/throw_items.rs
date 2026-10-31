// Hotbar use sequences for Throwpot, ThrowDebuff and AutoHeal (Legit): pick
// hotbar slots, then select each slot, press use, optionally drop the held
// bowl, wait, and finally restore the saved slot. Everything here is pure;
// the runtime owns the hotbar/use claims and the host applies the effects
// through the ordinary selected-slot and use-input paths.
use super::super::super::inventory::types::{Item, Snapshot};
use std::collections::VecDeque;

/// Press-to-release time of the reference's simulated key presses.
pub const PRESS_MS: u64 = 51;

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
/// Debuff toggles in reference order with the effect each one names.
pub const DEBUFFS: [(&str, &str); 4] = [
    ("Harming", "instant_damage"),
    ("Weakness", "weakness"),
    ("Poison", "poison"),
    ("Slowness", "slowness"),
];

pub fn is_soup(item: &Item) -> bool {
    item.fingerprint.id == "minecraft:mushroom_stew"
}
pub fn is_bowl(item: &Item) -> bool {
    item.fingerprint.id == "minecraft:bowl"
}
/// Splash (not lingering) potion.
fn is_splash(item: &Item) -> bool {
    item.fingerprint.id == "minecraft:splash_potion"
}
/// Health an instant-health splash restores by the reference's formula
/// (`4 * (amplifier + 1)`), or `None` for other items.
pub fn pot_healing(item: &Item) -> Option<f32> {
    if !is_splash(item) {
        return None;
    }
    item.meta
        .effects
        .iter()
        .find(|e| e.id == "instant_health")
        .map(|e| 4.0 * (e.amplifier.max(0) + 1) as f32)
}

/// One planned use: the hotbar slot and whether to drop the held item after
/// (Throwpot's "Throw bowls" for soup).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Use {
    pub slot: u8,
    pub drop_after: bool,
}

/// Throwpot's item choice. `order` is the hotbar scan order (shuffled when
/// "Random" is on). Dynamic adds items until the next one would heal past
/// max health; Single takes the first eligible item.
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
        let heal = if soup && is_soup(item) {
            8.0
        } else if let Some(heal) = pots.then(|| pot_healing(item)).flatten() {
            heal
        } else {
            continue;
        };
        let planned = Use {
            slot,
            drop_after: throw_bowls && is_soup(item),
        };
        if !dynamic {
            return vec![planned];
        }
        if queued + heal + health > max {
            continue;
        }
        queued += heal;
        out.push(planned);
    }
    out
}

/// ThrowDebuff's item choice over splash potions in the hotbar, matched by
/// effect (or by a custom name containing the debuff's name).
pub fn debuff_plan(snapshot: &Snapshot, mode: DebuffMode, enabled: [bool; 4]) -> Vec<Use> {
    let mut queued = [false; 4];
    let mut out = Vec::new();
    for slot in 0..9 {
        let Some(item) = snapshot.hotbar(slot).and_then(|s| s.item.as_ref()) else {
            continue;
        };
        if !is_splash(item) {
            continue;
        }
        let name = item
            .meta
            .custom_name
            .as_deref()
            .unwrap_or("")
            .to_lowercase();
        let found = DEBUFFS.iter().enumerate().position(|(i, (label, effect))| {
            enabled[i]
                && !(mode == DebuffMode::OneOfEach && queued[i])
                && (item.meta.effects.iter().any(|e| e.id == *effect)
                    || name.contains(&label.to_lowercase()))
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

/// Steps from `from` to `to` one slot at a time ("Scroll"), each followed by
/// the scroll delay; without scrolling a single selection.
fn select(out: &mut VecDeque<Action>, from: u8, to: u8, scroll: Option<u64>) {
    let Some(delay) = scroll else {
        out.push_back(Action::Select(to));
        return;
    };
    let mut current = from;
    while current != to {
        current = if to > current {
            current + 1
        } else {
            current - 1
        };
        out.push_back(Action::Select(current));
        out.push_back(Action::Wait(delay));
    }
}

/// The reference's per-item sequence: select, use, (drop), delay; then the
/// saved slot. Items are taken nearest the saved slot first.
pub fn sequence(
    mut uses: Vec<Use>,
    saved: u8,
    mut delay: impl FnMut() -> u64,
    scroll: Option<u64>,
) -> VecDeque<Action> {
    uses.sort_by_key(|u| (u.slot.abs_diff(saved), u.slot));
    let mut out = VecDeque::new();
    let mut current = saved;
    for planned in uses {
        select(&mut out, current, planned.slot, scroll);
        current = planned.slot;
        out.push_back(Action::Use);
        out.push_back(Action::Wait(PRESS_MS));
        if planned.drop_after {
            out.push_back(Action::Drop);
            out.push_back(Action::Wait(PRESS_MS));
        }
        out.push_back(Action::Wait(delay()));
    }
    select(&mut out, current, saved, scroll);
    out
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
        i
    }
    #[test]
    fn dynamic_throwpot_stops_before_overhealing() {
        let mut s = snapshot(false, std::time::Instant::now());
        s.vitals.health = 9.5;
        s.vitals.max_health = 20.0;
        s.slots[1].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        s.slots[2].item = Some(pot(0, "instant_health"));
        s.slots[3].item = Some(pot(1, "instant_health"));
        s.slots[4].item = Some(pot(0, "poison"));
        let order: Vec<u8> = (0..9).collect();
        let plan = throwpot_plan(&s, PotKind::Both, true, true, &order);
        // 9 + 8 = 17, +4 = 21 > 20 skipped, +8 = 25 skipped.
        assert_eq!(
            plan,
            vec![Use {
                slot: 1,
                drop_after: true
            }]
        );
        let single = throwpot_plan(&s, PotKind::Pots, false, true, &order);
        assert_eq!(single[0].slot, 2);
    }
    #[test]
    fn debuffs_one_of_each_and_sequence_restores() {
        let mut s = snapshot(false, std::time::Instant::now());
        s.slots[0].item = Some(pot(0, "poison"));
        s.slots[1].item = Some(pot(0, "poison"));
        s.slots[5].item = Some(pot(0, "instant_damage"));
        let plan = debuff_plan(&s, DebuffMode::OneOfEach, [true; 4]);
        assert_eq!(plan.iter().map(|u| u.slot).collect::<Vec<_>>(), vec![0, 5]);
        let mut seq = Sequencer::default();
        seq.start(sequence(plan, 4, || 100, None), 0);
        let effects: Vec<_> = (0..1000).flat_map(|ms| seq.tick(ms)).collect();
        assert_eq!(
            effects,
            vec![
                Effect::Select(5),
                Effect::Use,
                Effect::Select(0),
                Effect::Use,
                Effect::Select(4)
            ]
        );
        assert!(!seq.active());
        let scrolled = sequence(
            vec![Use {
                slot: 2,
                drop_after: false,
            }],
            0,
            || 0,
            Some(10),
        );
        assert_eq!(scrolled[0], Action::Select(1));
    }
}
