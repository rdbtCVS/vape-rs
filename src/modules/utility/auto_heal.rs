// AutoHeal: Legit mode uses a hotbar healing item (soup, or the custom
// "Healing Items") below the health threshold through the shared use
// sequencer; its inventory side handles empty bowls ("Bowl Mode" Throw or
// Stack into slot 17) and "Replace" (moves healing items from the main
// inventory into empty hotbar slots) one gated click at a time.
use super::super::super::inventory::{
    arrange::place_carried,
    session::{Policy, Session, Step},
    types::{Click, Item, SlotRole, Snapshot},
};
use super::throw_items::{is_bowl, is_soup};
use std::collections::BTreeSet;

/// The reference's bowl stack slot (container slot 17, a main row slot).
pub const BOWL_SLOT: u16 = 17;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BowlMode {
    Stack,
    Throw,
    None,
}
#[derive(Clone, Debug)]
pub struct Options {
    pub soup: bool,
    pub bowl_mode: BowlMode,
    pub custom_items: bool,
    pub healing_items: BTreeSet<String>,
    pub replace: bool,
    /// Health threshold in HP (of 20).
    pub health: f32,
}
pub fn is_healing(item: &Item, options: &Options) -> bool {
    (options.soup && is_soup(item))
        || (options.custom_items && options.healing_items.contains(&item.fingerprint.id))
}
/// The first hotbar healing item when health is at or under the threshold
/// (the reference compares health / max with threshold / 20).
pub fn heal_slot(snapshot: &Snapshot, options: &Options) -> Option<u8> {
    let v = &snapshot.vitals;
    if v.health <= 0.0 || v.health / v.max_health.max(1.0) > options.health / 20.0 {
        return None;
    }
    (0..9).find(|slot| {
        snapshot
            .hotbar(*slot)
            .and_then(|s| s.item.as_ref())
            .is_some_and(|i| is_healing(i, options))
    })
}

/// Next inventory click and whether it may open the inventory (only
/// Replace does, as in the reference).
pub fn next_click(
    snapshot: &Snapshot,
    options: &Options,
    origin: &mut Option<u16>,
) -> Option<((u16, Click), bool)> {
    if snapshot.carried.is_some() {
        let bowl = snapshot.carried.as_ref().is_some_and(is_bowl);
        if bowl
            && *origin != Some(BOWL_SLOT)
            && snapshot.slot(BOWL_SLOT).is_some_and(|s| {
                // Empty, a non-full bowl stack, or another item (the
                // pickup swaps it out; it then goes back to the origin).
                s.item
                    .as_ref()
                    .is_none_or(|i| !is_bowl(i) || i.fingerprint.count < i.max_count)
            })
        {
            return Some(((BOWL_SLOT, Click::Pickup), false));
        }
        return place_carried(snapshot, *origin).map(|c| (c, false));
    }
    if options.soup && options.bowl_mode != BowlMode::None {
        let bowl_slot = snapshot.slot(BOWL_SLOT);
        let stacked = bowl_slot.and_then(|s| s.item.as_ref()).is_some_and(is_bowl);
        for slot in snapshot.slots(|r| matches!(r, SlotRole::Main | SlotRole::Hotbar(_))) {
            let Some(item) = slot.item.as_ref().filter(|i| is_bowl(i)) else {
                continue;
            };
            if options.bowl_mode == BowlMode::Throw {
                return Some(((slot.index, Click::ThrowStack), false));
            }
            if slot.index == BOWL_SLOT {
                continue;
            }
            let full = bowl_slot.and_then(|s| s.item.as_ref()).is_some_and(|i| {
                u16::from(i.fingerprint.count) + u16::from(item.fingerprint.count)
                    > u16::from(i.max_count)
            });
            if stacked && full {
                continue;
            }
            return Some(match (slot.role, stacked) {
                // Hotbar bowl onto slot 17: a number-key swap when 17 holds
                // something else, a shift-click merge when it holds bowls.
                (SlotRole::Hotbar(n), false) => ((BOWL_SLOT, Click::SwapHotbar(n)), false),
                (SlotRole::Hotbar(_), true) => ((slot.index, Click::QuickMove), false),
                _ => {
                    *origin = Some(slot.index);
                    ((slot.index, Click::Pickup), false)
                }
            });
        }
    }
    if options.replace {
        let empty = snapshot
            .slots(|r| matches!(r, SlotRole::Hotbar(_)))
            .find(|s| s.item.is_none());
        let source = snapshot
            .slots(|r| r == SlotRole::Main)
            .find(|s| s.item.as_ref().is_some_and(|i| is_healing(i, options)));
        if let (Some(empty), Some(source)) = (empty, source)
            && let SlotRole::Hotbar(n) = empty.role
        {
            return Some(((source.index, Click::SwapHotbar(n)), true));
        }
    }
    None
}

#[derive(Debug, Default)]
pub struct AutoHeal {
    session: Session,
    origin: Option<u16>,
}
impl AutoHeal {
    pub fn cancel(&mut self) {
        self.session.cancel();
        self.origin = None;
    }
    pub fn opened(&self) -> bool {
        self.session.opened()
    }
    pub fn active(&self) -> bool {
        self.session.active()
    }
    pub fn step(&mut self, snapshot: &Snapshot, options: &Options) -> Step {
        let next = next_click(snapshot, options, &mut self.origin);
        let policy = Policy {
            open: next.is_some_and(|(_, open)| open),
            ..Policy::default()
        };
        let step = self.session.step(snapshot, policy, next.map(|(c, _)| c));
        if step == Step::Done {
            self.origin = None;
        }
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{rules::tests::item, session::tests::snapshot, types::ItemClass};

    fn options() -> Options {
        Options {
            soup: true,
            bowl_mode: BowlMode::Stack,
            custom_items: false,
            healing_items: BTreeSet::new(),
            replace: true,
            health: 17.0,
        }
    }
    fn bowl(count: u8) -> Item {
        let mut i = item("bowl", ItemClass::Other, 0.0);
        i.max_count = 64;
        i.fingerprint.count = count;
        i
    }
    #[test]
    fn heals_below_threshold_and_stacks_bowls() {
        let mut s = snapshot(true, std::time::Instant::now());
        s.slots[20].item = None;
        s.vitals.health = 16.0;
        s.vitals.max_health = 20.0;
        s.slots[3].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        assert_eq!(heal_slot(&s, &options()), Some(3));
        s.vitals.health = 18.0;
        assert_eq!(heal_slot(&s, &options()), None);
        s.slots[2].item = Some(bowl(1));
        assert_eq!(
            next_click(&s, &options(), &mut None),
            Some(((BOWL_SLOT, Click::SwapHotbar(2)), false))
        );
        s.slots[17].item = s.slots[2].item.take();
        s.slots[25].item = Some(bowl(1));
        let mut origin = None;
        assert_eq!(
            next_click(&s, &options(), &mut origin),
            Some(((25, Click::Pickup), false))
        );
        s.carried = s.slots[25].item.take();
        assert_eq!(
            next_click(&s, &options(), &mut origin),
            Some(((BOWL_SLOT, Click::Pickup), false))
        );
    }
    #[test]
    fn replace_moves_soup_to_an_empty_hotbar_slot() {
        let mut s = snapshot(false, std::time::Instant::now());
        for i in 0..9 {
            s.slots[i].item = Some(item("stone", ItemClass::Other, 0.0));
        }
        s.slots[4].item = None;
        s.slots[30].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        assert_eq!(
            next_click(&s, &options(), &mut None),
            Some(((30, Click::SwapHotbar(4)), true))
        );
    }
}
