// AutoHotbar: arranges hotbar slots from per-slot rules (item, any
// material, only best). Each move is one gated number-key swap
// (`SwapHotbar`); topping up a partial stack is a pickup merge.
use super::super::super::inventory::{
    arrange::{place_carried, storage},
    rules::Category,
    session::{Policy, Session, Step},
    types::{Click, Item, ItemClass, SlotRole, Snapshot},
};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub struct SlotRule {
    pub slot: u8,
    /// Namespaced item id.
    pub item: String,
    pub any_material: bool,
    pub only_best: bool,
}
/// Parses the `hotbar` rules setting:
/// `[{"slot":0,"item":"minecraft:diamond_sword","anyMaterial":true,"onlyBest":true}]`.
/// Legacy `item-id`/`meta`/`anyMeta` numeric fields have no modern meaning
/// and are ignored.
pub fn parse_rules(text: &str) -> Result<Vec<SlotRule>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut rules = Vec::new();
    for rule in value
        .as_array()
        .ok_or("hotbar rules must be a JSON array")?
    {
        let slot = rule
            .get("slot")
            .and_then(Value::as_u64)
            .filter(|s| *s < 9)
            .ok_or("hotbar rule needs slot 0..8")? as u8;
        let Some(item) = rule
            .get("item")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let item = if item.contains(':') {
            item.to_owned()
        } else {
            format!("minecraft:{item}")
        };
        if rules.iter().any(|r: &SlotRule| r.slot == slot) {
            continue;
        }
        rules.push(SlotRule {
            slot,
            item,
            any_material: rule
                .get("anyMaterial")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            only_best: rule
                .get("onlyBest")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    rules.sort_by_key(|r| r.slot);
    Ok(rules)
}
/// Item kind standing in for the reference's item class equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Class(ItemClass),
    Armor,
    Food,
    Potion,
    Block,
    Other,
}
fn kind(item: &Item) -> Kind {
    if item.class != ItemClass::Other {
        Kind::Class(item.class)
    } else if item.armor.is_some() {
        Kind::Armor
    } else if item.meta.food.is_some() {
        Kind::Food
    } else if item.fingerprint.id.ends_with("potion") {
        Kind::Potion
    } else if item.meta.block {
        Kind::Block
    } else {
        Kind::Other
    }
}
fn category(kind: Kind) -> Option<Category> {
    Some(match kind {
        Kind::Class(ItemClass::Sword | ItemClass::Axe) => Category::WeaponDamage,
        Kind::Class(ItemClass::Bow) => Category::BowDamage,
        Kind::Class(_) => Category::ToolSpeed,
        Kind::Armor => Category::Armor,
        Kind::Food => Category::Nutrition,
        Kind::Potion => Category::PotionAmplifier,
        Kind::Block | Kind::Other => return None,
    })
}
impl SlotRule {
    fn exemplar_kind(&self, snapshot: &Snapshot) -> Option<Kind> {
        snapshot
            .slots(|_| true)
            .filter_map(|s| s.item.as_ref())
            .find(|i| i.fingerprint.id == self.item)
            .map(kind)
    }
    fn matches(&self, item: &Item, exemplar: Option<Kind>) -> bool {
        item.fingerprint.id == self.item
            || (self.any_material && exemplar.is_some_and(|k| kind(item) == k && k != Kind::Other))
    }
}

#[derive(Debug, Default)]
pub struct AutoHotbar {
    session: Session,
    origin: Option<u16>,
}
impl AutoHotbar {
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
    pub fn step(&mut self, snapshot: &Snapshot, rules: &[SlotRule], policy: Policy) -> Step {
        let next = next_click(snapshot, rules, &mut self.origin);
        let step = self.session.step(snapshot, policy, next);
        if step == Step::Done {
            self.origin = None;
        }
        step
    }
}

pub fn next_click(
    snapshot: &Snapshot,
    rules: &[SlotRule],
    origin: &mut Option<u16>,
) -> Option<(u16, Click)> {
    if let Some(carried) = snapshot.carried.as_ref() {
        // Finishing a top-up: merge into the rule slot holding this kind.
        for rule in rules {
            if let Some(target) = snapshot.hotbar(rule.slot)
                && target.item.as_ref().is_some_and(|i| {
                    i.same_stack_kind(carried) && i.fingerprint.count < i.max_count
                })
                && Some(target.index) != *origin
            {
                return Some((target.index, Click::Pickup));
            }
        }
        return place_carried(snapshot, *origin);
    }
    let mut touched = BTreeSet::new();
    for rule in rules {
        let Some(target) = snapshot.hotbar(rule.slot) else {
            continue;
        };
        let exemplar = rule.exemplar_kind(snapshot);
        let mut candidates: Vec<_> = storage(snapshot)
            .filter(|s| !touched.contains(&s.index))
            .filter(|s| s.item.as_ref().is_some_and(|i| rule.matches(i, exemplar)))
            .collect();
        let current = target.item.as_ref().filter(|i| rule.matches(i, exemplar));
        if rule.only_best
            && let Some(category) = exemplar.and_then(category)
        {
            candidates.sort_by(|a, b| {
                category.compare(a.item.as_ref().unwrap(), b.item.as_ref().unwrap())
            });
            if let (Some(current), Some(best)) = (current, candidates.first())
                && category
                    .compare(current, best.item.as_ref().unwrap())
                    .is_le()
            {
                candidates.retain(|s| s.index == target.index);
            }
        } else if current.is_some() {
            candidates.retain(|s| s.index == target.index);
        }
        let Some(source) = candidates.first() else {
            touched.insert(target.index);
            continue;
        };
        touched.insert(target.index);
        if source.index == target.index {
            // Already placed: top up a partial stack from main storage.
            let item = current.unwrap();
            if item.fingerprint.count < item.max_count
                && let Some(extra) = snapshot.slots(|r| r == SlotRole::Main).find(|s| {
                    !touched.contains(&s.index)
                        && s.item.as_ref().is_some_and(|i| i.same_stack_kind(item))
                })
            {
                *origin = Some(extra.index);
                return Some((extra.index, Click::Pickup));
            }
            continue;
        }
        return Some((source.index, Click::SwapHotbar(rule.slot)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{rules::tests::item, session::tests::snapshot};

    #[test]
    fn swaps_best_matching_item_and_tops_up_stacks() {
        let rules = parse_rules(
            r#"[{"slot":0,"item":"minecraft:iron_sword","anyMaterial":true,"onlyBest":true},
                {"slot":1,"item":"cobblestone"}]"#,
        )
        .unwrap();
        let mut s = snapshot(true, std::time::Instant::now());
        s.slots[10].item = Some(item("iron_sword", ItemClass::Sword, 6.0));
        s.slots[11].item = Some(item("diamond_sword", ItemClass::Sword, 7.0));
        assert_eq!(
            next_click(&s, &rules, &mut None),
            Some((11, Click::SwapHotbar(0)))
        );
        s.slots[0].item = s.slots[11].item.take();
        let mut cobble = item("cobblestone", ItemClass::Other, 0.0);
        cobble.max_count = 64;
        cobble.fingerprint.count = 20;
        s.slots[1].item = Some(cobble.clone());
        s.slots[12].item = Some(cobble);
        let mut origin = None;
        assert_eq!(
            next_click(&s, &rules, &mut origin),
            Some((12, Click::Pickup))
        );
        s.carried = s.slots[12].item.take();
        assert_eq!(
            next_click(&s, &rules, &mut origin),
            Some((1, Click::Pickup))
        );
    }
}
