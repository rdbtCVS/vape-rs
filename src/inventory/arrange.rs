// Click-planning helpers shared by the inventory arrangement planners. All
// functions read one snapshot and return at most one next click; planners
// re-plan from every new snapshot instead of replaying a queued script.
use super::{
    rules::armor_slot_index,
    types::{ArmorSlot, Click, Item, Slot, SlotRole, Snapshot},
};

pub const ARMOR: [ArmorSlot; 4] = [
    ArmorSlot::Head,
    ArmorSlot::Chest,
    ArmorSlot::Legs,
    ArmorSlot::Feet,
];

/// Player storage: main inventory then hotbar (the reference iterates
/// container slots 9..45, which is the same order).
pub fn storage(snapshot: &Snapshot) -> impl Iterator<Item = &Slot> {
    snapshot
        .slots(|r| r == SlotRole::Main)
        .chain(snapshot.slots(|r| matches!(r, SlotRole::Hotbar(_))))
}
pub fn equipped(snapshot: &Snapshot, slot: ArmorSlot) -> Option<&Item> {
    snapshot
        .slot(armor_slot_index(slot))
        .and_then(|s| s.item.as_ref())
}
/// Durability left, for tie-breaking equal scores (more is better).
fn durability_left(item: &Item) -> i32 {
    i32::from(item.max_damage) - i32::from(item.durability_used())
}
/// The best stored piece for `slot` that beats the equipped one, if any.
pub fn better_armor(snapshot: &Snapshot, slot: ArmorSlot) -> Option<&Slot> {
    let current = equipped(snapshot, slot);
    let mut best: Option<&Slot> = None;
    for candidate in storage(snapshot) {
        let Some(item) = candidate.item.as_ref().filter(|i| i.armor == Some(slot)) else {
            continue;
        };
        let beats = |other: &Item| {
            item.score > other.score
                || (item.score == other.score && durability_left(item) > durability_left(other))
        };
        if current.is_some_and(|c| !beats(c)) {
            continue;
        }
        if best.is_none_or(|b| beats(b.item.as_ref().unwrap())) {
            best = Some(candidate);
        }
    }
    best
}
/// The best piece per armor slot across storage and equipment, so cleaners
/// keep it.
pub fn best_armor_slots(snapshot: &Snapshot) -> Vec<u16> {
    ARMOR
        .iter()
        .filter_map(|slot| {
            better_armor(snapshot, *slot)
                .map(|s| s.index)
                .or_else(|| equipped(snapshot, *slot).map(|_| armor_slot_index(*slot)))
        })
        .collect()
}
pub fn first_empty(snapshot: &Snapshot) -> Option<&Slot> {
    storage(snapshot).find(|s| s.item.is_none())
}
/// Puts a carried stack back: into `origin` when it is empty or stacks, else
/// into the first empty storage slot.
pub fn place_carried(snapshot: &Snapshot, origin: Option<u16>) -> Option<(u16, Click)> {
    let carried = snapshot.carried.as_ref()?;
    let fits = |slot: &Slot| {
        slot.item
            .as_ref()
            .is_none_or(|i| i.same_stack_kind(carried) && i.fingerprint.count < i.max_count)
    };
    origin
        .and_then(|o| snapshot.slot(o))
        .filter(|s| fits(s) && !matches!(s.role, SlotRole::Armor(_) | SlotRole::Other))
        .or_else(|| first_empty(snapshot))
        .map(|s| (s.index, Click::Pickup))
}
/// Next click equipping a better armor piece: a shift-click into an empty
/// armor slot, else the three-click pickup swap (piece, armor slot, back).
pub fn equip_next(snapshot: &Snapshot, origin: &mut Option<u16>) -> Option<(u16, Click)> {
    if let Some(carried) = snapshot.carried.as_ref() {
        if let Some(slot) = carried.armor {
            let beats = equipped(snapshot, slot).is_none_or(|c| carried.score > c.score);
            if beats {
                return Some((armor_slot_index(slot), Click::Pickup));
            }
        }
        return place_carried(snapshot, *origin);
    }
    for slot in ARMOR {
        let Some(source) = better_armor(snapshot, slot) else {
            continue;
        };
        *origin = Some(source.index);
        return Some(if equipped(snapshot, slot).is_none() {
            (source.index, Click::QuickMove)
        } else {
            (source.index, Click::Pickup)
        });
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::inventory::{rules::tests::item, types::ItemClass};

    pub(crate) fn armor(id: &str, slot: ArmorSlot, score: f32) -> Item {
        let mut i = item(id, ItemClass::Other, score);
        i.armor = Some(slot);
        i
    }
    #[test]
    fn equips_with_shift_click_or_pickup_swap() {
        let t = std::time::Instant::now();
        let mut s = crate::inventory::session::tests::snapshot(true, t);
        s.slots[20].item = Some(armor("diamond_helmet", ArmorSlot::Head, 3.0));
        s.slots.push(Slot {
            index: 500,
            item: None,
            role: SlotRole::Armor(ArmorSlot::Head),
        });
        let mut origin = None;
        assert_eq!(equip_next(&s, &mut origin), Some((20, Click::QuickMove)));
        let last = s.slots.len() - 1;
        s.slots[last].item = Some(armor("iron_helmet", ArmorSlot::Head, 2.0));
        assert_eq!(equip_next(&s, &mut origin), Some((20, Click::Pickup)));
        s.carried = s.slots[20].item.take();
        assert_eq!(equip_next(&s, &mut origin), Some((500, Click::Pickup)));
        s.carried = s.slots[last].item.replace(s.carried.take().unwrap());
        assert_eq!(equip_next(&s, &mut origin), Some((20, Click::Pickup)));
    }
}
