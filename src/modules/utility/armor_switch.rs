// ArmorSwitch: swaps the worn armor set between two materials. The worn
// material is read from the first worn piece; the other configured set must
// be complete in storage. Each piece is a three-click pickup swap (stored
// piece, armor slot, back to the stored slot), one gated click at a time.
use super::super::super::inventory::{
    arrange::{ARMOR, equipped, storage},
    rules::armor_slot_index,
    session::{Policy, Session, Step},
    types::{ArmorSlot, Click, Item, Snapshot},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Material {
    Diamond,
    Gold,
    Iron,
    Leather,
    Chain,
}
impl Material {
    pub const ALL: [Self; 5] = [
        Self::Diamond,
        Self::Gold,
        Self::Iron,
        Self::Leather,
        Self::Chain,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Diamond => "Diamond",
            Self::Gold => "Gold",
            Self::Iron => "Iron",
            Self::Leather => "Leather",
            Self::Chain => "Chain",
        }
    }
    pub fn find(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }
    fn prefix(self) -> &'static str {
        match self {
            Self::Diamond => "minecraft:diamond_",
            Self::Gold => "minecraft:golden_",
            Self::Iron => "minecraft:iron_",
            Self::Leather => "minecraft:leather_",
            Self::Chain => "minecraft:chainmail_",
        }
    }
    fn of(item: &Item) -> Option<Self> {
        item.armor?;
        Self::ALL
            .into_iter()
            .find(|m| item.fingerprint.id.starts_with(m.prefix()))
    }
}

#[derive(Debug, Default)]
pub struct ArmorSwitch {
    session: Session,
    target: Option<Material>,
    /// Stored slot of the piece being swapped (where the old piece goes).
    origin: Option<u16>,
    started: bool,
}
impl ArmorSwitch {
    pub fn cancel(&mut self) {
        self.session.cancel();
        self.target = None;
        self.origin = None;
        self.started = false;
    }
    pub fn opened(&self) -> bool {
        self.session.opened()
    }
    pub fn active(&self) -> bool {
        self.session.active()
    }
    /// `Done` means the module disables itself (switched, or no switch is
    /// possible: no worn armor or an incomplete other set).
    pub fn step(
        &mut self,
        snapshot: &Snapshot,
        sets: (Material, Material),
        policy: Policy,
    ) -> Step {
        if !self.started {
            self.started = true;
            self.target = target(snapshot, sets);
        }
        let next = self
            .target
            .and_then(|m| next_click(snapshot, m, &mut self.origin));
        self.session.step(snapshot, policy, next)
    }
}
fn piece(snapshot: &Snapshot, material: Material, slot: ArmorSlot) -> Option<u16> {
    storage(snapshot)
        .filter(|s| {
            s.item
                .as_ref()
                .is_some_and(|i| i.armor == Some(slot) && Material::of(i) == Some(material))
        })
        .last()
        .map(|s| s.index)
}
/// The set to switch to, when one worn piece exists and the other set is
/// complete in storage.
pub fn target(snapshot: &Snapshot, sets: (Material, Material)) -> Option<Material> {
    let worn = ARMOR
        .iter()
        .find_map(|slot| equipped(snapshot, *slot))
        .and_then(Material::of)?;
    let target = if worn == sets.0 { sets.1 } else { sets.0 };
    ARMOR
        .iter()
        .all(|slot| piece(snapshot, target, *slot).is_some())
        .then_some(target)
}
pub fn next_click(
    snapshot: &Snapshot,
    material: Material,
    origin: &mut Option<u16>,
) -> Option<(u16, Click)> {
    if let Some(carried) = snapshot.carried.as_ref() {
        if Material::of(carried) == Some(material)
            && let Some(slot) = carried.armor
        {
            return Some((armor_slot_index(slot), Click::Pickup));
        }
        // The old piece goes back where the new one came from.
        let back = origin
            .filter(|o| snapshot.slot(*o).is_some_and(|s| s.item.is_none()))
            .or_else(|| {
                storage(snapshot)
                    .find(|s| s.item.is_none())
                    .map(|s| s.index)
            })?;
        return Some((back, Click::Pickup));
    }
    for slot in ARMOR {
        if equipped(snapshot, slot).and_then(Material::of) == Some(material) {
            continue;
        }
        let source = piece(snapshot, material, slot)?;
        *origin = Some(source);
        return Some((source, Click::Pickup));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        arrange::tests::armor,
        session::tests::snapshot,
        types::{Slot, SlotRole},
    };

    #[test]
    fn swaps_each_piece_with_three_pickups() {
        let mut s = snapshot(true, std::time::Instant::now());
        for (n, slot) in ARMOR.iter().enumerate() {
            let name = ["helmet", "chestplate", "leggings", "boots"][n];
            s.slots.push(Slot {
                index: armor_slot_index(*slot),
                item: Some(armor(&format!("diamond_{name}"), *slot, 3.0)),
                role: SlotRole::Armor(*slot),
            });
            s.slots[10 + n].item = Some(armor(&format!("golden_{name}"), *slot, 2.0));
        }
        let sets = (Material::Diamond, Material::Gold);
        assert_eq!(target(&s, sets), Some(Material::Gold));
        let mut origin = None;
        assert_eq!(
            next_click(&s, Material::Gold, &mut origin),
            Some((10, Click::Pickup))
        );
        s.carried = s.slots[10].item.take();
        assert_eq!(
            next_click(&s, Material::Gold, &mut origin),
            Some((500, Click::Pickup))
        );
        let head = s.slots.iter().position(|x| x.index == 500).unwrap();
        s.carried = s.slots[head].item.replace(s.carried.take().unwrap());
        assert_eq!(
            next_click(&s, Material::Gold, &mut origin),
            Some((10, Click::Pickup))
        );
        s.slots[10].item = s.carried.take();
        assert_eq!(
            next_click(&s, Material::Gold, &mut origin),
            Some((11, Click::Pickup))
        );
        s.slots[13].item = None;
        assert_eq!(target(&s, sets), None);
    }
}
