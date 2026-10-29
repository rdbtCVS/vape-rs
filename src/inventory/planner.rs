use super::types::{Request, Snapshot};
use std::time::{Duration, Instant};

/// How long a submitted click may stay unresolved. Local and remote clicks are
/// predicted by the host menu engine on the next frame; a click that has shown
/// no effect by then is treated as a no-op rather than awaited indefinitely.
pub const DEADLINE: Duration = Duration::from_millis(1500);

/// Why a pending click was abandoned without a visible effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// The authoritative revision moved but the clicked slot and cursor did not.
    NoEffect,
    /// Nothing observable happened before [`DEADLINE`].
    TimedOut,
}

/// The result of observing a snapshot against the pending click.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// No click is pending; the planner may plan.
    Idle,
    /// The click is still unresolved; the planner must not plan.
    Waiting,
    /// The clicked slot or cursor changed. This is the host's (possibly
    /// predicted) result, not proof of server acceptance; later corrections
    /// arrive as ordinary new snapshots.
    Applied,
    /// The click is abandoned. The gate stays stalled until content changes.
    Failed(Failure),
    /// The session or menu was replaced; the click no longer applies.
    Invalidated,
}

#[derive(Debug)]
struct Pending {
    request: Request,
    deadline: Instant,
}

/// One-at-a-time transaction gate with a bounded wait. Every issued request
/// resolves to an [`Outcome`] within [`DEADLINE`]. After a failed click the
/// gate refuses new requests until the session or inventory content differs
/// from what the failure observed, so deterministic planners cannot repeat
/// the same no-op forever.
#[derive(Debug, Default)]
pub struct Gate {
    pending: Option<Pending>,
    stall: Option<(u64, u64)>,
}
impl Gate {
    pub fn pending(&self) -> Option<&Request> {
        self.pending.as_ref().map(|p| &p.request)
    }
    /// Drops the pending click. Stall memory is kept: it is scoped to the
    /// session and content it observed and expires by itself.
    pub fn clear(&mut self) {
        self.pending = None;
    }
    pub fn stalled(&self, snapshot: &Snapshot) -> bool {
        self.stall == Some((snapshot.session, snapshot.digest()))
    }
    pub fn issue(&mut self, snapshot: &Snapshot, request: Request) -> Option<Request> {
        if self.pending.is_none()
            && !self.stalled(snapshot)
            && request.session == snapshot.session
            && request.container_id == snapshot.container_id
            && request.revision == snapshot.revision
        {
            self.pending = Some(Pending {
                request: request.clone(),
                deadline: snapshot.observed + DEADLINE,
            });
            Some(request)
        } else {
            None
        }
    }
    pub fn observe(&mut self, snapshot: &Snapshot) -> Outcome {
        let Some(pending) = &self.pending else {
            return Outcome::Idle;
        };
        let request = &pending.request;
        if request.session != snapshot.session || request.container_id != snapshot.container_id {
            self.clear();
            return Outcome::Invalidated;
        }
        let slot = snapshot
            .slot(request.slot)
            .and_then(|s| s.item.as_ref())
            .map(|i| &i.fingerprint);
        let carried = snapshot.carried.as_ref().map(|i| &i.fingerprint);
        if slot != request.expected.as_ref() || carried != request.expected_carried.as_ref() {
            self.clear();
            return Outcome::Applied;
        }
        let failure = if snapshot.revision != request.revision {
            Failure::NoEffect
        } else if snapshot.observed >= pending.deadline {
            Failure::TimedOut
        } else {
            return Outcome::Waiting;
        };
        self.clear();
        self.stall = Some((snapshot.session, snapshot.digest()));
        Outcome::Failed(failure)
    }
    pub fn interrupted(&mut self, snapshot: &Snapshot) -> bool {
        self.pending.as_ref().is_some_and(|p| {
            p.request.session != snapshot.session || p.request.container_id != snapshot.container_id
        }) && {
            self.clear();
            true
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::inventory::types::{Click, Fingerprint, Item, ItemClass, Slot, SlotRole};

    pub(crate) fn item(id: &str, count: u8) -> Item {
        Item {
            fingerprint: Fingerprint {
                id: id.into(),
                count,
                damage: 0,
                tag_hash: 0,
            },
            max_count: 64,
            max_damage: 0,
            armor: None,
            class: ItemClass::Other,
            score: 0.0,
            healing: None,
            harmful_splash: false,
        }
    }
    fn snapshot(at: Instant) -> Snapshot {
        Snapshot {
            session: 1,
            container_id: 0,
            revision: 7,
            slots: vec![
                Slot {
                    index: 9,
                    item: Some(item("minecraft:stone", 1)),
                    role: SlotRole::Main,
                },
                Slot {
                    index: 0,
                    item: None,
                    role: SlotRole::Hotbar(0),
                },
            ],
            carried: None,
            screen_open: true,
            is_player_inventory: true,
            is_chest: false,
            chest_title_standard: true,
            observed: at,
        }
    }
    fn click(gate: &mut Gate, s: &Snapshot) -> Request {
        let request = Request::for_slot(s, s.slot(9).unwrap(), Click::QuickMove);
        gate.issue(s, request).unwrap()
    }

    #[test]
    fn visible_slot_change_applies_without_a_revision_change() {
        // Remote servers send nothing when a predicted click matches, so the
        // state id may never move for a successful click.
        let t = Instant::now();
        let mut gate = Gate::default();
        let mut s = snapshot(t);
        click(&mut gate, &s);
        assert_eq!(gate.observe(&s), Outcome::Waiting);
        s.slots[0].item = None;
        s.slots[1].item = Some(item("minecraft:stone", 1));
        assert_eq!(gate.observe(&s), Outcome::Applied);
        assert_eq!(gate.observe(&s), Outcome::Idle);
        assert!(!gate.stalled(&s));
    }

    #[test]
    fn no_op_click_times_out_and_stalls_until_content_changes() {
        let t = Instant::now();
        let mut gate = Gate::default();
        let s = snapshot(t);
        let request = click(&mut gate, &s);
        let mut later = snapshot(t + DEADLINE - Duration::from_millis(1));
        assert_eq!(gate.observe(&later), Outcome::Waiting);
        later.observed = t + DEADLINE;
        assert_eq!(gate.observe(&later), Outcome::Failed(Failure::TimedOut));
        assert!(gate.pending().is_none(), "a no-op must not stay pending");
        assert!(gate.stalled(&later));
        assert_eq!(gate.issue(&later, request.clone()), None);
        // Reopening the same menu with the same content stays stalled, even
        // with a new remote state id.
        let mut reopened = later.clone();
        reopened.revision += 3;
        assert!(gate.stalled(&reopened));
        // Any content change (here: an item picked up) allows a new attempt.
        let mut changed = later.clone();
        changed.slots[1].item = Some(item("minecraft:dirt", 1));
        assert!(!gate.stalled(&changed));
        let mut fresh = Request::for_slot(&changed, changed.slot(9).unwrap(), Click::QuickMove);
        fresh.revision = changed.revision;
        assert!(gate.issue(&changed, fresh).is_some());
    }

    #[test]
    fn revision_without_effect_fails_immediately() {
        let t = Instant::now();
        let mut gate = Gate::default();
        let mut s = snapshot(t);
        click(&mut gate, &s);
        s.revision += 1;
        assert_eq!(gate.observe(&s), Outcome::Failed(Failure::NoEffect));
    }

    #[test]
    fn menu_or_session_replacement_invalidates_without_stalling() {
        let t = Instant::now();
        let mut gate = Gate::default();
        let mut s = snapshot(t);
        click(&mut gate, &s);
        s.container_id = 4;
        assert_eq!(gate.observe(&s), Outcome::Invalidated);
        assert!(!gate.stalled(&s));
        let mut s = snapshot(t);
        click(&mut gate, &s);
        s.session = 2;
        assert!(gate.interrupted(&s));
        assert!(gate.pending().is_none());
    }

    #[test]
    fn stall_is_scoped_to_its_session() {
        let t = Instant::now();
        let mut gate = Gate::default();
        let s = snapshot(t);
        click(&mut gate, &s);
        let late = snapshot(t + DEADLINE);
        assert!(matches!(gate.observe(&late), Outcome::Failed(_)));
        let mut next = late.clone();
        next.session = 2;
        assert!(!gate.stalled(&next));
    }
}
