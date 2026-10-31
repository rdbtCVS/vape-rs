// InvCleaner: throws out blacklisted items, food other than golden apples,
// negative splash potions and, with Best Items, every sword, pickaxe, bow,
// axe and armor piece that is not the best of its kind. Equipped armor is
// never touched. Each removal is one gated `ThrowStack` click (the
// reference's pickup + click outside the window, as one drop action).
use super::super::super::inventory::{
    arrange::ARMOR,
    rules::armor_slot_index,
    session::{Policy, Session, Step},
    types::{Click, Item, ItemClass, Slot, SlotRole, Snapshot},
};
use std::collections::BTreeSet;

/// Legacy default blacklist ids 280, 287, 318, 345, 288, 374, 116, 54, 145.
pub const DEFAULT_BLACKLIST: [&str; 9] = [
    "minecraft:stick",
    "minecraft:string",
    "minecraft:flint",
    "minecraft:compass",
    "minecraft:feather",
    "minecraft:glass_bottle",
    "minecraft:enchanting_table",
    "minecraft:chest",
    "minecraft:anvil",
];

#[derive(Clone, Debug)]
pub struct Options {
    pub best_items: bool,
    pub remove_negative_potions: bool,
    pub remove_food: bool,
    pub blacklist: BTreeSet<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            best_items: true,
            remove_negative_potions: true,
            remove_food: true,
            blacklist: DEFAULT_BLACKLIST.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
}

#[derive(Debug, Default)]
pub struct InvCleaner {
    session: Session,
}
impl InvCleaner {
    pub fn cancel(&mut self) {
        self.session.cancel();
    }
    pub fn opened(&self) -> bool {
        self.session.opened()
    }
    pub fn active(&self) -> bool {
        self.session.active()
    }
    pub fn step(&mut self, snapshot: &Snapshot, options: &Options, policy: Policy) -> Step {
        let next = next_removal(snapshot, options).map(|slot| (slot, Click::ThrowStack));
        self.session.step(snapshot, policy, next)
    }
}

/// Reference scan order for "best" ties: hotbar, then main rows top-down.
fn best_order(snapshot: &Snapshot) -> impl Iterator<Item = &Slot> {
    snapshot
        .slots(|r| matches!(r, SlotRole::Hotbar(_)))
        .chain(snapshot.slots(|r| r == SlotRole::Main))
}
fn best_of(snapshot: &Snapshot, class: ItemClass) -> Option<u16> {
    let mut best: Option<(&Slot, f32)> = None;
    for slot in best_order(snapshot) {
        let Some(item) = slot
            .item
            .as_ref()
            .filter(|i| i.class == class && i.armor.is_none())
        else {
            continue;
        };
        if best.is_none_or(|(_, score)| item.score > score) {
            best = Some((slot, item.score));
        }
    }
    best.map(|(s, _)| s.index)
}
/// Best piece per armor slot: worn pieces first (the reference walks the
/// container from slot 5), so a stored equal piece is removed.
fn best_armor(snapshot: &Snapshot) -> Vec<u16> {
    ARMOR
        .iter()
        .filter_map(|armor| {
            let worn = snapshot.slot(armor_slot_index(*armor));
            let mut best: Option<(u16, f32)> =
                worn.and_then(|s| Some((s.index, s.item.as_ref()?.score)));
            for slot in snapshot.slots(|r| r == SlotRole::Main || matches!(r, SlotRole::Hotbar(_)))
            {
                let Some(item) = slot.item.as_ref().filter(|i| i.armor == Some(*armor)) else {
                    continue;
                };
                if best.is_none_or(|(_, score)| item.score > score) {
                    best = Some((slot.index, item.score));
                }
            }
            best.map(|(i, _)| i)
        })
        .collect()
}
fn golden_apple(item: &Item) -> bool {
    matches!(
        item.fingerprint.id.as_str(),
        "minecraft:golden_apple" | "minecraft:enchanted_golden_apple"
    )
}
/// The first storage slot (main inventory, then hotbar) to throw out.
pub fn next_removal(snapshot: &Snapshot, options: &Options) -> Option<u16> {
    let armor = best_armor(snapshot);
    let classes = [
        ItemClass::Sword,
        ItemClass::Pickaxe,
        ItemClass::Bow,
        ItemClass::Axe,
    ];
    let best: Vec<(ItemClass, Option<u16>)> = classes
        .iter()
        .map(|c| (*c, best_of(snapshot, *c)))
        .collect();
    snapshot
        .slots(|r| r == SlotRole::Main)
        .chain(snapshot.slots(|r| matches!(r, SlotRole::Hotbar(_))))
        .find(|slot| {
            let Some(item) = slot.item.as_ref() else {
                return false;
            };
            // Every armor piece has a best piece of its type (possibly
            // itself); the reference removes the rest regardless of
            // Best Items.
            if item.armor.is_some() && !armor.contains(&slot.index) {
                return true;
            }
            let not_best = options.best_items
                && item.armor.is_none()
                && best
                    .iter()
                    .any(|(class, index)| item.class == *class && *index != Some(slot.index));
            options.blacklist.contains(&item.fingerprint.id)
                || not_best
                || (options.remove_food && item.meta.food.is_some() && !golden_apple(item))
                || (options.remove_negative_potions && item.harmful_splash)
        })
        .map(|s| s.index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        arrange::tests::armor, rules::tests::item, session::tests::snapshot, types::ArmorSlot,
    };

    #[test]
    fn removes_junk_and_worse_gear_but_keeps_best_and_worn() {
        let mut s = snapshot(true, std::time::Instant::now());
        let o = Options::default();
        // Stick at 20 is blacklisted.
        assert_eq!(next_removal(&s, &o), Some(20));
        s.slots[20].item = None;
        s.slots[0].item = Some(item("diamond_sword", ItemClass::Sword, 7.0));
        s.slots[15].item = Some(item("stone_sword", ItemClass::Sword, 5.0));
        assert_eq!(next_removal(&s, &o), Some(15));
        s.slots[15].item = None;
        s.slots.push(Slot {
            index: 500,
            item: Some(armor("diamond_helmet", ArmorSlot::Head, 3.0)),
            role: SlotRole::Armor(ArmorSlot::Head),
        });
        s.slots[16].item = Some(armor("iron_helmet", ArmorSlot::Head, 2.0));
        assert_eq!(next_removal(&s, &o), Some(16));
        s.slots[16].item = Some(armor("netherite_helmet", ArmorSlot::Head, 3.5));
        assert_eq!(next_removal(&s, &o), None);
        let mut apple = item("golden_apple", ItemClass::Other, 0.0);
        apple.meta.food = Some((4, 1.2));
        s.slots[17].item = Some(apple.clone());
        assert_eq!(next_removal(&s, &o), None);
        apple.fingerprint.id = "minecraft:bread".into();
        s.slots[17].item = Some(apple);
        assert_eq!(next_removal(&s, &o), Some(17));
    }
}
