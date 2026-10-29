// Bounded host-owned inventory snapshots and optimistic container requests.
use std::collections::BTreeMap;

pub const MAX_SLOTS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ArmorSlot {
    Head,
    Chest,
    Legs,
    Feet,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Healing {
    Soup,
    Potion,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ItemClass {
    Sword,
    Bow,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Other,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fingerprint {
    pub id: String,
    pub count: u8,
    pub damage: u16,
    pub tag_hash: u64,
}
#[derive(Clone, Debug)]
pub struct Item {
    pub fingerprint: Fingerprint,
    pub max_count: u8,
    pub max_damage: u16,
    pub armor: Option<ArmorSlot>,
    pub class: ItemClass,
    /// Host-calculated effective strength (base attributes plus applicable enchantments).
    pub score: f32,
    pub healing: Option<Healing>,
    pub harmful_splash: bool,
}
impl Item {
    pub fn durability_used(&self) -> u16 {
        self.fingerprint.damage
    }
    pub fn same_stack_kind(&self, other: &Self) -> bool {
        self.fingerprint.id == other.fingerprint.id
            && self.fingerprint.tag_hash == other.fingerprint.tag_hash
    }
}
#[derive(Clone, Debug)]
pub struct Slot {
    pub index: u16,
    pub item: Option<Item>,
    pub role: SlotRole,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotRole {
    Container,
    Main,
    Hotbar(u8),
    Armor(ArmorSlot),
    Other,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub session: u64,
    pub container_id: u32,
    pub revision: u32,
    pub slots: Vec<Slot>,
    pub carried: Option<Item>,
    pub screen_open: bool,
    pub is_player_inventory: bool,
    pub is_chest: bool,
    pub chest_title_standard: bool,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.slots.len() > MAX_SLOTS {
            return Err("inventory snapshot has too many slots".into());
        }
        let mut seen = BTreeMap::new();
        for slot in &self.slots {
            if seen.insert(slot.index, ()).is_some() {
                return Err("inventory snapshot repeats a slot".into());
            }
            if let Some(item) = &slot.item
                && (item.fingerprint.id.is_empty()
                    || item.fingerprint.id.len() > 256
                    || item.fingerprint.count == 0
                    || item.max_count == 0
                    || item.fingerprint.count > item.max_count
                    || !item.score.is_finite())
            {
                return Err("invalid inventory item metadata".into());
            }
        }
        Ok(())
    }
    pub fn slot(&self, index: u16) -> Option<&Slot> {
        self.slots.iter().find(|s| s.index == index)
    }
    pub fn slots(&self, role: impl Fn(SlotRole) -> bool) -> impl Iterator<Item = &Slot> {
        self.slots.iter().filter(move |s| role(s.role))
    }
    pub fn hotbar(&self, index: u8) -> Option<&Slot> {
        self.slots
            .iter()
            .find(|s| s.role == SlotRole::Hotbar(index))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Click {
    Pickup,
    SwapOffhand,
    QuickMove,
    Throw,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub session: u64,
    pub container_id: u32,
    pub revision: u32,
    pub slot: u16,
    pub click: Click,
    pub expected: Option<Fingerprint>,
    pub expected_carried: Option<Fingerprint>,
}
impl Request {
    pub fn for_slot(snapshot: &Snapshot, slot: &Slot, click: Click) -> Self {
        Self {
            session: snapshot.session,
            container_id: snapshot.container_id,
            revision: snapshot.revision,
            slot: slot.index,
            click,
            expected: slot.item.as_ref().map(|i| i.fingerprint.clone()),
            expected_carried: snapshot.carried.as_ref().map(|i| i.fingerprint.clone()),
        }
    }
}
