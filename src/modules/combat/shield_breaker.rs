// ShieldBreaker (reference `ShieldBreaker`): an attack on a player whose
// shield is raised is cancelled; the hotbar axe is selected, and after the
// swap delay the axe attacks (twice with Double click), then after the swap
// back delay the original slot comes back.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit;
use super::weapon_swap::{Op, Source, View, select};
use serde_json::{Value, json};

pub const ID: &str = "combat.shield-breaker";

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let describe = |mut d: Definition, text: &str| {
        d.description = text.into();
        d
    };
    vec![
        describe(
            Definition::number(&key("swap-delay"), "Swap delay", 0.0, 5.0, 20.0, 1.0),
            "Delay between swapping to an axe and attacking",
        ),
        describe(
            Definition::boolean(&key("auto-swap-back"), "Auto swap back", true),
            "Sweeping back to the original slot",
        ),
        describe(
            Definition::number(
                &key("swap-back-delay"),
                "Swap back delay",
                0.0,
                5.0,
                20.0,
                1.0,
            ),
            "Delay between attacking and sweeping back to the original slot",
        )
        .when(&key("auto-swap-back"), json!(true)),
        describe(
            Definition::boolean(&key("double-click"), "Double click", false),
            "Attacks again immediately after breaking the shield to knock the target back",
        ),
        describe(
            Definition::boolean(&key("limit-items"), "Limit to items", false),
            "ShieldBreaker functions only while holding selected items",
        ),
        Definition::new(
            &key("allowed-items"),
            "Allowed Items",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!(["swords"]),
        )
        .when(&key("limit-items"), json!(true)),
    ]
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub swap_delay: i32,
    pub auto_swap_back: bool,
    pub swap_back_delay: i32,
    pub double_click: bool,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let id = |k: &str| format!("{ID}.{k}");
        let boolean = |k: &str| {
            settings
                .effective(&id(k))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };
        // `getTickValue`: the raw value, at least 0.
        let ticks = |k: &str| {
            settings
                .get(&id(k))
                .and_then(Value::as_f64)
                .map_or(0, |v| v.max(0.0) as i32)
        };
        Self {
            swap_delay: ticks("swap-delay"),
            auto_swap_back: boolean("auto-swap-back"),
            swap_back_delay: ticks("swap-back-delay"),
            double_click: boolean("double-click"),
            limit_items: boolean("limit-items"),
            allowed_items: settings
                .get(&id("allowed-items"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum State {
    #[default]
    Idle,
    WaitingToAttack,
    WaitingToSwapBack,
}

/// `requestSyntheticAttack(this)`: true when the press went through.
pub type Request<'a> = dyn FnMut(&mut View, &mut Vec<Op>) -> bool + 'a;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShieldBreaker {
    state: State,
    original_slot: Option<usize>,
    ticks_remaining: i32,
    attack_release_pending: bool,
}

fn axe_slot(view: &View) -> Option<usize> {
    view.hotbar.iter().position(|s| s.axe())
}

impl ShieldBreaker {
    pub fn active(&self) -> bool {
        self.state != State::Idle
    }
    /// `hasAxeInHotbar`.
    pub fn has_axe(view: &View) -> bool {
        axe_slot(view).is_some()
    }
    /// `resetState`; `onDisable` restores the slot.
    pub fn reset(&mut self, view: &mut View, ops: &mut Vec<Op>, restore_slot: bool) {
        if self.attack_release_pending {
            ops.push(Op::Release);
        }
        if restore_slot && let Some(slot) = self.original_slot {
            select(view, ops, slot);
        }
        *self = Self::default();
    }
    /// The attack mapping going down (skipped when already cancelled). True
    /// cancels the press.
    pub fn press(
        &mut self,
        options: &Options,
        view: &mut View,
        ops: &mut Vec<Op>,
        request: &mut Request,
    ) -> bool {
        self.handle_attack(options, view, ops, request)
    }
    /// `onSyntheticAttack` (skipped when already cancelled).
    pub fn synthetic(
        &mut self,
        options: &Options,
        source: Source,
        view: &mut View,
        ops: &mut Vec<Op>,
        request: &mut Request,
    ) -> bool {
        if matches!(
            source,
            Source::ShieldBreaker | Source::HitSwap | Source::AutoMace(true)
        ) {
            return false;
        }
        self.handle_attack(options, view, ops, request)
    }

    fn handle_attack(
        &mut self,
        options: &Options,
        view: &mut View,
        ops: &mut Vec<Op>,
        request: &mut Request,
    ) -> bool {
        if self.state != State::Idle || view.screen_open {
            return false;
        }
        let allowed =
            !options.limit_items || item_limit::matches(&options.allowed_items, &view.held().item);
        let raised = view
            .hit_player()
            .and_then(|t| t.shield)
            .is_some_and(|s| s.raised());
        if !allowed || !raised {
            return false;
        }
        if view.held().axe() {
            if options.double_click && self.attack_release_pending {
                ops.push(Op::Release);
                self.attack_release_pending = request(view, ops);
            }
            return false;
        }
        let Some(axe) = axe_slot(view) else {
            return false;
        };
        self.original_slot = Some(view.selected);
        select(view, ops, axe);
        self.state = State::WaitingToAttack;
        self.ticks_remaining = options.swap_delay;
        if self.ticks_remaining == 0 {
            self.attack_with_axe(options, view, ops, request);
        }
        true
    }

    fn attack_with_axe(
        &mut self,
        options: &Options,
        view: &mut View,
        ops: &mut Vec<Op>,
        request: &mut Request,
    ) {
        if !view.held().axe() {
            let Some(axe) = axe_slot(view) else {
                self.reset(view, ops, true);
                return;
            };
            select(view, ops, axe);
        }
        ops.push(Op::Release);
        self.attack_release_pending = request(view, ops);
        if options.double_click && self.attack_release_pending {
            ops.push(Op::Release);
            self.attack_release_pending = request(view, ops);
        }
        if options.auto_swap_back {
            self.state = State::WaitingToSwapBack;
            self.ticks_remaining = options.swap_back_delay;
        } else {
            self.original_slot = None;
            self.ticks_remaining = 0;
            self.state = State::Idle;
        }
    }

    /// `onTick` (EventPreTick).
    pub fn tick(
        &mut self,
        options: &Options,
        view: &mut View,
        ops: &mut Vec<Op>,
        request: &mut Request,
    ) {
        if view.screen_open {
            self.reset(view, ops, true);
            return;
        }
        if self.attack_release_pending {
            ops.push(Op::Release);
            self.attack_release_pending = false;
        }
        if self.state == State::Idle {
            return;
        }
        if self.state == State::WaitingToSwapBack && !options.auto_swap_back {
            self.reset(view, ops, false);
            return;
        }
        if self.ticks_remaining > 0 {
            self.ticks_remaining -= 1;
            if self.ticks_remaining > 0 {
                return;
            }
        }
        match self.state {
            State::WaitingToAttack => self.attack_with_axe(options, view, ops, request),
            State::WaitingToSwapBack => self.reset(view, ops, true),
            State::Idle => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::inventory::item_limit::HeldItem;
    use super::super::weapon_swap::{Crosshair, Shield, Slot, Target};
    use super::*;

    fn view(items: &[&str], elapsed: i32) -> View {
        let mut hotbar: Vec<Slot> = items
            .iter()
            .enumerate()
            .map(|(i, id)| Slot {
                item: HeldItem {
                    id: (!id.is_empty()).then(|| (*id).into()),
                    slot: i,
                    ..HeldItem::default()
                },
                ..Slot::default()
            })
            .collect();
        hotbar.resize(9, Slot::default());
        View {
            hotbar,
            crosshair: Some(Crosshair {
                player: Some(Target {
                    shield: Some(Shield {
                        using: elapsed > 0,
                        elapsed,
                    }),
                }),
                ..Crosshair::default()
            }),
            ..View::default()
        }
    }
    fn options() -> Options {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s.validate_conditions().unwrap();
        Options::read(&s)
    }
    fn press(b: &mut ShieldBreaker, o: &Options, v: &mut View) -> (bool, Vec<Op>) {
        let mut ops = Vec::new();
        let cancel = b.press(o, v, &mut ops, &mut |_, ops| {
            ops.push(Op::Press);
            true
        });
        (cancel, ops)
    }
    fn tick(b: &mut ShieldBreaker, o: &Options, v: &mut View) -> Vec<Op> {
        let mut ops = Vec::new();
        b.tick(o, v, &mut ops, &mut |_, ops| {
            ops.push(Op::Press);
            true
        });
        ops
    }

    #[test]
    fn defaults_match_the_reference() {
        let o = options();
        assert_eq!((o.swap_delay, o.swap_back_delay), (5, 5));
        assert!(o.auto_swap_back && !o.double_click && !o.limit_items);
        assert_eq!(o.allowed_items, vec!["swords".to_owned()]);
    }

    #[test]
    fn raised_shield_swaps_waits_attacks_and_swaps_back() {
        let o = Options {
            swap_delay: 2,
            swap_back_delay: 1,
            ..options()
        };
        let mut v = view(&["diamond_sword", "stone_axe"], 1);
        let mut b = ShieldBreaker::default();
        assert_eq!(press(&mut b, &o, &mut v), (true, vec![Op::Select(1)]));
        assert!(tick(&mut b, &o, &mut v).is_empty());
        assert_eq!(tick(&mut b, &o, &mut v), vec![Op::Release, Op::Press]);
        // The swing is released; the back delay of 1 runs out on this tick.
        assert_eq!(tick(&mut b, &o, &mut v), vec![Op::Release, Op::Select(0)]);
        assert!(!b.active());
        assert_eq!(v.selected, 0);
    }

    #[test]
    fn zero_delay_attacks_inside_the_press_and_double_clicks() {
        let o = Options {
            swap_delay: 0,
            double_click: true,
            auto_swap_back: false,
            ..options()
        };
        let mut v = view(&["diamond_sword", "", "iron_axe"], 3);
        let mut b = ShieldBreaker::default();
        assert_eq!(
            press(&mut b, &o, &mut v),
            (
                true,
                vec![
                    Op::Select(2),
                    Op::Release,
                    Op::Press,
                    Op::Release,
                    Op::Press
                ]
            )
        );
        // No swap back: idle, the axe stays and the swing is released.
        assert!(!b.active());
        assert_eq!(tick(&mut b, &o, &mut v), vec![Op::Release]);
        assert_eq!(v.selected, 2);
    }

    #[test]
    fn lowered_shields_axes_in_hand_screens_and_limits_do_nothing() {
        let o = options();
        let mut b = ShieldBreaker::default();
        assert!(!press(&mut b, &o, &mut view(&["stick", "iron_axe"], 0)).0);
        assert_eq!(
            press(&mut b, &o, &mut view(&["iron_axe"], 4)),
            (false, vec![])
        );
        assert!(!press(&mut b, &o, &mut view(&["stick"], 4)).0);
        let mut v = view(&["stick", "iron_axe"], 4);
        v.screen_open = true;
        assert!(!press(&mut b, &o, &mut v).0);
        let limited = Options {
            limit_items: true,
            ..o.clone()
        };
        assert!(!press(&mut b, &limited, &mut view(&["stick", "iron_axe"], 4)).0);
        // Its own and HitSwap's synthetic attacks pass through.
        let mut v = view(&["stick", "iron_axe"], 4);
        for source in [
            Source::ShieldBreaker,
            Source::HitSwap,
            Source::AutoMace(true),
        ] {
            assert!(!b.synthetic(&o, source, &mut v, &mut Vec::new(), &mut |_, _| true));
        }
        assert!(b.synthetic(
            &o,
            Source::TriggerBot,
            &mut v,
            &mut Vec::new(),
            &mut |_, _| true
        ));
    }

    #[test]
    fn opening_a_screen_or_disabling_restores_the_slot() {
        let o = options();
        let mut v = view(&["diamond_sword", "stone_axe"], 2);
        let mut b = ShieldBreaker::default();
        assert!(press(&mut b, &o, &mut v).0);
        v.screen_open = true;
        assert_eq!(tick(&mut b, &o, &mut v), vec![Op::Select(0)]);
        assert!(!b.active());
        // Turning Auto swap back off while waiting to swap back leaves the axe.
        let o = Options {
            swap_delay: 0,
            ..options()
        };
        let mut v = view(&["diamond_sword", "stone_axe"], 2);
        assert!(press(&mut b, &o, &mut v).0);
        let off = Options {
            auto_swap_back: false,
            ..o
        };
        assert_eq!(tick(&mut b, &off, &mut v), vec![Op::Release]);
        assert_eq!(v.selected, 1);
    }
}
