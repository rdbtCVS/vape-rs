use super::super::super::inventory::{
    planner::{Gate, Outcome},
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
    /// Whether this planner opened the screen that is currently shown.
    pub fn opened(&self) -> bool {
        self.opened
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
        let outcome = self.gate.observe(snapshot);
        if outcome == Outcome::Waiting {
            return Vec::new();
        }
        if outcome == Outcome::Invalidated {
            self.cancel();
            return Vec::new();
        }
        // The next click this planner would make. A stalled gate means the
        // previous click changed nothing, so it would only repeat; treat that
        // like having nothing to do instead of holding the inventory open.
        let next = (!self.gate.stalled(snapshot))
            .then(|| next_request(snapshot, options))
            .flatten();
        let Some(request) = next else {
            if (self.close_after || self.opened) && snapshot.screen_open {
                self.close_after = false;
                self.opened = false;
                return vec![Effect::CloseInventory];
            }
            self.close_after = false;
            self.opened = false;
            return Vec::new();
        };
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
        let equipping = request.click == Click::QuickMove
            && snapshot
                .slot(request.slot)
                .is_some_and(|slot| !matches!(slot.role, SlotRole::Armor(_)));
        match self.gate.issue(snapshot, request) {
            Some(request) => {
                if equipping {
                    self.close_after = self.opened;
                }
                vec![Effect::Request(request)]
            }
            None => Vec::new(),
        }
    }
}
/// Removes a worse equipped piece first, then equips the best candidate.
fn next_request(snapshot: &Snapshot, options: Options) -> Option<Request> {
    let (armor, candidate) = best(snapshot, options.check_durability)?;
    let equipped = snapshot
        .slots(|role| role == SlotRole::Armor(armor))
        .next()
        .filter(|s| s.item.is_some());
    Some(match equipped {
        Some(equipped) => {
            let click = if options.drop_equipped {
                Click::Throw
            } else {
                Click::QuickMove
            };
            Request::for_slot(snapshot, equipped, click)
        }
        None => Request::for_slot(snapshot, candidate, Click::QuickMove),
    })
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
