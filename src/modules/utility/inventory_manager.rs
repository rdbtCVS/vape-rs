// InventoryManager: arranges the hotbar from a profile's slot rules, equips
// best armor, and applies inventory item rules (drop / move into inventory /
// condense). One gated click per step; every step re-plans from the snapshot.
use super::super::super::inventory::{
    arrange::{self, place_carried, storage},
    rules::{Action, Category, Filter, Matcher, Profile},
    session::{Policy, Session, Step},
    types::{Click, Item, Slot, SlotRole, Snapshot},
};
use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct InventoryManager {
    session: Session,
    origin: Option<u16>,
}
impl InventoryManager {
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
    pub fn step(&mut self, snapshot: &Snapshot, profile: &Profile, policy: Policy) -> Step {
        // The session resolves a pending click before it reads `next`.
        let next = next_click(snapshot, profile, &mut self.origin);
        let step = self.session.step(snapshot, policy, next);
        if step == Step::Done {
            self.origin = None;
        }
        step
    }
}

fn sorted<'a>(
    snapshot: &'a Snapshot,
    category: Category,
    accept: impl Fn(&Item) -> bool,
    touched: &BTreeSet<u16>,
) -> Vec<&'a Slot> {
    let mut out: Vec<_> = storage(snapshot)
        .filter(|s| !touched.contains(&s.index))
        .filter(|s| s.item.as_ref().is_some_and(&accept))
        .collect();
    out.sort_by(|a, b| category.compare(a.item.as_ref().unwrap(), b.item.as_ref().unwrap()));
    out
}

