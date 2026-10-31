// Runtime side of the modern-item swap modules: HitSwap, AutoMace and
// ShieldBreaker, dispatched in the reference's handler order (all HIGH, in
// mod registration order) with `SyntheticAttackRequestEvent` cancellation:
// HitSwap ignores it, AutoMace only resets its input timer once cancelled
// and ShieldBreaker skips cancelled events.
use super::super::modules::combat::{
    auto_mace::{self, AutoMace},
    hit_swap::{self, HitSwap},
    shield_breaker::{self, ShieldBreaker},
    trigger_bot::Cooldown,
    weapon_swap::{Op, Source, Target, View},
};
use super::super::targeting::entities::EntityView;
use super::super::targeting::filter::People;
use super::settings::Definition;
use super::{ModuleId, Runtime};

#[derive(Debug)]
pub struct ModernCombat {
    hit_swap: HitSwap,
    auto_mace: AutoMace,
    shield_breaker: ShieldBreaker,
    /// Steps a disable produced (a pending release, ShieldBreaker's slot
    /// restore) and EventPostAttack's follow-up, which waits for the attack
    /// it follows to finish. The host applies them before its next frame.
    pending: Vec<Op>,
}
impl Default for ModernCombat {
    fn default() -> Self {
        Self {
            hit_swap: HitSwap::default(),
            auto_mace: AutoMace::new(0x9e37_79b9_7f4a_7c15),
            shield_breaker: ShieldBreaker::default(),
            pending: Vec::new(),
        }
    }
}

pub fn definitions() -> Vec<Definition> {
    hit_swap::definitions()
        .into_iter()
        .chain(auto_mace::definitions())
        .chain(shield_breaker::definitions())
        .collect()
}

/// What the host sees for one dispatch.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub view: View,
    /// The crosshair entity, for AutoMace's target filter.
    pub entity: Option<EntityView>,
    /// Other players within 5 blocks and under 90 degrees of yaw, in world
    /// order, by name (`RotationUtil.m(player, 5, 90, true)` skips friends).
    pub nearby: Vec<(String, Target)>,
}

/// The enabled modules' options.
#[derive(Default)]
struct Enabled {
    hit_swap: Option<hit_swap::Options>,
    auto_mace: Option<auto_mace::Options>,
    shield_breaker: Option<shield_breaker::Options>,
}

/// A request no ported handler intercepts: HitSwap's, ShieldBreaker's and
/// AutoMace's in-progress follow-up go straight to the press.
fn press(_: &mut View, ops: &mut Vec<Op>) -> bool {
    ops.push(Op::Press);
    true
}

/// `requestSyntheticAttack(source)` for a source the swap modules handle:
/// true when no handler cancelled it (the caller then presses).
fn dispatch(
    hit_swap: &mut HitSwap,
    auto_mace: Option<&mut AutoMace>,
    shield_breaker: &mut ShieldBreaker,
    enabled: &Enabled,
    source: Source,
    view: &mut View,
    ops: &mut Vec<Op>,
) -> bool {
    let mut cancelled = false;
    if let Some(o) = &enabled.hit_swap {
        cancelled |= hit_swap.synthetic(o, source, view, ops);
    }
    if let (Some(o), Some(auto_mace)) = (&enabled.auto_mace, auto_mace) {
        cancelled |= auto_mace.synthetic(o, source, view, ops, cancelled);
    }
    if !cancelled && let Some(o) = &enabled.shield_breaker {
        cancelled |= shield_breaker.synthetic(o, source, view, ops, &mut press);
    }
    !cancelled
}

impl Runtime {
    fn modern_enabled(&self) -> Enabled {
        let on = |m| self.enabled(m) && self.session().in_world();
        Enabled {
            hit_swap: on(ModuleId::HitSwap).then(|| hit_swap::Options::read(&self.settings)),
            auto_mace: on(ModuleId::AutoMace).then(|| auto_mace::Options::read(&self.settings)),
            shield_breaker: on(ModuleId::ShieldBreaker)
                .then(|| shield_breaker::Options::read(&self.settings)),
        }
    }
    /// Fills what the runtime decides: the nearest non-friend player and
    /// whether AutoMace's target filter takes the crosshair entity.
    fn modern_view(&self, frame: Frame, enabled: &Enabled) -> View {
        let people = self.target_people();
        let mut view = frame.view;
        // The click GUI is a screen and turns `isInputEnabled` off.
        view.screen_open |= self.gui.open;
        view.input_enabled = !self.gui.open;
        view.nearby_player = frame
            .nearby
            .iter()
            .find(|(name, _)| !people.friend(name))
            .map(|(_, t)| *t);
        if let Some(c) = view.crosshair.as_mut() {
            c.mace_target = enabled.auto_mace.as_ref().is_some_and(|o| {
                frame
                    .entity
                    .as_ref()
                    .is_some_and(|e| o.filter.valid(e, &people))
            });
        }
        view
    }

