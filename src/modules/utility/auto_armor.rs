use super::super::super::inventory::{
    planner::Gate,
    types::{ArmorSlot, Click, Request, Slot, SlotRole, Snapshot},
};

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub inventory_only: bool,
    pub open_inventory: bool,
    pub check_durability: bool,
    pub drop_equipped: bool,
    pub combat_check: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            inventory_only: true,
            open_inventory: true,
            check_durability: true,
            drop_equipped: false,
            combat_check: false,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    Request(Request),
    OpenInventory,
    CloseInventory,
}
#[derive(Debug, Default)]
pub struct AutoArmor {
    gate: Gate,
    opened: bool,
    close_after: bool,
}
impl AutoArmor {
    pub fn cancel(&mut self) {
        self.gate.clear();
        self.opened = false;
        self.close_after = false;
    }
    pub fn active(&self) -> bool {
        self.gate.pending().is_some() || self.opened || self.close_after
    }
    pub fn step(
        &mut self,
        snapshot: &Snapshot,
        options: Options,
        combat_blocked: bool,
    ) -> Vec<Effect> {
        if self.gate.interrupted(snapshot) || !snapshot.screen_open && self.opened {
            self.cancel();
        }
        if combat_blocked && options.combat_check {
            self.gate.clear();
            return Vec::new();
        }
        if !snapshot.is_player_inventory {
            if options.inventory_only {
                self.gate.clear();
                return Vec::new();
            }
            if options.open_inventory && !snapshot.screen_open {
                self.opened = true;
                return vec![Effect::OpenInventory];
            }
            return Vec::new();
        }
        if !self.gate.acknowledge(snapshot) {
            return Vec::new();
        }
        let Some((armor, candidate)) = best(snapshot, options.check_durability) else {
            if self.close_after && snapshot.screen_open {
                self.close_after = false;
                self.opened = false;
                return vec![Effect::CloseInventory];
            }
            return Vec::new();
        };
        let equipped = snapshot.slots(|role| role == SlotRole::Armor(armor)).next();
        let mut out = Vec::new();
        if let Some(equipped) = equipped.filter(|s| s.item.is_some()) {
            let click = if options.drop_equipped {
                Click::Throw
            } else {
                Click::QuickMove
            };
            if let Some(request) = self
                .gate
                .issue(snapshot, Request::for_slot(snapshot, equipped, click))
            {
                out.push(Effect::Request(request));
            }
            return out;
        }
        if let Some(request) = self.gate.issue(
            snapshot,
            Request::for_slot(snapshot, candidate, Click::QuickMove),
        ) {
            out.push(Effect::Request(request));
            self.close_after = self.opened;
        }
        out
    }
}
fn best(snapshot: &Snapshot, durability: bool) -> Option<(ArmorSlot, &Slot)> {
    let mut result: Option<(ArmorSlot, &Slot)> = None;
    for candidate in snapshot.slots(|role| matches!(role, SlotRole::Main | SlotRole::Hotbar(_))) {
        let Some(item) = &candidate.item else {
            continue;
        };
        let Some(armor) = item.armor else {
            continue;
        };
        let equipped = snapshot
            .slots(|role| role == SlotRole::Armor(armor))
            .next()
            .and_then(|s| s.item.as_ref());
        let better = equipped.is_none_or(|old| {
            item.score > old.score
                || (durability
                    && item.score == old.score
                    && item.durability_used() < old.durability_used())
        });
        if !better {
            continue;
        }
        if result.as_ref().is_none_or(|(_, current)| {
            item.score > current.item.as_ref().unwrap().score
                || (durability
                    && item.score == current.item.as_ref().unwrap().score
                    && item.durability_used() < current.item.as_ref().unwrap().durability_used())
        }) {
            result = Some((armor, candidate));
        }
    }
    result
}