/// The next click for `profile`, or `None` when the inventory conforms.
pub fn next_click(
    snapshot: &Snapshot,
    profile: &Profile,
    origin: &mut Option<u16>,
) -> Option<(u16, Click)> {
    if snapshot.carried.is_some() {
        if profile.best_armor
            && let Some(click) = arrange::equip_next(snapshot, origin)
        {
            return Some(click);
        }
        // Condense continues: drop the carried part onto a matching stack.
        if let Some(target) =
            condense_target(snapshot, profile, snapshot.carried.as_ref()?, *origin)
        {
            return Some((target, Click::Pickup));
        }
        return place_carried(snapshot, *origin);
    }
    if profile.best_armor
        && let Some(click) = arrange::equip_next(snapshot, origin)
    {
        return Some(click);
    }
    let mut touched = BTreeSet::new();
    // Pass 1: slots whose current item already is the rule's best stay put
    // and reserve the item.
    for rule in &profile.slots {
        let Some(slot) = rule.slot else {
            continue;
        };
        let Some(target) = snapshot.hotbar(slot) else {
            continue;
        };
        let Some(current) = target.item.as_ref().filter(|i| rule.accepts(Some(i))) else {
            continue;
        };
        let category = rule.category();
        let best = sorted(snapshot, category, |i| rule.accepts(Some(i)), &touched);
        let keep = category == Category::AnyType
            || best
                .first()
                .is_none_or(|b| category.compare(current, b.item.as_ref().unwrap()).is_le());
        if keep {
            touched.insert(target.index);
        }
    }
    // Pass 2: fill each remaining slot with its best candidate.
    for rule in &profile.slots {
        let Some(slot) = rule.slot else {
            continue;
        };
        let Some(target) = snapshot.hotbar(slot) else {
            continue;
        };
        if touched.contains(&target.index) {
            continue;
        }
        if rule.filter == Filter::Matcher(Matcher::Hand) {
            // "Hand" keeps the slot empty: move whatever is there out.
            touched.insert(target.index);
            if target.item.is_some() && arrange::first_empty(snapshot).is_some() {
                return Some((target.index, Click::QuickMove));
            }
            continue;
        }
        let candidates = sorted(
            snapshot,
            rule.category(),
            |i| rule.accepts(Some(i)),
            &touched,
        );
        touched.insert(target.index);
        if let Some(best) = candidates.into_iter().find(|s| s.index != target.index) {
            return Some((best.index, Click::SwapHotbar(slot)));
        }
        if let Some(current) = target.item.as_ref()
            && !rule.accepts(Some(current))
            && profile
                .item_rule(current)
                .is_some_and(|r| r.action == Action::Move && r.accepts(Some(current)))
        {
            return Some((target.index, Click::QuickMove));
        }
    }
    // Pass 3: inventory item rules.
    for slot in storage(snapshot) {
        let Some(item) = slot.item.as_ref() else {
            continue;
        };
        if touched.contains(&slot.index) || slot_rule_keeps(snapshot, profile, slot.index, item) {
            continue;
        }
        let Some(rule) = profile.item_rule(item).filter(|r| r.accepts(Some(item))) else {
            continue;
        };
        match rule.action {
            Action::Remove => return Some((slot.index, Click::ThrowStack)),
            Action::Move => {
                if let SlotRole::Hotbar(_) = slot.role
                    && snapshot
                        .slots(|r| r == SlotRole::Main)
                        .any(|s| s.item.is_none())
                {
                    return Some((slot.index, Click::QuickMove));
                }
            }
            Action::Condense => {
                if condense_target(snapshot, profile, item, Some(slot.index)).is_some()
                    && item.fingerprint.count < item.max_count
                {
                    *origin = Some(slot.index);
                    return Some((slot.index, Click::Pickup));
                }
            }
        }
    }
    None
}
fn slot_rule_keeps(snapshot: &Snapshot, profile: &Profile, index: u16, item: &Item) -> bool {
    snapshot.slot(index).is_some_and(|s| match s.role {
        SlotRole::Hotbar(n) => profile.slot_rule(n).is_some_and(|r| r.accepts(Some(item))),
        _ => false,
    })
}
/// The fullest non-full stack of the same kind (other than `origin`) that a
/// condense rule covers.
fn condense_target(
    snapshot: &Snapshot,
    profile: &Profile,
    item: &Item,
    origin: Option<u16>,
) -> Option<u16> {
    let rule = profile.item_rule(item)?;
    if rule.action != Action::Condense {
        return None;
    }
    storage(snapshot)
        .filter(|s| Some(s.index) != origin)
        .filter_map(|s| Some((s.index, s.item.as_ref()?)))
        .filter(|(_, i)| i.same_stack_kind(item) && i.fingerprint.count < i.max_count)
        .max_by_key(|(index, i)| (i.fingerprint.count, std::cmp::Reverse(*index)))
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        rules::{parse_profiles, tests::item},
        session::tests::snapshot,
        types::ItemClass,
    };

    fn profile(text: &str) -> Profile {
        parse_profiles(text, &[]).unwrap().remove(0)
    }
    #[test]
    fn moves_best_sword_into_rule_slot() {
        let p = profile(r#"{"name":"a","slots":[{"slot":0,"itemFilter":"swords"}]}"#);
        let mut s = snapshot(true, std::time::Instant::now());
        s.slots[20].item = Some(item("stone_sword", ItemClass::Sword, 5.0));
        s.slots[21].item = Some(item("diamond_sword", ItemClass::Sword, 7.0));
        assert_eq!(
            next_click(&s, &p, &mut None),
            Some((21, Click::SwapHotbar(0)))
        );
        s.slots[0].item = s.slots[21].item.take();
        assert_eq!(next_click(&s, &p, &mut None), None);
    }
    #[test]
    fn drops_and_condenses_by_item_rules() {
        let p = profile(
            r#"{"name":"a","inventoryFilters":[
                {"itemFilter":"minecraft:stick","filterAction":"Drop item"},
                {"itemFilter":"minecraft:cobblestone","filterAction":"Condense stacks"}]}"#,
        );
        let mut s = snapshot(true, std::time::Instant::now());
        assert_eq!(next_click(&s, &p, &mut None), Some((20, Click::ThrowStack)));
        s.slots[20].item = None;
        let mut cobble = item("cobblestone", ItemClass::Other, 0.0);
        cobble.max_count = 64;
        cobble.fingerprint.count = 10;
        s.slots[12].item = Some(cobble.clone());
        cobble.fingerprint.count = 30;
        s.slots[13].item = Some(cobble);
        let mut origin = None;
        assert_eq!(next_click(&s, &p, &mut origin), Some((12, Click::Pickup)));
        s.carried = s.slots[12].item.take();
        assert_eq!(next_click(&s, &p, &mut origin), Some((13, Click::Pickup)));
    }
}
