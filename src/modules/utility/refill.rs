use super::super::super::inventory::{
    planner::{Gate, Outcome},
    types::{Click, Healing, Item, Request, Slot, SlotRole, Snapshot},
};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Both,
    Pots,
    Soup,
}
#[derive(Clone, Debug)]
pub struct Options {
    pub kind: Kind,
    pub vertical: bool,
    pub scatter: bool,
    pub hotbar_clear: bool,
    pub allowed: BTreeSet<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            kind: Kind::Both,
            vertical: false,
            scatter: false,
            hotbar_clear: false,
            allowed: BTreeSet::new(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    OpenInventory,
    CloseInventory,
    Request(Request),
    Disable,
}
#[derive(Debug, Default)]
pub struct Refill {
    gate: Gate,
    opened: bool,
    done: bool,
    seed: u64,
    queued: Vec<(u16, Click)>,
}
impl Refill {
    /// Whether this planner opened the screen that is currently shown.
    pub fn opened(&self) -> bool {
        self.opened
    }
    pub fn active(&self) -> bool {
        self.gate.pending().is_some() || !self.queued.is_empty() || self.opened || self.done
    }
    pub fn cancel(&mut self) {
        self.gate.clear();
        self.opened = false;
        self.done = false;
        self.queued.clear();
    }
    pub fn step(&mut self, snapshot: &Snapshot, options: &Options) -> Vec<Effect> {
        if self.gate.interrupted(snapshot) {
            self.cancel();
            return vec![Effect::Disable];
        }
        if self.done {
            self.cancel();
            return if snapshot.screen_open {
                vec![Effect::CloseInventory, Effect::Disable]
            } else {
                vec![Effect::Disable]
            };
        }
        if !snapshot.is_player_inventory {
            if !snapshot.screen_open {
                self.opened = true;
                return vec![Effect::OpenInventory];
            }
            return Vec::new();
        }
        match self.gate.observe(snapshot) {
            Outcome::Waiting => return Vec::new(),
            Outcome::Invalidated => {
                self.cancel();
                return vec![Effect::Disable];
            }
            // A click in a multi-click swap did nothing: abandon the rest of
            // the sequence and finish (close and disable) on the next step.
            // Closing the inventory returns any cursor stack through the host.
            Outcome::Failed(_) => {
                self.queued.clear();
                self.done = true;
                return Vec::new();
            }
            Outcome::Idle | Outcome::Applied => {}
        }
        if let Some((slot, click)) = self.queued.first().copied() {
            self.queued.remove(0);
            return snapshot
                .slot(slot)
                .and_then(|source| {
                    self.gate
                        .issue(snapshot, Request::for_slot(snapshot, source, click))
                })
                .map(Effect::Request)
                .into_iter()
                .collect();
        }
        // Never start a new transaction with a user-owned cursor stack.
        if snapshot.carried.is_some() {
            return Vec::new();
        }
        let mut sources: Vec<&Slot> = snapshot
            .slots(|r| r == SlotRole::Main)
            .filter(|s| s.item.as_ref().is_some_and(|i| heal(i, options.kind)))
            .collect();
        if options.vertical {
            sources.sort_by_key(|s| (s.index % 9, s.index / 9));
        }
        if options.scatter {
            self.shuffle(&mut sources);
        }
        let mut retained = BTreeSet::new();
        let targets: Vec<&Slot> = (0..9)
            .filter_map(|n| snapshot.hotbar(n))
            .filter(|s| match &s.item {
                None => true,
                Some(item) if options.hotbar_clear => {
                    if heal(item, options.kind) {
                        false
                    } else if options.allowed.contains(&item.fingerprint.id) {
                        !retained.insert(item.fingerprint.id.clone())
                    } else {
                        true
                    }
                }
                Some(_) => false,
            })
            .collect();
        let (Some(source), Some(target)) = (sources.first(), targets.first()) else {
            self.done = true;
            return Vec::new();
        };
        if target.item.is_some() {
            // The reference performs source → hotbar target → source pickup
            // clicks, preserving the displaced stack rather than dropping it.
            self.queued = vec![
                (source.index, Click::Pickup),
                (target.index, Click::Pickup),
                (source.index, Click::Pickup),
            ];
            let (slot, click) = self.queued.remove(0);
            return snapshot
                .slot(slot)
                .and_then(|slot| {
                    self.gate
                        .issue(snapshot, Request::for_slot(snapshot, slot, click))
                })
                .map(Effect::Request)
                .into_iter()
                .collect();
        }
        self.gate
            .issue(
                snapshot,
                Request::for_slot(snapshot, source, Click::QuickMove),
            )
            .map(Effect::Request)
            .into_iter()
            .collect()
    }
    fn shuffle(&mut self, slots: &mut Vec<&Slot>) {
        if self.seed == 0 {
            self.seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos() as u64)
                | 1;
        }
        for n in (1..slots.len()).rev() {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            slots.swap(n, self.seed as usize % (n + 1));
        }
    }
}
fn heal(item: &Item, kind: Kind) -> bool {
    matches!(
        (item.healing, kind),
        (Some(Healing::Potion), Kind::Both | Kind::Pots)
            | (Some(Healing::Soup), Kind::Both | Kind::Soup)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::types::{Fingerprint, ItemClass};

    fn item(id: &str, healing: Option<Healing>) -> Item {
        Item {
            fingerprint: Fingerprint {
                id: id.into(),
                count: 1,
                damage: 0,
                tag_hash: 0,
            },
            max_count: 64,
            max_damage: 0,
            armor: None,
            class: ItemClass::Other,
            score: 0.0,
            healing,
            harmful_splash: false,
            meta: Default::default(),
        }
    }
    fn snapshot() -> Snapshot {
        let mut slots: Vec<_> = (0..9)
            .map(|n| Slot {
                index: n as u16,
                role: SlotRole::Hotbar(n),
                item: Some(item("sword", None)),
            })
            .collect();
        slots.push(Slot {
            index: 9,
            role: SlotRole::Main,
            item: Some(item("pot", Some(Healing::Potion))),
        });
        Snapshot {
            session: 1,
            container_id: 0,
            revision: 1,
            slots,
            carried: None,
            screen_open: true,
            is_player_inventory: true,
            is_chest: false,
            chest_title_standard: false,
            vitals: Default::default(),
            observed: std::time::Instant::now(),
        }
    }
    #[test]
    fn whitelist_retains_one_stack_and_replaces_duplicate() {
        let mut refill = Refill::default();
        let mut options = Options {
            hotbar_clear: true,
            ..Options::default()
        };
        options.allowed.insert("sword".into());
        let mut snap = snapshot();
        assert!(
            matches!(refill.step(&snap, &options).as_slice(), [Effect::Request(r)] if r.slot == 9 && r.click == Click::Pickup)
        );
        snap.revision += 1;
        snap.carried = snap.slot(9).unwrap().item.clone();
        snap.slots.last_mut().unwrap().item = None;
        assert!(
            matches!(refill.step(&snap, &options).as_slice(), [Effect::Request(r)] if r.slot == 1)
        );
    }
    #[test]
    fn mode_and_user_cursor_prevent_new_actions() {
        let mut refill = Refill::default();
        let mut snap = snapshot();
        snap.slots[0].item = None;
        snap.carried = Some(item("held", None));
        assert!(refill.step(&snap, &Options::default()).is_empty());
        snap.carried = None;
        let options = Options {
            kind: Kind::Soup,
            ..Options::default()
        };
        assert!(refill.step(&snap, &options).is_empty());
        assert!(matches!(
            refill.step(&snap, &options).as_slice(),
            [Effect::CloseInventory, Effect::Disable]
        ));
    }
    #[test]
    fn failed_click_abandons_the_swap_and_finishes() {
        use crate::inventory::planner::DEADLINE;
        let mut refill = Refill::default();
        let options = Options {
            hotbar_clear: true,
            ..Options::default()
        };
        let mut snap = snapshot();
        assert!(matches!(
            refill.step(&snap, &options).as_slice(),
            [Effect::Request(_)]
        ));
        // The pickup visibly does nothing until the deadline.
        snap.observed += DEADLINE;
        assert!(refill.step(&snap, &options).is_empty());
        assert!(refill.queued.is_empty(), "the rest of the swap is dropped");
        assert!(matches!(
            refill.step(&snap, &options).as_slice(),
            [Effect::CloseInventory, Effect::Disable]
        ));
        assert!(!refill.active());
    }
    #[test]
    fn scatter_initializes_nonzero_state() {
        let mut refill = Refill::default();
        let snap = snapshot();
        let mut slots: Vec<_> = snap.slots.iter().collect();
        refill.shuffle(&mut slots);
        assert_ne!(refill.seed, 0);
    }
}
