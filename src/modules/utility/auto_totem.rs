use super::super::super::inventory::{
    planner::{Gate, Outcome},
    types::{Click, Request, SlotRole, Snapshot},
};
use std::time::Instant;

#[derive(Debug, Default)]
pub struct AutoTotem {
    gate: Gate,
    opened: bool,
    /// "Random slot": take a random totem instead of the first one.
    pub random_slot: bool,
    seed: u64,
    /// A "Silent open" session (`inventoryOpen && silentActive`): the
    /// player menu is clicked without a screen.
    silent: bool,
    /// `suppressInput`: movement keys and clicks are held.
    suppress: bool,
    /// `actionTimer`: the last silent open, click or session tick.
    action_at: Option<Instant>,
    /// A silent session ended by [`AutoTotem::cancel`]; the host still owes
    /// the player menu its close.
    close_owed: bool,
}

/// The settings one step reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub inventory_only: bool,
    pub open: bool,
    pub close: bool,
    /// "Silent open".
    pub silent: bool,
}

pub const TOTEM: &str = "minecraft:totem_of_undying";

/// Totems in the main inventory and hotbar ("Show totem count" counts
/// container slots 9..=45, which includes the offhand).
pub fn totem_count(snapshot: &Snapshot) -> u32 {
    snapshot
        .slots(|role| matches!(role, SlotRole::Main | SlotRole::Hotbar(_)))
        .chain(snapshot.slot(504))
        .filter_map(|slot| slot.item.as_ref())
        .filter(|i| i.fingerprint.id == TOTEM)
        .map(|i| u32::from(i.fingerprint.count))
        .sum()
}
/// The reference's count colour: red under 2, yellow under 4, else green.
pub fn count_color(count: u32) -> [u8; 3] {
    match count {
        0..=1 => [255, 20, 20],
        2..=3 => [255, 249, 18],
        _ => [2, 190, 58],
    }
}

#[derive(Debug)]
pub enum Effect {
    Request(Request),
    OpenInventory,
    CloseInventory,
    /// A silent session began: no screen, the sent rotation is held.
    SilentOpen,
    /// A click on the player menu of a silent session (shown or not).
    SilentRequest(Request),
    /// The silent session ended: close the player menu, return the rotation.
    SilentClose,
}

