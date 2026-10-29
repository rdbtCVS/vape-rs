use super::super::super::inventory::{
    planner::Gate,
    types::{Click, Request, SlotRole, Snapshot},
};

#[derive(Debug, Default)]
pub struct AutoTotem {
    gate: Gate,
    opened: bool,
}

#[derive(Debug)]
pub enum Effect {
    Request(Request),
    OpenInventory,
    CloseInventory,
}

impl AutoTotem {
    pub fn cancel(&mut self) {
        self.gate.clear();
        self.opened = false;
    }
    pub fn active(&self) -> bool {
        self.opened || self.gate.pending().is_some()
    }
    pub fn step(
        &mut self,
        snapshot: &Snapshot,
        inventory_only: bool,
        open: bool,
        close: bool,
    ) -> Option<Effect> {
        if self.gate.interrupted(snapshot)
            || (self.opened && !snapshot.screen_open && self.gate.pending().is_some())
        {
            self.cancel();
            return None;
        }
        if !self.gate.acknowledge(snapshot) {
            return None;
        }
        let equipped = snapshot
            .slot(504)?
            .item
            .as_ref()
            .is_some_and(|i| i.fingerprint.id == "minecraft:totem_of_undying");
        if equipped {
            if self.opened && close && snapshot.is_player_inventory {
                self.opened = false;
                return Some(Effect::CloseInventory);
            }
            self.opened = false;
            return None;
        }
        let source = snapshot
            .slots(|role| matches!(role, SlotRole::Main | SlotRole::Hotbar(_)))
            .find(|slot| {
                slot.item
                    .as_ref()
                    .is_some_and(|i| i.fingerprint.id == "minecraft:totem_of_undying")
            })?;
        if !snapshot.is_player_inventory {
            if !inventory_only && open && !snapshot.screen_open {
                self.opened = true;
                return Some(Effect::OpenInventory);
            }
            return None;
        }
        if snapshot.carried.is_some() {
            return None;
        }
        self.gate
            .issue(
                snapshot,
                Request::for_slot(snapshot, source, Click::SwapOffhand),
            )
            .map(Effect::Request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::types::{Fingerprint, Item, ItemClass, Slot};

    fn item(id: &str) -> Item {
        Item {
            fingerprint: Fingerprint {
                id: id.into(),
                count: 1,
                damage: 0,
                tag_hash: 0,
            },
            max_count: 1,
            max_damage: 0,
            armor: None,
            class: ItemClass::Other,
            score: 0.0,
            healing: None,
            harmful_splash: false,
        }
    }
    fn snapshot() -> Snapshot {
        Snapshot {
            session: 1,
            container_id: 0,
            revision: 1,
            slots: vec![
                Slot {
                    index: 9,
                    role: SlotRole::Main,
                    item: Some(item("minecraft:totem_of_undying")),
                },
                Slot {
                    index: 504,
                    role: SlotRole::Other,
                    item: Some(item("minecraft:shield")),
                },
            ],
            carried: None,
            screen_open: true,
            is_player_inventory: true,
            is_chest: false,
            chest_title_standard: true,
        }
    }
    #[test]
    fn swaps_once_and_waits_for_revision() {
        let mut module = AutoTotem::default();
        let mut s = snapshot();
        assert!(matches!(
            module.step(&s, false, true, true),
            Some(Effect::Request(Request {
                slot: 9,
                click: Click::SwapOffhand,
                ..
            }))
        ));
        assert!(module.step(&s, false, true, true).is_none());
        s.revision += 1;
        s.slots[1].item = Some(item("minecraft:totem_of_undying"));
        assert!(module.step(&s, false, true, true).is_none());
        assert!(!module.active());
    }
    #[test]
    fn respects_cursor_and_other_container() {
        let mut module = AutoTotem::default();
        let mut s = snapshot();
        s.carried = Some(item("minecraft:shield"));
        assert!(module.step(&s, false, true, true).is_none());
        s.carried = None;
        s.is_player_inventory = false;
        assert!(module.step(&s, false, true, true).is_none());
    }
}
