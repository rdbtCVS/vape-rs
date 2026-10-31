use super::super::super::{
    actions::HotbarSlot,
    core::lifecycle::{EntityHandle, Epoch},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Block([i32; 3]),
    Entity(EntityHandle),
}
#[derive(Clone, Debug)]
pub struct Candidate {
    pub item: Option<String>,
    pub score: f32,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub epoch: Epoch,
    pub tick: u64,
    pub selected: HotbarSlot,
    pub target: Option<Target>,
    pub candidates: [Candidate; 9],
    pub attack_down: bool,
    pub sneaking: bool,
    pub using_item: bool,
    pub combat_blocked: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub swap_weapon: bool,
    pub instant_weapon: bool,
    pub swap_to_ticks: u64,
    pub swap_back: bool,
    pub swap_back_ticks: u64,
    pub require_attack: bool,
    pub only_sneaking: bool,
    pub not_during_use: bool,
    pub not_during_combat: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            swap_weapon: true,
            instant_weapon: true,
            swap_to_ticks: 1,
            swap_back: false,
            swap_back_ticks: 7,
            require_attack: true,
            only_sneaking: false,
            not_during_use: false,
            not_during_combat: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Select,
    Restore,
}
#[derive(Clone, Debug)]
pub struct Decision {
    pub selected: HotbarSlot,
    pub expected: HotbarSlot,
    pub item: Option<String>,
    pub kind: Kind,
}
#[derive(Debug, Default)]
pub struct AutoTool {
    epoch: Option<Epoch>,
    pending: Option<u64>,
    back_at: Option<u64>,
    original: Option<HotbarSlot>,
    owned_selection: Option<HotbarSlot>,
    interrupted: Option<Target>,
}
impl AutoTool {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn retains_claim(&self) -> bool {
        self.original.is_some() && self.owned_selection.is_some()
    }
    pub fn interrupt(&mut self, target: Option<Target>) {
        self.reset();
        self.interrupted = target;
    }
    pub fn step(&mut self, snapshot: &Snapshot, options: Options) -> Option<Decision> {
        if self.epoch.is_some_and(|epoch| epoch != snapshot.epoch) {
            self.reset();
        }
        self.epoch = Some(snapshot.epoch);
        if let Some(owned) = self.owned_selection
            && owned != snapshot.selected
        {
            self.interrupt(snapshot.target);
            self.epoch = Some(snapshot.epoch);
        }
        if self.interrupted.is_some() {
            if self.interrupted == snapshot.target
                && (!options.require_attack || snapshot.attack_down)
            {
                return None;
            }
            self.interrupted = None;
        }
        if (options.not_during_combat && snapshot.combat_blocked)
            || (options.not_during_use && snapshot.using_item)
        {
            // The reference returns before touching its state: a pending swap
            // or swap back keeps its timer and runs once the guard lifts.
            return None;
        }
        let eligible = snapshot.target.is_some()
            && (!options.require_attack || snapshot.attack_down)
            && (!options.only_sneaking || snapshot.sneaking)
            && (options.swap_weapon || !matches!(snapshot.target, Some(Target::Entity(_))));
        let best = if eligible {
            let mut best = None;
            let mut score = 1.0;
            for (i, candidate) in snapshot.candidates.iter().enumerate() {
                if candidate.item.is_some()
                    && candidate.score.is_finite()
                    && candidate.score > score
                {
                    score = candidate.score;
                    best = HotbarSlot::new(i);
                }
            }
            best
        } else {
            None
        };
        if !options.swap_back {
            self.original = None;
            self.owned_selection = None;
            self.back_at = None;
        }
        if self.back_at.is_none() && best.is_none() && self.original.is_some() && options.swap_back
        {
            self.back_at = Some(snapshot.tick.saturating_add(options.swap_back_ticks));
        }
        let (selected, kind) = if let Some(deadline) = self.back_at {
            if snapshot.tick < deadline {
                return None;
            }
            (self.original?, Kind::Restore)
        } else {
            // A swap is scheduled when a better slot appears and then runs
            // when "Swap to delay" has passed (at once for "Instant swap" on
            // an entity), against whatever slot is best by then; the
            // schedule survives ticks without a best slot.
            if self.pending.is_none() && best.is_some_and(|best| best != snapshot.selected) {
                self.pending = Some(snapshot.tick.saturating_add(options.swap_to_ticks));
            }
            let deadline = self.pending?;
            let instant = options.swap_weapon
                && options.instant_weapon
                && matches!(snapshot.target, Some(Target::Entity(_)));
            if !instant && snapshot.tick < deadline {
                return None;
            }
            let Some(best) = best.filter(|best| *best != snapshot.selected) else {
                self.pending = None;
                return None;
            };
            (best, Kind::Select)
        };
        Some(Decision {
            selected,
            expected: snapshot.selected,
            item: snapshot.candidates[selected.index()].item.clone(),
            kind,
        })
    }
    pub fn applied(&mut self, decision: &Decision, options: Options) {
        if decision.kind == Kind::Restore {
            self.original = None;
            self.owned_selection = None;
            self.back_at = None;
        } else {
            self.pending = None;
            if options.swap_back {
                self.original.get_or_insert(decision.expected);
                self.owned_selection = Some(decision.selected);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::super::super::core::lifecycle::Session;
    use super::*;
    fn snapshot() -> Snapshot {
        let mut s = Snapshot {
            epoch: Session::default().epoch(),
            tick: 10,
            selected: HotbarSlot::new(0).unwrap(),
            target: Some(Target::Block([1, 64, 1])),
            candidates: std::array::from_fn(|_| Candidate {
                item: None,
                score: 1.0,
            }),
            attack_down: true,
            sneaking: false,
            using_item: false,
            combat_blocked: false,
        };
        s.candidates[2] = Candidate {
            item: Some("minecraft:diamond_pickaxe".into()),
            score: 8.0,
        };
        s
    }
    #[test]
    fn delayed_selection_and_conditional_swap_back_follow_actual_selection() {
        let mut module = AutoTool::default();
        let mut s = snapshot();
        let options = Options {
            swap_back: true,
            ..Options::default()
        };
        assert!(module.step(&s, options).is_none());
        s.tick += 1;
        let change = module.step(&s, options).unwrap();
        assert_eq!(change.selected.index(), 2);
        module.applied(&change, options);
        s.selected = change.selected;
        s.target = None;
        assert!(module.step(&s, options).is_none());
        s.tick += 6;
        assert!(module.step(&s, options).is_none());
        s.tick += 1;
        let back = module.step(&s, options).unwrap();
        assert_eq!(back.selected.index(), 0);
        assert_eq!(back.kind, Kind::Restore);
        module.applied(&back, options);
        assert!(!module.retains_claim());
    }
    #[test]
    fn manual_override_and_session_changes_cancel_future_swaps() {
        let mut module = AutoTool::default();
        let mut s = snapshot();
        let options = Options {
            swap_to_ticks: 0,
            swap_back: true,
            ..Options::default()
        };
        let change = module.step(&s, options).unwrap();
        module.applied(&change, options);
        s.selected = HotbarSlot::new(3).unwrap();
        s.tick += 100;
        assert!(module.step(&s, options).is_none());
        assert!(!module.retains_claim());
        s.target = None;
        assert!(module.step(&s, options).is_none());
        s.target = Some(Target::Block([1, 64, 1]));
        let change = module.step(&s, options).unwrap();
        module.applied(&change, options);
        let mut session = Session::default();
        session.advance();
        s.epoch = session.epoch();
        s.target = None;
        assert!(module.step(&s, options).is_none());
        assert!(!module.retains_claim());
    }
    #[test]
    fn pending_swaps_keep_their_timer_through_guards_and_gaps() {
        let mut module = AutoTool::default();
        let mut s = snapshot();
        let options = Options {
            swap_to_ticks: 4,
            not_during_use: true,
            ..Options::default()
        };
        assert!(module.step(&s, options).is_none());
        // Eating does not cancel the schedule; it only holds the swap.
        s.tick += 4;
        s.using_item = true;
        assert!(module.step(&s, options).is_none());
        s.using_item = false;
        s.tick += 1;
        assert_eq!(module.step(&s, options).unwrap().selected.index(), 2);
        // A schedule survives a pause in mining and swaps at once when
        // mining resumes after its delay.
        let mut module = AutoTool::default();
        let mut s = snapshot();
        assert!(module.step(&s, options).is_none());
        s.attack_down = false;
        s.tick += 2;
        assert!(module.step(&s, options).is_none());
        s.attack_down = true;
        s.tick += 2;
        assert_eq!(module.step(&s, options).unwrap().selected.index(), 2);
        // Maturing without a best slot drops it; the next one waits again.
        let mut module = AutoTool::default();
        let mut s = snapshot();
        assert!(module.step(&s, options).is_none());
        s.attack_down = false;
        s.tick += 5;
        assert!(module.step(&s, options).is_none());
        assert!(module.pending.is_none());
        s.attack_down = true;
        assert!(module.step(&s, options).is_none());
        s.tick += 4;
        assert_eq!(module.step(&s, options).unwrap().selected.index(), 2);
    }
    #[test]
    fn weapon_instant_and_guards_reject_invalid_candidates() {
        let mut module = AutoTool::default();
        let mut s = snapshot();
        let options = Options::default();
        s.target = Some(Target::Entity(EntityHandle {
            epoch: s.epoch,
            id: 7,
        }));
        assert_eq!(module.step(&s, options).unwrap().selected.index(), 2);
        module.reset();
        s.using_item = true;
        assert!(
            module
                .step(
                    &s,
                    Options {
                        not_during_use: true,
                        ..options
                    }
                )
                .is_none()
        );
        s.using_item = false;
        s.candidates[2].score = f32::NAN;
        assert!(module.step(&s, options).is_none());
        s.candidates[2].score = 8.0;
        s.attack_down = false;
        assert!(module.step(&s, options).is_none());
        s.attack_down = true;
        assert!(
            module
                .step(
                    &s,
                    Options {
                        only_sneaking: true,
                        ..options
                    }
                )
                .is_none()
        );
        s.combat_blocked = true;
        assert!(
            module
                .step(
                    &s,
                    Options {
                        not_during_combat: true,
                        ..options
                    }
                )
                .is_none()
        );
    }
}