impl AutoTotem {
    fn next(&mut self) -> u64 {
        if self.seed == 0 {
            self.seed = 0x70e7_d1ce;
        }
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// The next click delay in ms. Without "Extra randomization" uniform in
    /// the range; with it a normal around the centre (sd range/4) clamped to
    /// the range, plus an 18 % chance of a long extra pause, as in the
    /// reference's `computeDelay`.
    pub fn delay(&mut self, (min, max): (u64, u64), extra: bool) -> u64 {
        let (min, max) = (min.min(max) as f64, min.max(max) as f64);
        if !extra {
            return (min + (max - min) * self.unit()).round() as u64;
        }
        let range = (max - min).max(1.0);
        let center = min + range * 0.5;
        let sd = (range / 4.0).max(1.0);
        // Box-Muller.
        let (u1, u2) = (self.unit().max(f64::MIN_POSITIVE), self.unit());
        let gaussian = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
        let mut result = (center + gaussian * sd).clamp(min, max);
        if self.unit() < 0.18 {
            let extra_min = (range * 1.25).max(120.0);
            let extra_max = (range * 3.25).max(350.0);
            result += extra_min + (extra_max - extra_min) * self.unit();
        }
        result.max(1.0).round() as u64
    }
    pub fn cancel(&mut self) {
        self.gate.clear();
        self.opened = false;
        if std::mem::take(&mut self.silent) {
            self.close_owed = true;
        }
    }
    /// Whether this planner opened the screen that is currently shown.
    pub fn opened(&self) -> bool {
        self.opened
    }
    pub fn active(&self) -> bool {
        self.opened || self.silent || self.gate.pending().is_some()
    }
    /// A silent session is open.
    pub fn silent(&self) -> bool {
        self.silent
    }
    /// Takes the close a cancelled silent session still owes.
    pub fn take_close_owed(&mut self) -> bool {
        std::mem::take(&mut self.close_owed)
    }
    /// One client tick of `suppressInput` (the reference's `onTick` head):
    /// the hold ends once a fresh "Silent move delay" sample has passed
    /// since the last silent action; an open session renews that time every
    /// tick, so the hold lasts the whole session plus one delay. Returns
    /// whether movement keys and clicks are held this tick.
    pub fn suppress_tick(&mut self, now: Instant, (min, max): (u64, u64)) -> bool {
        if self.suppress {
            let (low, high) = (min.min(max) as f64, min.max(max) as f64);
            let sample = (low + (high - low) * self.unit()) as u64;
            let elapsed = self.action_at.map_or(u64::MAX, |at| {
                now.saturating_duration_since(at).as_millis() as u64
            });
            if elapsed >= sample {
                self.suppress = false;
            }
        }
        if self.silent {
            self.action_at = Some(now);
        }
        self.suppress
    }
    /// Whether movement keys and clicks are held (`suppressInput`).
    pub fn suppressing(&self) -> bool {
        self.suppress
    }
    /// A disabled module holds no input.
    pub fn release_input(&mut self) {
        self.suppress = false;
    }
    pub fn step(
        &mut self,
        snapshot: &Snapshot,
        options: Options,
        blocked: impl FnOnce() -> bool,
    ) -> Option<Effect> {
        let now = snapshot.observed;
        // The reference returns while another window is open. A silent
        // session ends there without a close of its own: the container's
        // close puts the server back on the player menu.
        if self.silent && snapshot.container_id != 0 {
            self.gate.clear();
            return Some(self.end_silent(now));
        }
        if self.gate.interrupted(snapshot)
            || (self.opened
                && !self.silent
                && !snapshot.screen_open
                && self.gate.pending().is_some())
        {
            self.cancel();
            return None;
        }
        match self.gate.observe(snapshot) {
            Outcome::Waiting => return None,
            Outcome::Invalidated => {
                self.cancel();
                return None;
            }
            Outcome::Idle | Outcome::Applied | Outcome::Failed(_) => {}
        }
        if snapshot.container_id != 0 {
            return None;
        }
        // After a no-op swap nothing changed, so the same swap would repeat.
        // Wind down (closing an inventory this module opened) until the
        // content differs.
        if self.gate.stalled(snapshot) {
            return self.wind_down(snapshot, options.close);
        }
        let equipped = snapshot
            .slot(504)?
            .item
            .as_ref()
            .is_some_and(|i| i.fingerprint.id == TOTEM);
        if equipped {
            return self.wind_down(snapshot, options.close);
        }
        let sources: Vec<_> = snapshot
            .slots(|role| matches!(role, SlotRole::Main | SlotRole::Hotbar(_)))
            .filter(|slot| {
                slot.item
                    .as_ref()
                    .is_some_and(|i| i.fingerprint.id == TOTEM)
            })
            .collect();
        if sources.is_empty() {
            return self.wind_down(snapshot, options.close);
        }
        // A silent session clicks the player menu with no screen shown (or
        // through the player's own inventory screen if one was opened).
        let menu = snapshot.is_player_inventory || (self.silent && !snapshot.screen_open);
        if !menu {
            if !options.inventory_only && options.open && !snapshot.screen_open {
                if options.silent {
                    // `isSilentOpenBlocked`: another module's rotation claim
                    // that this one cannot take over.
                    if blocked() {
                        return None;
                    }
                    self.silent = true;
                    self.suppress = true;
                    self.action_at = Some(now);
                    return Some(Effect::SilentOpen);
                }
                self.opened = true;
                return Some(Effect::OpenInventory);
            }
            return None;
        }
        if snapshot.carried.is_some() {
            return None;
        }
        let source = if self.random_slot {
            sources[(self.next() % sources.len() as u64) as usize]
        } else {
            sources[0]
        };
        let request = self.gate.issue(
            snapshot,
            Request::for_slot(snapshot, source, Click::SwapOffhand),
        )?;
        if self.silent {
            self.action_at = Some(now);
            return Some(Effect::SilentRequest(request));
        }
        Some(Effect::Request(request))
    }
    fn wind_down(&mut self, snapshot: &Snapshot, close: bool) -> Option<Effect> {
        if self.silent {
            return Some(self.end_silent(snapshot.observed));
        }
        let opened = std::mem::take(&mut self.opened);
        (opened && close && snapshot.is_player_inventory).then_some(Effect::CloseInventory)
    }
    fn end_silent(&mut self, now: Instant) -> Effect {
        self.silent = false;
        self.action_at = Some(now);
        Effect::SilentClose
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::types::{Fingerprint, Item, ItemClass, Slot};
    use std::time::Duration;

    const OPTIONS: Options = Options {
        inventory_only: false,
        open: true,
        close: true,
        silent: false,
    };
    const SILENT: Options = Options {
        silent: true,
        ..OPTIONS
    };

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
            meta: Default::default(),
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
            vitals: Default::default(),
            observed: std::time::Instant::now(),
        }
    }
    #[test]
    fn swaps_once_and_waits_for_revision() {
        let mut module = AutoTotem::default();
        let mut s = snapshot();
        assert!(matches!(
            module.step(&s, OPTIONS, || false),
            Some(Effect::Request(Request {
                slot: 9,
                click: Click::SwapOffhand,
                ..
            }))
        ));
        assert!(module.step(&s, OPTIONS, || false).is_none());
        s.revision += 1;
        s.slots[1].item = Some(item("minecraft:totem_of_undying"));
        assert!(module.step(&s, OPTIONS, || false).is_none());
        assert!(!module.active());
    }
    #[test]
    fn no_op_swap_closes_its_inventory_and_does_not_reopen() {
        use crate::inventory::planner::DEADLINE;
        let mut module = AutoTotem::default();
        let mut s = snapshot();
        s.screen_open = false;
        s.is_player_inventory = false;
        assert!(matches!(
            module.step(&s, OPTIONS, || false),
            Some(Effect::OpenInventory)
        ));
        s.screen_open = true;
        s.is_player_inventory = true;
        assert!(matches!(
            module.step(&s, OPTIONS, || false),
            Some(Effect::Request(_))
        ));
        s.observed += DEADLINE;
        assert!(matches!(
            module.step(&s, OPTIONS, || false),
            Some(Effect::CloseInventory)
        ));
        assert!(!module.active());
        s.screen_open = false;
        s.is_player_inventory = false;
        assert!(module.step(&s, OPTIONS, || false).is_none());
        assert!(!module.active());
    }
    #[test]
    fn random_slot_count_and_delay() {
        let mut module = AutoTotem {
            random_slot: true,
            ..AutoTotem::default()
        };
        let mut s = snapshot();
        s.slots.push(Slot {
            index: 20,
            role: SlotRole::Main,
            item: Some(item("minecraft:totem_of_undying")),
        });
        assert_eq!(totem_count(&s), 2);
        assert_eq!(count_color(2), [255, 249, 18]);
        let mut picked = std::collections::BTreeSet::new();
        for _ in 0..32 {
            module.cancel();
            if let Some(Effect::Request(r)) = module.step(&s, OPTIONS, || false) {
                picked.insert(r.slot);
            }
        }
        assert_eq!(picked.len(), 2, "both totem slots are chosen");
        let plain: Vec<_> = (0..200).map(|_| module.delay((100, 120), false)).collect();
        assert!(plain.iter().all(|d| (100..=120).contains(d)));
        let extra: Vec<_> = (0..400).map(|_| module.delay((100, 120), true)).collect();
        assert!(extra.iter().all(|d| *d >= 100 && *d <= 120 + 350));
        assert!(extra.iter().any(|d| *d > 200), "occasional long pause");
        assert!(extra.iter().filter(|d| **d <= 120).count() > 250);
    }
    #[test]
    fn silent_open_swaps_without_a_screen_and_holds_input_one_delay_after() {
        let mut module = AutoTotem::default();
        let mut s = snapshot();
        s.screen_open = false;
        s.is_player_inventory = false;
        let start = s.observed;
        assert!(matches!(
            module.step(&s, SILENT, || false),
            Some(Effect::SilentOpen)
        ));
        assert!(module.silent() && module.active() && !module.opened());
        assert!(module.suppress_tick(start, (150, 150)));
        // The session's own snapshot (no screen, player menu) gets the swap.
        let request = match module.step(&s, SILENT, || false) {
            Some(Effect::SilentRequest(r)) => r,
            other => panic!("{other:?}"),
        };
        assert_eq!((request.slot, request.click), (9, Click::SwapOffhand));
        assert!(
            module.step(&s, SILENT, || false).is_none(),
            "one click at a time"
        );
        s.revision += 1;
        s.slots[0].item = Some(item("minecraft:shield"));
        s.slots[1].item = Some(item(TOTEM));
        s.observed = start + Duration::from_millis(100);
        assert!(module.suppress_tick(s.observed, (150, 150)));
        assert!(matches!(
            module.step(&s, SILENT, || false),
            Some(Effect::SilentClose)
        ));
        assert!(!module.silent() && !module.active());
        assert!(!module.take_close_owed(), "step's own close is the effect");
        // Held until one delay after the close, then released for good.
        assert!(module.suppress_tick(start + Duration::from_millis(240), (150, 150)));
        assert!(!module.suppress_tick(start + Duration::from_millis(250), (150, 150)));
        assert!(!module.suppressing());
        assert!(
            module.step(&s, SILENT, || false).is_none(),
            "no reopen once equipped"
        );
    }
    #[test]
    fn silent_open_waits_for_a_blocked_claim_and_yields_to_containers() {
        let mut module = AutoTotem::default();
        let mut s = snapshot();
        s.screen_open = false;
        s.is_player_inventory = false;
        assert!(module.step(&s, SILENT, || true).is_none());
        assert!(!module.silent() && !module.suppressing());
        assert!(module.step(&s, SILENT, || false).is_some());
        // A container replaces the player menu: the session ends without a
        // close of its own and no click goes to the container.
        s.container_id = 3;
        s.screen_open = true;
        s.is_chest = true;
        assert!(matches!(
            module.step(&s, SILENT, || false),
            Some(Effect::SilentClose)
        ));
        assert!(module.step(&s, SILENT, || false).is_none());
        // Inventory-only never opens anything.
        let mut module = AutoTotem::default();
        s.container_id = 0;
        s.screen_open = false;
        s.is_chest = false;
        let only = Options {
            inventory_only: true,
            ..SILENT
        };
        assert!(module.step(&s, only, || false).is_none());
    }
    #[test]
    fn cancelled_silent_session_owes_its_close_once() {
        let mut module = AutoTotem::default();
        let mut s = snapshot();
        s.screen_open = false;
        s.is_player_inventory = false;
        module.step(&s, SILENT, || false);
        assert!(matches!(
            module.step(&s, SILENT, || false),
            Some(Effect::SilentRequest(_))
        ));
        module.cancel();
        assert!(!module.silent() && !module.active());
        assert!(module.take_close_owed());
        assert!(!module.take_close_owed());
        module.release_input();
        assert!(!module.suppressing());
    }
    #[test]
    fn respects_cursor_and_other_container() {
        let mut module = AutoTotem::default();
        let mut s = snapshot();
        s.carried = Some(item("minecraft:shield"));
        assert!(module.step(&s, OPTIONS, || false).is_none());
        s.carried = None;
        s.is_player_inventory = false;
        assert!(module.step(&s, OPTIONS, || false).is_none());
    }
}