    pub(super) fn modern_combat_disabled(&mut self, module: ModuleId) {
        let c = &mut self.modern_combat;
        match module {
            ModuleId::HitSwap => c.hit_swap.reset(&mut c.pending),
            ModuleId::AutoMace => c.auto_mace.reset(&mut c.pending),
            ModuleId::ShieldBreaker => {
                c.shield_breaker
                    .reset(&mut View::default(), &mut c.pending, true)
            }
            _ => {}
        }
    }
    /// Session cleanup: nothing survives a world change, and nothing from
    /// the old world is applied to the new one.
    pub(super) fn modern_combat_session_reset(&mut self) {
        let c = &mut self.modern_combat;
        c.hit_swap.reset(&mut Vec::new());
        c.auto_mace.reset(&mut Vec::new());
        c.shield_breaker
            .reset(&mut View::default(), &mut Vec::new(), false);
        c.pending.clear();
    }

    /// Steps waiting for the host, applied before it builds its next frame.
    pub fn modern_combat_take_pending(&mut self) -> Vec<Op> {
        std::mem::take(&mut self.modern_combat.pending)
    }

    /// Any swap module is on, or steps wait to be applied.
    pub fn modern_combat_wanted(&self) -> bool {
        let on = [
            ModuleId::HitSwap,
            ModuleId::AutoMace,
            ModuleId::ShieldBreaker,
        ]
        .into_iter()
        .any(|m| self.enabled(m));
        (on && self.session().in_world()) || !self.modern_combat.pending.is_empty()
    }

    /// The physical attack mapping went down; `cancelled` when TriggerBot
    /// already swallowed it (HitSwap still sees it, AutoMace only resets its
    /// input timer). True cancels the press.
    pub fn modern_combat_press(&mut self, frame: Frame, cancelled: bool) -> (bool, Vec<Op>) {
        let enabled = self.modern_enabled();
        let mut view = self.modern_view(frame, &enabled);
        let c = &mut self.modern_combat;
        let mut ops = Vec::new();
        let mut cancelled = cancelled;
        if let Some(o) = &enabled.hit_swap {
            cancelled |= c.hit_swap.press(o, &mut view, &mut ops);
        }
        if let Some(o) = &enabled.auto_mace {
            cancelled |= c.auto_mace.press(o, &mut view, &mut ops, cancelled);
        }
        if !cancelled && let Some(o) = &enabled.shield_breaker {
            cancelled |= c.shield_breaker.press(o, &mut view, &mut ops, &mut press);
        }
        (cancelled, ops)
    }

    /// Another module (TriggerBot, AutoClicker) asks for a synthetic attack.
    /// True when it may press; the caller makes the press itself.
    pub fn modern_combat_request(&mut self, source: Source, frame: Frame) -> (bool, Vec<Op>) {
        let enabled = self.modern_enabled();
        let mut view = self.modern_view(frame, &enabled);
        let c = &mut self.modern_combat;
        let mut ops = Vec::new();
        let allowed = dispatch(
            &mut c.hit_swap,
            Some(&mut c.auto_mace),
            &mut c.shield_breaker,
            &enabled,
            source,
            &mut view,
            &mut ops,
        );
        (allowed, ops)
    }

    /// `EventPreTick`, HitSwap, AutoMace then ShieldBreaker.
    pub fn modern_combat_tick(&mut self, frame: Frame, cooldown: Cooldown) -> Vec<Op> {
        let enabled = self.modern_enabled();
        let mut view = self.modern_view(frame, &enabled);
        let ModernCombat {
            hit_swap,
            auto_mace,
            shield_breaker,
            ..
        } = &mut self.modern_combat;
        let mut ops = Vec::new();
        if let Some(o) = &enabled.hit_swap {
            hit_swap.tick(o, &mut view, &mut ops, &mut press);
        }
        if let Some(o) = &enabled.auto_mace {
            // The follow-up (marked in progress) passes; Attack's own press
            // is offered to HitSwap and ShieldBreaker first.
            let mut request = |view: &mut View, ops: &mut Vec<Op>, mark: bool| {
                if !mark
                    && !dispatch(
                        hit_swap,
                        None,
                        shield_breaker,
                        &enabled,
                        Source::AutoMace(false),
                        view,
                        ops,
                    )
                {
                    return false;
                }
                press(view, ops)
            };
            auto_mace.tick(o, &mut view, &mut ops, cooldown, &mut request);
        }
        if let Some(o) = &enabled.shield_breaker {
            shield_breaker.tick(o, &mut view, &mut ops, &mut press);
        }
        ops
    }

    /// `EventPostAttack`: the attack on entity `target` is under way. Its
    /// follow-up (the mace after Stun slam's axe) joins the pending steps.
    pub fn modern_combat_post_attack(&mut self, frame: Frame, target: i32) {
        let enabled = self.modern_enabled();
        let mut view = self.modern_view(frame, &enabled);
        let c = &mut self.modern_combat;
        if enabled.auto_mace.is_some() {
            c.auto_mace
                .post_attack(&mut view, &mut c.pending, target, &mut |view, ops, _| {
                    press(view, ops)
                });
        }
    }

