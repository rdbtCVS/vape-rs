// Shared click session for planners that rearrange the player inventory one
// gated click at a time: opening the inventory screen when allowed, the open
// wait before the first click, and the close timer after the last one.
use super::{
    planner::{Gate, Outcome},
    types::{Click, Request, Snapshot},
};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    Request(Request),
    OpenInventory,
    CloseInventory,
}
/// Session result for one step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step {
    /// Hand this effect to the host.
    Effect(Effect),
    /// Work remains; call again later.
    Wait,
    /// Work exists but cannot run now (screen closed and not allowed to
    /// open, a foreign container is open).
    Blocked,
    /// No work remains; any screen this session opened is closed.
    Done,
}
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    /// Open the player inventory when work exists and no screen is shown.
    pub open: bool,
    /// Close a screen this session opened once work is finished.
    pub close: bool,
    /// Wait after opening before the first click (the reference's open wait).
    pub open_wait: Duration,
    /// Wait after the last click before closing (the reference's close timer).
    pub close_wait: Duration,
    /// Click the player's menu (window 0) while no screen is shown, as the
    /// reference's direct window clicks do; the host sends them with no
    /// screen.
    pub screenless: bool,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            open: true,
            close: true,
            open_wait: Duration::ZERO,
            close_wait: Duration::ZERO,
            screenless: false,
        }
    }
}
#[derive(Debug, Default)]
pub struct Session {
    gate: Gate,
    opened: Option<Instant>,
    last_click: Option<Instant>,
    /// The planned clicks reported `NoEffect`/timeouts; finish instead of
    /// repeating them.
    failed: bool,
}
impl Session {
    pub fn cancel(&mut self) {
        self.gate.clear();
        self.opened = None;
        self.last_click = None;
        self.failed = false;
    }
    pub fn opened(&self) -> bool {
        self.opened.is_some()
    }
    pub fn active(&self) -> bool {
        self.opened.is_some() || self.gate.pending().is_some()
    }
    pub fn pending(&self) -> bool {
        self.gate.pending().is_some()
    }
    /// Advances the session. `next` is the planner's next `(slot, click)`
    /// for this snapshot, or `None` when the inventory is arranged. Planners
    /// read only player slots, so they can plan before the screen opens.
    pub fn step(
        &mut self,
        snapshot: &Snapshot,
        policy: Policy,
        next: Option<(u16, Click)>,
    ) -> Step {
        let now = snapshot.observed;
        if self.gate.interrupted(snapshot) {
            self.cancel();
            return Step::Done;
        }
        if self.opened.is_some() && !snapshot.screen_open {
            // The user (or the server) closed the screen this session opened.
            self.cancel();
            return Step::Done;
        }
        match self.gate.observe(snapshot) {
            Outcome::Waiting => return Step::Wait,
            Outcome::Invalidated => {
                self.cancel();
                return Step::Done;
            }
            Outcome::Failed(_) => self.failed = true,
            Outcome::Idle | Outcome::Applied => {}
        }
        if self.failed || self.gate.stalled(snapshot) || next.is_none() {
            return self.finish(snapshot, policy, now);
        }
        let screenless = policy.screenless && !snapshot.screen_open && snapshot.container_id == 0;
        if !snapshot.is_player_inventory && !screenless {
            if policy.open && !snapshot.screen_open {
                self.opened = Some(now);
                return Step::Effect(Effect::OpenInventory);
            }
            return Step::Blocked;
        }
        if self
            .opened
            .is_some_and(|at| now.saturating_duration_since(at) < policy.open_wait)
        {
            return Step::Wait;
        }
        if snapshot.carried.is_some() && next.is_some_and(|(_, c)| c != Click::Pickup) {
            return Step::Wait;
        }
        let Some((index, click)) = next else {
            return self.finish(snapshot, policy, now);
        };
        let Some(slot) = snapshot.slot(index) else {
            return self.finish(snapshot, policy, now);
        };
        match self
            .gate
            .issue(snapshot, Request::for_slot(snapshot, slot, click))
        {
            Some(request) => {
                self.last_click = Some(now);
                Step::Effect(Effect::Request(request))
            }
            None => self.finish(snapshot, policy, now),
        }
    }
    fn finish(&mut self, snapshot: &Snapshot, policy: Policy, now: Instant) -> Step {
        if self.opened.is_some() && snapshot.is_player_inventory && policy.close {
            if self
                .last_click
                .is_some_and(|at| now.saturating_duration_since(at) < policy.close_wait)
            {
                return Step::Wait;
            }
            self.cancel();
            return Step::Effect(Effect::CloseInventory);
        }
        self.cancel();
        Step::Done
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::inventory::planner::tests::item;
    use crate::inventory::types::{Slot, SlotRole, Vitals};

    pub(crate) fn snapshot(open: bool, at: Instant) -> Snapshot {
        let mut slots: Vec<Slot> = (0..36)
            .map(|i| Slot {
                index: i,
                item: None,
                role: if i < 9 {
                    SlotRole::Hotbar(i as u8)
                } else {
                    SlotRole::Main
                },
            })
            .collect();
        slots[20].item = Some(item("minecraft:stick", 1));
        Snapshot {
            session: 1,
            container_id: 0,
            revision: 1,
            slots,
            carried: None,
            screen_open: open,
            is_player_inventory: open,
            is_chest: false,
            chest_title_standard: true,
            vitals: Vitals::default(),
            observed: at,
        }
    }
    #[test]
    fn opens_waits_clicks_and_closes() {
        let t = Instant::now();
        let policy = Policy {
            open_wait: Duration::from_millis(200),
            close_wait: Duration::from_millis(200),
            ..Policy::default()
        };
        let mut s = Session::default();
        let plan = Some((20, Click::ThrowStack));
        assert_eq!(
            s.step(&snapshot(false, t), policy, plan),
            Step::Effect(Effect::OpenInventory)
        );
        assert_eq!(s.step(&snapshot(true, t), policy, plan), Step::Wait);
        let later = t + Duration::from_millis(250);
        assert!(matches!(
            s.step(&snapshot(true, later), policy, plan),
            Step::Effect(Effect::Request(_))
        ));
        let mut done = snapshot(true, later + Duration::from_millis(10));
        done.slots[20].item = None;
        done.revision = 2;
        assert_eq!(s.step(&done, policy, None), Step::Wait);
        done.observed = later + Duration::from_millis(300);
        assert_eq!(
            s.step(&done, policy, None),
            Step::Effect(Effect::CloseInventory)
        );
        assert!(!s.active());
    }
    #[test]
    fn closed_inventory_without_open_permission_is_blocked() {
        let mut s = Session::default();
        let policy = Policy {
            open: false,
            ..Policy::default()
        };
        assert_eq!(
            s.step(
                &snapshot(false, Instant::now()),
                policy,
                Some((20, Click::ThrowStack))
            ),
            Step::Blocked
        );
    }
    #[test]
    fn screenless_clicks_the_player_menu_and_never_opens() {
        let t = Instant::now();
        let mut s = Session::default();
        let policy = Policy {
            open: false,
            screenless: true,
            ..Policy::default()
        };
        let plan = Some((20, Click::ThrowStack));
        let Step::Effect(Effect::Request(request)) = s.step(&snapshot(false, t), policy, plan)
        else {
            panic!("a screenless click");
        };
        assert_eq!((request.slot, request.click), (20, Click::ThrowStack));
        assert!(!s.opened());
        let mut done = snapshot(false, t + Duration::from_millis(10));
        done.slots[20].item = None;
        done.revision = 2;
        assert_eq!(s.step(&done, policy, None), Step::Done);
        // A foreign container is still not the player's menu.
        let mut chest = snapshot(false, t);
        chest.container_id = 3;
        assert_eq!(s.step(&chest, policy, plan), Step::Blocked);
    }
}
