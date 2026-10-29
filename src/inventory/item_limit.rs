// Reference `ItemLimitData.matches` and `ItemHelper` groups for item lists
// such as AimAssist's "Allowed Items". Entries are free text: a group name
// ("swords", "pickaxes", "blocks", "fists"...), `slotN` for a hotbar slot,
// an item id ("diamond_sword") or a display name ("Diamond Sword").
// A leading `!` disables an entry without removing it.

/// What the host knows about one held stack.
#[derive(Clone, Debug, Default)]
pub struct HeldItem {
    /// Item id without the namespace; None for an empty hand.
    pub id: Option<String>,
    pub display_name: String,
    pub block: bool,
    pub food: bool,
    pub potion: bool,
    /// Selected hotbar slot, 0-based.
    pub slot: usize,
}

fn group(name: &str, held: &HeldItem) -> Option<bool> {
    let id = held.id.as_deref().unwrap_or("");
    let ends = |suffix: &str| held.id.is_some() && id.ends_with(suffix);
    Some(match name {
        "sword" | "swords" => ends("_sword"),
        "shovel" | "shovels" | "spade" | "spades" => ends("_shovel"),
        "axe" | "axes" => ends("_axe") && !ends("_pickaxe"),
        "pickaxe" | "pickaxes" => ends("_pickaxe"),
        "block" | "blocks" => held.id.is_some() && held.block,
        "fists" | "none" | "fist" | "hand" => held.id.is_none(),
        "food" | "foods" => held.id.is_some() && held.food,
        "potion" | "potions" => held.id.is_some() && held.potion,
        "bed" | "beds" => ends("_bed"),
        _ => return None,
    })
}

/// One entry against the held stack.
pub fn entry_matches(entry: &str, held: &HeldItem) -> bool {
    let entry = entry.trim();
    if entry.is_empty() || entry.starts_with('!') {
        return false;
    }
    let name = entry.to_lowercase();
    if let Some(slot) = name.strip_prefix("slot")
        && let Ok(slot) = slot.parse::<usize>()
        && slot >= 1
        && held.slot + 1 == slot
    {
        return true;
    }
    if let Some(found) = group(&name, held)
        && found
    {
        return true;
    }
    let Some(id) = held.id.as_deref() else {
        return false;
    };
    let id = id.trim_start_matches("minecraft:");
    name.trim_start_matches("minecraft:") == id || name == held.display_name.to_lowercase()
}

/// `LimitValue.isValid`: any enabled entry matches.
pub fn matches(entries: &[String], held: &HeldItem) -> bool {
    entries.iter().any(|e| entry_matches(e, held))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn held(id: Option<&str>) -> HeldItem {
        HeldItem {
            id: id.map(Into::into),
            display_name: "Diamond Sword".into(),
            slot: 2,
            ..HeldItem::default()
        }
    }
    #[test]
    fn groups_ids_names_and_slots() {
        let sword = held(Some("diamond_sword"));
        assert!(entry_matches("swords", &sword));
        assert!(entry_matches("Diamond Sword", &sword));
        assert!(entry_matches("minecraft:diamond_sword", &sword));
        assert!(entry_matches("slot3", &sword));
        assert!(!entry_matches("slot1", &sword));
        assert!(!entry_matches("!swords", &sword));
        assert!(!entry_matches("pickaxes", &sword));
        assert!(entry_matches("axes", &held(Some("iron_axe"))));
        assert!(!entry_matches("axes", &held(Some("iron_pickaxe"))));
        assert!(entry_matches("fists", &held(None)));
        assert!(!entry_matches("swords", &held(None)));
        assert!(matches(&["pickaxes".into(), "swords".into()], &sword));
    }
}
