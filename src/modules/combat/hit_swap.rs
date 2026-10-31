// HitSwap (reference `HitSwap`): an attack press (physical, or another
// module's synthetic attack) is cancelled and replayed on the next tick from
// a better hotbar weapon, then the original slot comes back. The weapon is
// an axe against a player whose shield has been up for more than 5 ticks, a
// mace (only when the hit will smash, and of the configured Density/Breach
// enchantment when the hotbar has one), or a sword. Stun slam hits a
// shielding player with an axe first and follows with a mace slam.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit;
use super::weapon_swap::{Op, Slot, Source, View, select};
use serde_json::{Value, json};

pub const ID: &str = "combat.hit-swap";

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let describe = |mut d: Definition, text: &str| {
        d.description = text.into();
        d
    };
    let maces = key("maces");
    vec![
        Definition::boolean(&maces, "Maces", true),
        describe(
            Definition::boolean(&key("smash-only"), "Smash only", true),
            "Only swap to mace if will smash",
        )
        .when(&maces, json!(true)),
        describe(
            Definition::boolean(&key("breach-maces"), "Breach maces", false),
            "Will use Maces with Breach enchantment",
        )
        .when(&maces, json!(true)),
        // The reference repeats the Breach text for Density.
        describe(
            Definition::boolean(&key("density-maces"), "Density maces", true),
            "Will use Maces with Breach enchantment",
        )
        .when(&maces, json!(true)),
        describe(
            Definition::boolean(&key("stun-slam"), "Stun slam", false),
            "When holding an axe and attacking a shielded player:\nHits with axe first (breaks shield), then swaps to mace for a follow-up slam",
        )
        .when(&maces, json!(true)),
        Definition::boolean(&key("axes"), "Axes", true),
        Definition::boolean(&key("swords"), "Swords", false),
        Definition::boolean(&key("limit-items"), "Limit to items", false),
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
    pub maces: bool,
    pub smash_only: bool,
    pub breach_maces: bool,
    pub density_maces: bool,
    pub stun_slam: bool,
    pub axes: bool,
    pub swords: bool,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
}
impl Options {
    /// Effective values: the mace options read false while Maces is off.
    pub fn read(settings: &Settings) -> Self {
        let id = |k: &str| format!("{ID}.{k}");
        let boolean = |k: &str| {
            settings
                .effective(&id(k))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };
        Self {
            maces: boolean("maces"),
            smash_only: boolean("smash-only"),
            breach_maces: boolean("breach-maces"),
            density_maces: boolean("density-maces"),
            stun_slam: boolean("stun-slam"),
            axes: boolean("axes"),
            swords: boolean("swords"),
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

    /// `isValidMace`.
    fn valid_mace(&self, view: &View, slot: &Slot, require_enchantment: bool) -> bool {
        if self.smash_only && !view.will_smash {
            return false;
        }
        if !slot.mace() {
            return false;
        }
        if !require_enchantment {
            return true;
        }
        if self.density_maces && slot.density > 0 {
            return true;
        }
        if self.breach_maces && slot.breach > 0 {
            return true;
        }
        !self.density_maces && !self.breach_maces
    }
    /// `shouldFilterByEnchant`: an enchantment is chosen and the hotbar has
    /// such a mace (otherwise any mace will do).
    fn filter_by_enchant(&self, view: &View) -> bool {
        (self.density_maces || self.breach_maces)
            && view.hotbar.iter().any(|s| self.valid_mace(view, s, true))
    }
    fn mace_slot(&self, view: &View) -> Option<usize> {
        let filter = self.filter_by_enchant(view);
        view.hotbar
            .iter()
            .position(|s| self.valid_mace(view, s, filter))
    }
    /// `findAxeSlot`: another hotbar slot with an axe.
    fn axe_slot(&self, view: &View) -> Option<usize> {
        (0..view.hotbar.len()).find(|&i| i != view.selected && view.hotbar[i].axe())
    }
    /// `findWeaponSlot`: Ok(None) is -1 (no weapon), Err is -2 (the target's
    /// shield is still warming up: no swap).
    fn weapon_slot(&self, view: &View) -> Result<Option<usize>, ()> {
        let filter = self.filter_by_enchant(view);
        let shield = view.nearby_player.and_then(|t| t.shield);
        for (i, stack) in view.hotbar.iter().enumerate() {
            if i == view.selected || stack.empty() {
                continue;
            }
            if self.axes
                && stack.axe()
                && let Some(shield) = shield
                && shield.using
            {
                return if shield.blocking() {
                    Ok(Some(i))
                } else {
                    Err(())
                };
            }
            if self.maces && self.valid_mace(view, stack, filter) {
                return Ok(Some(i));
            }
            if self.swords && stack.sword() {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }
    /// `canHandleAttack`.
    fn can_handle(&self, view: &View) -> bool {
        !view.screen_open
            && (!self.limit_items || item_limit::matches(&self.allowed_items, &view.held().item))
    }
    /// `hasAlternateAxe`, for the modules that defer to HitSwap.
    pub fn has_alternate_axe(&self, view: &View) -> bool {
        self.axes && self.axe_slot(view).is_some()
    }
    /// `hasValidWeaponSwap`: the weapon HitSwap would take is a mace.
    pub fn has_valid_weapon_swap(&self, view: &View) -> bool {
        let Ok(Some(slot)) = self.weapon_slot(view) else {
            return false;
        };
        self.valid_mace(view, &view.hotbar[slot], self.filter_by_enchant(view))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HitSwap {
    swap_active: bool,
    tick_counter: i32,
    original_slot: Option<usize>,
    swap_attack_pending: bool,
    swing_release_pending: bool,
    breach_swap_pending: bool,
    stun_slam_active: bool,
    stun_slam_second_phase: bool,
}

impl HitSwap {
    pub fn active(&self) -> bool {
        self.swap_active
    }
    /// `onDisable` (also a world change): the slot is not restored; a
    /// pending swing is released.
    pub fn reset(&mut self, ops: &mut Vec<Op>) {
        if self.swing_release_pending {
            ops.push(Op::Release);
        }
        *self = Self::default();
    }
    /// `onMouseButton`/`onKeyPress` for the attack mapping going down. True
    /// cancels the press.
    pub fn press(&mut self, options: &Options, view: &mut View, ops: &mut Vec<Op>) -> bool {
        self.handle_attack(options, view, ops)
    }
    /// `onSyntheticAttack`: another module's attack. True cancels it.
    pub fn synthetic(
        &mut self,
        options: &Options,
        source: Source,
        view: &mut View,
        ops: &mut Vec<Op>,
    ) -> bool {
        if matches!(
            source,
            Source::HitSwap | Source::ShieldBreaker | Source::AutoMace(true)
        ) {
            return false;
        }
        self.handle_attack(options, view, ops)
    }

    fn handle_attack(&mut self, options: &Options, view: &mut View, ops: &mut Vec<Op>) -> bool {
        if self.swap_active
            || !options.can_handle(view)
            || view.using_item
            || view.crosshair.is_none()
        {
            return false;
        }
        let held = view.held();
        let target = view.nearby_player;
        let mace = options.mace_slot(view);
        if options.stun_slam
            && options.maces
            && !held.empty()
            && target.is_some_and(|t| t.shield.is_some_and(|s| s.blocking()))
            && mace.is_some()
        {
            if held.axe() {
                // The axe hit goes through; the mace follows next tick.
                self.original_slot = Some(view.selected);
                self.breach_swap_pending = true;
                self.swap_active = true;
                self.tick_counter = 0;
                return false;
            }
            if let Some(axe) = options.axe_slot(view) {
                self.original_slot = Some(view.selected);
                select(view, ops, axe);
                self.stun_slam_active = true;
                self.swap_active = true;
                self.tick_counter = 0;
                self.swap_attack_pending = true;
                return true;
            }
        }
        if let Ok(Some(weapon)) = options.weapon_slot(view) {
            self.original_slot = Some(view.selected);
            select(view, ops, weapon);
            self.swap_active = true;
            self.tick_counter = 0;
            self.swap_attack_pending = true;
            return true;
        }
        false
    }

    /// `onTick` (EventPreTick). `request` is `requestSyntheticAttack(this)`.
    pub fn tick(
        &mut self,
        options: &Options,
        view: &mut View,
        ops: &mut Vec<Op>,
        request: &mut dyn FnMut(&mut View, &mut Vec<Op>) -> bool,
    ) {
        if self.swing_release_pending {
            ops.push(Op::Release);
            self.swing_release_pending = false;
        }
        if self.breach_swap_pending && self.swap_active && self.tick_counter >= 1 {
            if let Some(mace) = options.mace_slot(view) {
                select(view, ops, mace);
                ops.push(Op::Release);
                request(view, ops);
                self.swing_release_pending = true;
            }
            self.breach_swap_pending = false;
            return;
        }
        if self.swap_attack_pending && self.swap_active {
            ops.push(Op::Release);
            request(view, ops);
            self.swing_release_pending = true;
            self.swap_attack_pending = false;
            if self.stun_slam_active {
                self.stun_slam_second_phase = true;
            }
        }
        if self.stun_slam_active
            && self.swap_active
            && self.stun_slam_second_phase
            && self.tick_counter >= 1
        {
            if let Some(mace) = options.mace_slot(view) {
                select(view, ops, mace);
                ops.push(Op::Release);
                request(view, ops);
                self.swing_release_pending = true;
            }
            self.stun_slam_second_phase = false;
            self.stun_slam_active = false;
            return;
        }
        if self.swap_active {
            let restore_delay = if self.stun_slam_active || self.stun_slam_second_phase {
                2
            } else {
                1
            };
            let counter = self.tick_counter;
            self.tick_counter += 1;
            if counter > restore_delay {
                if let Some(slot) = self.original_slot.take() {
                    select(view, ops, slot);
                }
                self.swap_active = false;
                self.tick_counter = 0;
                self.stun_slam_active = false;
                self.stun_slam_second_phase = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::settings::Settings;
    use super::super::super::super::inventory::item_limit::HeldItem;
    use super::super::weapon_swap::{Crosshair, Shield, Target};
    use super::*;

    fn slot(id: &str, density: i32, breach: i32) -> Slot {
        Slot {
            item: HeldItem {
                id: (!id.is_empty()).then(|| id.into()),
                ..HeldItem::default()
            },
            density,
            breach,
        }
    }
    fn view(items: &[&str]) -> View {
        let mut hotbar: Vec<Slot> = items.iter().map(|id| slot(id, 0, 0)).collect();
        hotbar.resize(9, Slot::default());
        for (i, s) in hotbar.iter_mut().enumerate() {
            s.item.slot = i;
        }
        View {
            hotbar,
            crosshair: Some(Crosshair::default()),
            will_smash: true,
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
    fn shielded(elapsed: i32) -> Option<Target> {
        Some(Target {
            shield: Some(Shield {
                using: true,
                elapsed,
            }),
        })
    }
    fn run(h: &mut HitSwap, o: &Options, v: &mut View) -> (Vec<Op>, u32) {
        let mut ops = Vec::new();
        let mut requests = 0;
        h.tick(o, v, &mut ops, &mut |_, ops| {
            requests += 1;
            ops.push(Op::Press);
            true
        });
        (ops, requests)
    }

    #[test]
    fn defaults_and_visibility_match_the_reference() {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        let o = Options::read(&s);
        assert!(o.maces && o.smash_only && o.density_maces && o.axes);
        assert!(!o.breach_maces && !o.stun_slam && !o.swords && !o.limit_items);
        assert_eq!(o.allowed_items, vec!["swords".to_owned()]);
        s.set(&format!("{ID}.maces"), json!(false)).unwrap();
        let o = Options::read(&s);
        assert!(!o.smash_only && !o.density_maces);
        assert!(!s.visible(&format!("{ID}.stun-slam")));
    }

    #[test]
    fn smashing_press_swaps_to_the_mace_attacks_next_tick_and_restores() {
        let o = options();
        let mut v = view(&["diamond_sword", "", "mace"]);
        let mut h = HitSwap::default();
        let mut ops = Vec::new();
        assert!(h.press(&o, &mut v, &mut ops));
        assert_eq!(ops, vec![Op::Select(2)]);
        assert!(h.active());
        // Tick 1: release and the synthetic attack from the mace.
        assert_eq!(run(&mut h, &o, &mut v), (vec![Op::Release, Op::Press], 1));
        // Tick 2: the swing is released; counter 1 is not past the delay.
        assert_eq!(run(&mut h, &o, &mut v), (vec![Op::Release], 0));
        // Tick 3: counter 2 > 1 restores the sword.
        assert_eq!(run(&mut h, &o, &mut v), (vec![Op::Select(0)], 0));
        assert!(!h.active());
        assert_eq!(v.selected, 0);
    }

    #[test]
    fn mace_needs_a_smash_and_the_chosen_enchantment_when_present() {
        let o = options();
        let mut v = view(&["diamond_sword", "mace"]);
        v.will_smash = false;
        let mut h = HitSwap::default();
        assert!(!h.press(&o, &mut v, &mut Vec::new()));
        // Density on: a Density mace is preferred over a plain one.
        let mut v = view(&["diamond_sword", "mace", "mace"]);
        v.hotbar[2].density = 3;
        assert_eq!(o.weapon_slot(&v), Ok(Some(2)));
        // With no Density mace in the hotbar, any mace qualifies.
        let v = view(&["diamond_sword", "mace"]);
        assert_eq!(o.weapon_slot(&v), Ok(Some(1)));
        // Swords only when enabled.
        let v = view(&["iron_axe", "diamond_sword"]);
        assert_eq!(o.weapon_slot(&view(&["stick", "diamond_sword"])), Ok(None));
        let swords = Options {
            swords: true,
            ..o.clone()
        };
        assert_eq!(swords.weapon_slot(&v), Ok(Some(1)));
    }

    #[test]
    fn axe_waits_for_a_raised_shield_past_warm_up() {
        let o = options();
        let mut v = view(&["diamond_sword", "iron_axe", "mace"]);
        v.nearby_player = shielded(3);
        // The axe slot comes first and the shield is still warming up: -2.
        assert_eq!(o.weapon_slot(&v), Err(()));
        assert!(!HitSwap::default().press(&o, &mut v, &mut Vec::new()));
        v.nearby_player = shielded(6);
        assert_eq!(o.weapon_slot(&v), Ok(Some(1)));
        // A lowered shield skips the axe for the mace.
        v.nearby_player = Some(Target {
            shield: Some(Shield::default()),
        });
        assert_eq!(o.weapon_slot(&v), Ok(Some(2)));
    }

    #[test]
    fn stun_slam_hits_with_the_axe_then_slams_with_the_mace() {
        let o = Options {
            stun_slam: true,
            ..options()
        };
        let mut v = view(&["diamond_sword", "iron_axe", "mace"]);
        v.nearby_player = shielded(10);
        let mut h = HitSwap::default();
        let mut ops = Vec::new();
        assert!(h.press(&o, &mut v, &mut ops));
        assert_eq!(ops, vec![Op::Select(1)]);
        // Axe attack; the second phase starts.
        assert_eq!(run(&mut h, &o, &mut v), (vec![Op::Release, Op::Press], 1));
        // Release, then (counter 1) the mace slam.
        assert_eq!(
            run(&mut h, &o, &mut v),
            (vec![Op::Release, Op::Select(2), Op::Release, Op::Press], 1)
        );
        assert_eq!(run(&mut h, &o, &mut v), (vec![Op::Release], 0));
        assert_eq!(run(&mut h, &o, &mut v), (vec![Op::Select(0)], 0));
        assert!(!h.active());
    }

    #[test]
    fn stun_slam_holding_an_axe_lets_the_hit_through_then_slams() {
        let o = Options {
            stun_slam: true,
            ..options()
        };
        let mut v = view(&["iron_axe", "mace"]);
        v.nearby_player = shielded(10);
        let mut h = HitSwap::default();
        assert!(!h.press(&o, &mut v, &mut Vec::new()));
        assert!(h.active());
        assert_eq!(run(&mut h, &o, &mut v), (vec![], 0));
        assert_eq!(
            run(&mut h, &o, &mut v),
            (vec![Op::Select(1), Op::Release, Op::Press], 1)
        );
        assert_eq!(run(&mut h, &o, &mut v), (vec![Op::Release], 0));
        assert_eq!(run(&mut h, &o, &mut v), (vec![Op::Select(0)], 0));
    }

    #[test]
    fn synthetic_sources_screens_item_use_and_limits() {
        let o = options();
        let mut h = HitSwap::default();
        let mut v = view(&["diamond_sword", "mace"]);
        for source in [
            Source::HitSwap,
            Source::ShieldBreaker,
            Source::AutoMace(true),
        ] {
            assert!(!h.synthetic(&o, source, &mut v, &mut Vec::new()));
        }
        v.using_item = true;
        assert!(!h.synthetic(&o, Source::TriggerBot, &mut v, &mut Vec::new()));
        v.using_item = false;
        v.crosshair = None;
        assert!(!h.synthetic(&o, Source::TriggerBot, &mut v, &mut Vec::new()));
        v.crosshair = Some(Crosshair::default());
        let limited = Options {
            limit_items: true,
            allowed_items: vec!["axes".into()],
            ..o.clone()
        };
        assert!(!h.synthetic(&limited, Source::TriggerBot, &mut v, &mut Vec::new()));
        assert!(h.synthetic(&o, Source::AutoMace(false), &mut v, &mut Vec::new()));
        // Disabling mid-swing releases the swing and keeps the swapped slot.
        run(&mut h, &o, &mut v);
        let mut ops = Vec::new();
        h.reset(&mut ops);
        assert_eq!(ops, vec![Op::Release]);
        assert_eq!(v.selected, 1);
    }

    #[test]
    fn alternate_axe_and_weapon_swap_queries() {
        let o = options();
        let v = view(&["diamond_sword", "iron_axe", "mace"]);
        assert!(o.has_alternate_axe(&v));
        assert!(!o.has_alternate_axe(&view(&["iron_axe"])));
        // The first weapon is the axe (no shield): skipped, then the mace.
        assert!(o.has_valid_weapon_swap(&v));
        let swords = Options {
            swords: true,
            maces: false,
            ..o
        };
        assert!(!swords.has_valid_weapon_swap(&view(&["stick", "diamond_sword"])));
    }
}