    /// TriggerBot's `canBypassCooldown` and whether its Shield check gives
    /// way (ShieldBreaker or HitSwap with an axe, AutoMace's Stun slam).
    pub fn modern_combat_trigger_assist(&self, frame: Frame) -> (bool, bool) {
        let enabled = self.modern_enabled();
        let view = self.modern_view(frame, &enabled);
        let bypass = view.will_smash
            && if enabled
                .auto_mace
                .as_ref()
                .is_some_and(|o| AutoMace::has_ready_mace(o, &view))
            {
                true
            } else if let Some(o) = &enabled.hit_swap {
                o.has_valid_weapon_swap(&view)
            } else {
                view.held().mace()
            };
        let exempt = (enabled.shield_breaker.is_some() && ShieldBreaker::has_axe(&view))
            || enabled
                .hit_swap
                .as_ref()
                .is_some_and(|o| o.has_alternate_axe(&view))
            || enabled
                .auto_mace
                .as_ref()
                .is_some_and(|o| AutoMace::can_handle_mace_attack(o, &view));
        (bypass, exempt)
    }

    /// A swap module cancelled TriggerBot's synthetic press.
    pub fn trigger_bot_attack_cancelled(&mut self) {
        self.trigger_bot.attack_cancelled();
    }

    /// AutoMace's suffix: "Auto" or the Mace type.
    pub fn auto_mace_suffix(&self) -> &'static str {
        auto_mace::Options::read(&self.settings).suffix()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::inventory::item_limit::HeldItem;
    use super::super::super::modules::combat::weapon_swap::{Crosshair, Shield, Slot};
    use super::super::lifecycle::Context;
    use super::*;

    fn runtime() -> Runtime {
        let mut runtime = Runtime::default();
        runtime.observe_context(Context {
            in_world: true,
            alive: true,
            interactive: true,
        });
        runtime
    }

    fn frame(items: &[&str], selected: usize) -> Frame {
        let mut hotbar: Vec<Slot> = items
            .iter()
            .enumerate()
            .map(|(i, id)| Slot {
                item: HeldItem {
                    id: (!id.is_empty()).then(|| (*id).into()),
                    slot: i,
                    ..HeldItem::default()
                },
                density: i32::from(*id == "mace"),
                ..Slot::default()
            })
            .collect();
        hotbar.resize(9, Slot::default());
        let shield = Target {
            shield: Some(Shield {
                using: true,
                elapsed: 8,
            }),
        };
        Frame {
            view: View {
                hotbar,
                selected,
                crosshair: Some(Crosshair {
                    id: 7,
                    player: Some(shield),
                    ..Crosshair::default()
                }),
                input_enabled: true,
                ..View::default()
            },
            entity: None,
            nearby: vec![("Friend".into(), shield)],
        }
    }

    #[test]
    fn shield_breaker_cancels_presses_and_requests_and_restores_on_disable() {
        let mut runtime = runtime();
        runtime.set_enabled(ModuleId::ShieldBreaker, true);
        // ShieldBreaker alone: a raised shield takes the axe and cancels.
        let (cancel, ops) =
            runtime.modern_combat_press(frame(&["diamond_sword", "iron_axe"], 0), false);
        assert!(cancel);
        assert_eq!(ops, vec![Op::Select(1)]);
        // TriggerBot's request is cancelled the same way while it waits.
        runtime.modern_combat_session_reset();
        let (allowed, _) = runtime
            .modern_combat_request(Source::TriggerBot, frame(&["diamond_sword", "iron_axe"], 0));
        assert!(!allowed);
        // Its own requests are never intercepted.
        let (allowed, _) = runtime.modern_combat_request(
            Source::ShieldBreaker,
            frame(&["diamond_sword", "iron_axe"], 1),
        );
        assert!(allowed);
        let (_, exempt) = runtime.modern_combat_trigger_assist(frame(&["iron_axe", "stick"], 1));
        assert!(exempt);
        // Disabling leaves the slot restore for the host.
        runtime.set_enabled(ModuleId::ShieldBreaker, false);
        assert!(runtime.modern_combat_wanted());
        assert_eq!(runtime.modern_combat_take_pending(), vec![Op::Select(0)]);
        assert!(!runtime.modern_combat_wanted());
        assert!(
            runtime
                .modern_combat_tick(frame(&["diamond_sword"], 1), Cooldown::default())
                .is_empty()
        );
    }

    #[test]
    fn friends_are_not_the_nearby_player_and_bypass_needs_a_smash() {
        let mut runtime = runtime();
        runtime.document.0["friends"] = serde_json::json!([{ "name": "Friend" }]);
        runtime.set_enabled(ModuleId::HitSwap, true);
        let f = frame(&["diamond_sword"], 0);
        let view = runtime.modern_view(f.clone(), &runtime.modern_enabled());
        assert_eq!(view.nearby_player, None);
        // HitSwap off: a held mace bypasses only while smashing.
        runtime.set_enabled(ModuleId::HitSwap, false);
        let mut f = frame(&["mace"], 0);
        assert!(!runtime.modern_combat_trigger_assist(f.clone()).0);
        f.view.will_smash = true;
        assert!(runtime.modern_combat_trigger_assist(f).0);
        assert_eq!(runtime.auto_mace_suffix(), "Density");
    }
}
