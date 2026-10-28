use super::super::super::inventory::{
    planner::Gate,
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
        if !self.gate.acknowledge(snapshot) {
            return Vec::new();
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
        let targets: Vec<&Slot> = (0..9)
            .filter_map(|n| snapshot.hotbar(n))
            .filter(|s| {
                s.item.is_none()
                    || (options.hotbar_clear && junk(s.item.as_ref().unwrap(), options))
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
fn junk(item: &Item, options: &Options) -> bool {
    !heal(item, options.kind) && !options.allowed.contains(&item.fingerprint.id)
}
