use super::super::super::inventory::{
    planner::{Gate, Outcome},
    types::{Click, ItemClass, Request, Slot, SlotRole, Snapshot},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct Options {
    pub check_menu: bool,
    pub best_only: bool,
    pub keep_open: bool,
    pub shuffle: bool,
    pub blacklist: BTreeSet<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            check_menu: true,
            best_only: false,
            keep_open: false,
            shuffle: false,
            blacklist: BTreeSet::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum Effect {
    Request(Request),
    Close,
}
#[derive(Debug, Default)]
pub struct ChestSteal {
    gate: Gate,
    queued: Vec<u16>,
    last: Option<u16>,
    closing: bool,
    seed: u64,
}
impl ChestSteal {
    pub fn cancel(&mut self) {
        self.gate.clear();
        self.queued.clear();
        self.last = None;
        self.closing = false;
    }
    pub fn active(&self) -> bool {
        self.gate.pending().is_some() || !self.queued.is_empty() || self.closing
    }
    pub fn step(&mut self, snapshot: &Snapshot, options: &Options) -> Vec<Effect> {
        if self.gate.interrupted(snapshot)
            || !snapshot.screen_open
            || !snapshot.is_chest
            || options.check_menu && !snapshot.chest_title_standard
        {
            self.cancel();
            return Vec::new();
        }
        if self.closing {
            self.cancel();
            return vec![Effect::Close];
        }
        match self.gate.observe(snapshot) {
            Outcome::Waiting => return Vec::new(),
            Outcome::Invalidated => {
                self.cancel();
                return Vec::new();
            }
            // A quick-move that moved nothing means the player inventory has
            // no room for it; the remaining queue would fail the same way.
            Outcome::Failed(_) => self.queued.clear(),
            Outcome::Idle | Outcome::Applied => {}
        }
        if self.gate.stalled(snapshot) {
            if !options.keep_open {
                self.closing = true;
            }
            return Vec::new();
        }
        if self.queued.is_empty() {
            self.queued = select(snapshot, options);
            if options.shuffle {
                self.shuffle();
            }
        }
        while let Some(slot) = self.queued.first().copied() {
            self.queued.remove(0);
            let Some(source) = snapshot.slot(slot) else {
                continue;
            };
            if source.item.is_none() {
                continue;
            }
            self.last = Some(slot);
            return self
                .gate
                .issue(
                    snapshot,
                    Request::for_slot(snapshot, source, Click::QuickMove),
                )
                .map(Effect::Request)
                .into_iter()
                .collect();
        }
        if !options.keep_open {
            self.closing = true;
        }
        Vec::new()
    }
    fn shuffle(&mut self) {
        for n in (1..self.queued.len()).rev() {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            self.queued.swap(n, self.seed as usize % (n + 1));
        }
        if let Some(last) = self.last {
            self.queued.sort_by_key(|slot| slot.abs_diff(last));
        }
    }
}
fn eligible(slot: &Slot, options: &Options) -> bool {
    slot.item.as_ref().is_some_and(|item| {
        !item.harmful_splash && !options.blacklist.contains(&item.fingerprint.id)
    })
}
fn select(snapshot: &Snapshot, options: &Options) -> Vec<u16> {
    let slots: Vec<&Slot> = snapshot
        .slots(|role| role == SlotRole::Container)
        .filter(|s| eligible(s, options))
        .collect();
    if !options.best_only {
        return slots.into_iter().map(|s| s.index).collect();
    }
    let mut best: BTreeMap<ItemClass, (&Slot, f32)> = BTreeMap::new();
    for slot in slots {
        let item = slot.item.as_ref().unwrap();
        if item.class == ItemClass::Other {
            continue;
        }
        let entry = best.entry(item.class).or_insert((slot, item.score));
        if item.score > entry.1 {
            *entry = (slot, item.score);
        }
    }
    best.into_values().map(|(slot, _)| slot.index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::planner::{DEADLINE, tests::item};

    fn chest() -> Snapshot {
        let slots = (0..3u16)
            .map(|n| Slot {
                index: 300 + n,
                item: Some(item("minecraft:stone", 64)),
                role: SlotRole::Container,
            })
            .collect();
        Snapshot {
            session: 1,
            container_id: 3,
            revision: 5,
            slots,
            carried: None,
            screen_open: true,
            is_player_inventory: false,
            is_chest: true,
            chest_title_standard: true,
            observed: std::time::Instant::now(),
        }
    }

    #[test]
    fn full_inventory_stops_the_pass_instead_of_waiting() {
        let mut steal = ChestSteal::default();
        let options = Options::default();
        let mut s = chest();
        assert!(
            matches!(steal.step(&s, &options).as_slice(), [Effect::Request(r)] if r.slot == 300)
        );
        s.observed += DEADLINE;
        assert!(steal.step(&s, &options).is_empty());
        assert!(matches!(
            steal.step(&s, &options).as_slice(),
            [Effect::Close]
        ));
        assert!(!steal.active());
    }

    #[test]
    fn keep_open_goes_idle_while_nothing_changes() {
        let mut steal = ChestSteal::default();
        let options = Options {
            keep_open: true,
            ..Options::default()
        };
        let mut s = chest();
        assert_eq!(steal.step(&s, &options).len(), 1);
        s.revision += 1;
        assert!(steal.step(&s, &options).is_empty());
        assert!(!steal.active());
        assert!(steal.step(&s, &options).is_empty());
        // Room appears (another slot changed): the pass resumes.
        s.slots[2].item = None;
        assert!(
            matches!(steal.step(&s, &options).as_slice(), [Effect::Request(r)] if r.slot == 300)
        );
    }
}
