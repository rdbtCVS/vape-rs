// Local, bounded macro scheduling.  This deliberately emits game effects
// instead of performing OS automation, packet IO, or synthetic input.
use serde_json::{Value, json};
use std::collections::BTreeSet;

const MAX_MACROS: usize = 256;
const MAX_NAME_CHARS: usize = 128;
const MAX_BIND_KEYS: usize = 3;
const MAX_KEY_CHARS: usize = 64;
const FISHING_ROD: &str = "fishing rod";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacroKind {
    Command { text: String },
    Item { name: String, fishing_rod: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delay {
    pub min_ms: u16,
    pub max_ms: u16,
}
impl Delay {
    pub fn bounded(min_ms: u16, max_ms: u16) -> Self {
        let min_ms = min_ms.clamp(1, 1000);
        Self {
            min_ms,
            max_ms: max_ms.clamp(min_ms, 1000),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Macro {
    pub name: String,
    pub binds: Vec<String>,
    pub delay: Delay,
    pub double_click: bool,
    pub double_click_delay: Delay,
}
impl Macro {
    /// Reference `Macro.create`: a case-sensitive "fishing rod" prefix first,
    /// then a leading slash, otherwise an item name.
    pub fn kind(&self) -> MacroKind {
        if self.name.starts_with(FISHING_ROD) {
            MacroKind::Item {
                fishing_rod: true,
                name: self.name.clone(),
            }
        } else if self.name.starts_with('/') {
            MacroKind::Command {
                text: self.name.clone(),
            }
        } else {
            MacroKind::Item {
                fishing_rod: false,
                name: self.name.clone(),
            }
        }
    }
    /// The name a newly created macro keeps: `FishingRodMacro` always names
    /// itself "fishing rod", whatever followed the prefix.
    pub fn created_name(name: &str) -> String {
        if name.starts_with(FISHING_ROD) {
            FISHING_ROD.into()
        } else {
            name.into()
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        valid_text(&self.name, MAX_NAME_CHARS, "macro name")?;
        if self.binds.is_empty() || self.binds.len() > MAX_BIND_KEYS {
            return Err("macro binds require 1–3 physical keys".into());
        }
        let mut seen = BTreeSet::new();
        for key in &self.binds {
            valid_text(key, MAX_KEY_CHARS, "macro binding")?;
            if !seen.insert(key) {
                return Err("macro binding repeats a key".into());
            }
        }
        if self.delay.min_ms == 0
            || self.delay.min_ms > self.delay.max_ms
            || self.delay.max_ms > 1000
            || self.double_click_delay.min_ms == 0
            || self.double_click_delay.min_ms > self.double_click_delay.max_ms
            || self.double_click_delay.max_ms > 1000
        {
            return Err("macro delay must be within 1–1000 ms".into());
        }
        Ok(())
    }
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name, "binds": self.binds,
            "delay": {"min_ms": self.delay.min_ms, "max_ms": self.delay.max_ms},
            "double_click": self.double_click,
            "double_click_delay": {"min_ms": self.double_click_delay.min_ms, "max_ms": self.double_click_delay.max_ms},
        })
    }
    pub fn from_json(value: &Value) -> Result<Self, String> {
        let object = value.as_object().ok_or("macro must be an object")?;
        let string = |key: &str| {
            object
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("macro {key} must be a string"))
        };
        let delay = |key: &str| -> Result<Delay, String> {
            let value = object
                .get(key)
                .and_then(Value::as_object)
                .ok_or_else(|| format!("macro {key} must be an object"))?;
            let min = value
                .get("min_ms")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("macro {key}.min_ms must be an integer"))?;
            let max = value
                .get("max_ms")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("macro {key}.max_ms must be an integer"))?;
            if min > u16::MAX as u64 || max > u16::MAX as u64 {
                return Err(format!("macro {key} is out of range"));
            }
            Ok(Delay {
                min_ms: min as u16,
                max_ms: max as u16,
            })
        };
        let binds = object
            .get("binds")
            .and_then(Value::as_array)
            .ok_or("macro binds must be an array")?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or("macro bind must be a string")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = Self {
            name: string("name")?,
            binds,
            delay: delay("delay")?,
            double_click: object
                .get("double_click")
                .and_then(Value::as_bool)
                .ok_or("macro double_click must be boolean")?,
            double_click_delay: delay("double_click_delay")?,
        };
        result.validate()?;
        Ok(result)
    }
}

fn valid_text(value: &str, limit: usize, name: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().count() > limit || value.chars().any(char::is_control) {
        Err(format!(
            "{name} must contain 1–{limit} non-control characters"
        ))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputAction {
    Press,
    Repeat,
    Release,
    FocusLost,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputOrigin {
    Physical,
    Synthetic,
}

#[derive(Clone, Debug, Default)]
pub struct MacroContext {
    /// Registry ids (`minecraft:snowball`) for slots 0–8; empty slots use None.
    pub hotbar: Vec<Option<String>>,
    /// Each slot's display names: the hover name (custom name, else the
    /// item-name component, else the localized item name) and the localized
    /// item name, formatting codes removed.
    pub hotbar_names: Vec<Vec<String>>,
    pub selected_slot: Option<u8>,
    /// Fishing-rod double-clicks may finish their wait early when the host
    /// observes a hook/entity state equivalent to the reference bridge.
    pub fishing_rod_ready: bool,
    /// The physical use button is held (`ClientSettings.isUseItemButtonDown`).
    pub use_held: bool,
    pub accepts_input: bool,
}
impl MacroContext {
    /// Reference `ItemMacro.findHotbarSlot`: the first slot whose registry id,
    /// id without namespace or display name equals the macro name ignoring
    /// case. (The reference's numeric item id has no 26.x equivalent.)
    fn slot_for(&self, name: &str) -> Option<u8> {
        self.hotbar
            .iter()
            .take(9)
            .enumerate()
            .position(|(slot, item)| {
                item.as_ref().is_some_and(|id| {
                    let short = id.split_once(':').map_or(id.as_str(), |(_, path)| path);
                    equals_ignore_case(id, name)
                        || equals_ignore_case(short, name)
                        || self
                            .hotbar_names
                            .get(slot)
                            .is_some_and(|names| names.iter().any(|n| equals_ignore_case(n, name)))
                })
            })
            .map(|slot| slot as u8)
    }
}

/// Java `String.equalsIgnoreCase`: per-character upper- then lower-case match.
fn equals_ignore_case(a: &str, b: &str) -> bool {
    a.chars().count() == b.chars().count()
        && a.chars().zip(b.chars()).all(|(x, y)| {
            x == y || x.to_uppercase().eq(y.to_uppercase()) || x.to_lowercase().eq(y.to_lowercase())
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacroEffect {
    /// A normal in-game chat message for the host chat path only.
    Chat { text: String },
    /// An in-game command, without its leading slash, for the host command path.
    Command { command: String },
    /// Choose a hotbar slot through the host's ordinary selection path.
    SelectHotbar { slot: u8 },
    /// Cleanup selection, permitted in menus only while the same world/claim remains valid.
    RestoreHotbar { slot: u8 },
    /// Set the host's normal use-item key state. The host must never feed this
    /// emitted state back to `input` as a physical macro trigger.
    UseItem { down: bool },
}

#[derive(Clone, Debug)]
enum Active {
    Command {
        text: String,
    },
    /// Reference `ItemMacroAction`: phases 0 use down, 1 use up, 2 double
    /// click, 3 delay, 4 restore.
    Item {
        name: String,
        fishing_rod: bool,
        delay: Delay,
        double_click: bool,
        double_click_delay: Delay,
        phase: u8,
        /// The selection to restore, inherited from a replaced item macro.
        original_slot: Option<u8>,
        selected: bool,
        timer_ms: u64,
        use_down: bool,
        second_started: bool,
    },
}

#[derive(Debug)]
pub struct MacroEngine {
    macros: Vec<Macro>,
    pressed: BTreeSet<String>,
    active: Option<Active>,
    random: u64,
}
impl Default for MacroEngine {
    fn default() -> Self {
        Self {
            macros: Vec::new(),
            pressed: BTreeSet::new(),
            active: None,
            random: 0x51ed_5eed,
        }
    }
}
impl MacroEngine {
    pub fn macros(&self) -> &[Macro] {
        &self.macros
    }
    pub fn uses_hotbar(&self) -> bool {
        matches!(self.active, Some(Active::Item { .. }))
    }
    pub fn active(&self) -> bool {
        self.active.is_some()
    }
    pub fn replace(&mut self, macros: Vec<Macro>) -> Result<Vec<MacroEffect>, String> {
        if macros.len() > MAX_MACROS {
            return Err(format!("at most {MAX_MACROS} local macros"));
        }
        for macro_ in &macros {
            macro_.validate()?;
        }
        let effects = self.cancel();
        self.macros = macros;
        Ok(effects)
    }
    pub fn to_json(&self) -> Value {
        Value::Array(self.macros.iter().map(Macro::to_json).collect())
    }
    pub fn load_json(&mut self, value: &Value) -> Result<Vec<MacroEffect>, String> {
        let macros = value
            .as_array()
            .ok_or("macros must be an array")?
            .iter()
            .map(Macro::from_json)
            .collect::<Result<Vec<_>, _>>()?;
        self.replace(macros)
    }
    /// Only physical press edges can start a macro. Synthetic use/select
    /// effects are intentionally ignored to prevent recursive triggering.
    /// Like `MacroEventListener`, every macro bound to the input is tried in
    /// order until one can start (an item macro needs its item in the hotbar).
    pub fn input(
        &mut self,
        key: &str,
        action: InputAction,
        origin: InputOrigin,
        now_ms: u64,
        context: &MacroContext,
    ) -> Vec<MacroEffect> {
        if action == InputAction::FocusLost {
            self.pressed.clear();
            return self.cancel();
        }
        if origin != InputOrigin::Physical {
            return Vec::new();
        }
        match action {
            InputAction::Release => {
                self.pressed.remove(key);
                Vec::new()
            }
            InputAction::Repeat => Vec::new(),
            InputAction::Press => {
                if !self.pressed.insert(key.into()) || !context.accepts_input {
                    return Vec::new();
                }
                let pressed = &self.pressed;
                let candidates: Vec<Macro> = self
                    .macros
                    .iter()
                    .filter(|macro_| {
                        macro_.binds.iter().any(|bound| bound == key)
                            && macro_.binds.iter().all(|k| pressed.contains(k))
                    })
                    .cloned()
                    .collect();
                for macro_ in candidates {
                    if let Some(effects) = self.start(macro_, now_ms, context) {
                        return effects;
                    }
                }
                Vec::new()
            }
            InputAction::FocusLost => unreachable!(),
        }
    }
    /// `startMacro`: None when the macro cannot start, so the next one is
    /// tried. A replaced item macro passes on the selection to restore.
    fn start(
        &mut self,
        macro_: Macro,
        now_ms: u64,
        context: &MacroContext,
    ) -> Option<Vec<MacroEffect>> {
        let kind = macro_.kind();
        if let MacroKind::Item { name, .. } = &kind
            && context.slot_for(name).is_none()
        {
            return None;
        }
        let mut effects = Vec::new();
        let inherited = match self.active.take() {
            Some(Active::Item {
                original_slot,
                use_down,
                ..
            }) => {
                if use_down {
                    effects.push(MacroEffect::UseItem { down: false });
                }
                original_slot
            }
            Some(Active::Command { .. }) | None => None,
        };
        self.active = Some(match kind {
            MacroKind::Command { text } => Active::Command { text },
            MacroKind::Item { name, fishing_rod } => Active::Item {
                name,
                fishing_rod,
                delay: macro_.delay,
                double_click: macro_.double_click,
                double_click_delay: macro_.double_click_delay,
                phase: 0,
                original_slot: inherited,
                selected: false,
                timer_ms: now_ms,
                use_down: false,
                second_started: false,
            },
        });
        Some(effects)
    }
    /// One client tick (the reference's `EventPreTick`): at most one phase.
    pub fn tick(&mut self, now_ms: u64, context: &MacroContext) -> Vec<MacroEffect> {
        let Some(active) = self.active.take() else {
            return Vec::new();
        };
        let mut effects = Vec::new();
        match active {
            Active::Command { text } => effects.push(MacroEffect::Command {
                command: text.strip_prefix('/').unwrap_or(&text).into(),
            }),
            mut item @ Active::Item { .. } => {
                let Active::Item {
                    ref name,
                    fishing_rod,
                    ref delay,
                    double_click,
                    ref double_click_delay,
                    ref mut phase,
                    ref mut original_slot,
                    ref mut selected,
                    ref mut timer_ms,
                    ref mut use_down,
                    ref mut second_started,
                } = item
                else {
                    unreachable!()
                };
                if !*selected {
                    // The reference skips this for an inherited selection and
                    // so would use the replaced macro's item; the target is
                    // selected either way here.
                    let Some(target) = context.slot_for(name) else {
                        return self.finish_item(*original_slot, *use_down, effects);
                    };
                    if original_slot.is_none() {
                        *original_slot = context.selected_slot;
                    }
                    *selected = true;
                    effects.push(MacroEffect::SelectHotbar { slot: target });
                    *timer_ms = now_ms;
                    // Already using: the held button does the first use.
                    if context.use_held {
                        *phase = 2;
                    }
                }
                let elapsed = now_ms.saturating_sub(*timer_ms);
                match *phase {
                    0 => {
                        effects.push(MacroEffect::UseItem { down: true });
                        *use_down = true;
                        *phase = 1;
                    }
                    1 => {
                        effects.push(MacroEffect::UseItem { down: false });
                        *use_down = false;
                        *timer_ms = now_ms;
                        *phase = 2;
                    }
                    2 if double_click && !*second_started => {
                        let wait = self.random_delay(double_click_delay);
                        if elapsed >= wait || (fishing_rod && context.fishing_rod_ready) {
                            *second_started = true;
                            *phase = 0;
                        }
                    }
                    2 => *phase = 3,
                    3 => {
                        let wait = self.random_delay(delay);
                        if elapsed as i64 >= wait as i64 - 2 {
                            *phase = 4;
                        }
                    }
                    _ => {
                        let original = *original_slot;
                        return self.finish_item(original, *use_down, effects);
                    }
                }
                self.active = Some(item);
            }
        }
        effects
    }
    fn finish_item(
        &mut self,
        original_slot: Option<u8>,
        use_down: bool,
        mut effects: Vec<MacroEffect>,
    ) -> Vec<MacroEffect> {
        if use_down {
            effects.push(MacroEffect::UseItem { down: false });
        }
        if let Some(slot) = original_slot {
            effects.push(MacroEffect::RestoreHotbar { slot });
        }
        effects
    }
    /// `RandomUtil.Y(min, max)`: min when the range is empty, otherwise
    /// uniform in min+1..=max.
    fn random_delay(&mut self, delay: &Delay) -> u64 {
        let span = u64::from(delay.max_ms.saturating_sub(delay.min_ms));
        if span == 0 {
            return delay.min_ms.into();
        }
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        u64::from(delay.min_ms) + 1 + self.random % span
    }
    /// Call on disable, GUI/context loss, profile replacement, world/session
    /// transition and panic. It releases/restores any engine-owned action;
    /// the physical key state is kept until releases or focus loss.
    pub fn cancel(&mut self) -> Vec<MacroEffect> {
        match self.active.take() {
            Some(Active::Item {
                original_slot,
                use_down,
                ..
            }) => self.finish_item(original_slot, use_down, Vec::new()),
            Some(Active::Command { .. }) | None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, key: &str, delay: (u16, u16)) -> Macro {
        Macro {
            name: name.into(),
            binds: vec![key.into()],
            delay: Delay::bounded(delay.0, delay.1),
            double_click: false,
            double_click_delay: Delay::bounded(100, 100),
        }
    }
    fn context() -> MacroContext {
        MacroContext {
            hotbar: vec![
                Some("minecraft:diamond_sword".into()),
                Some("minecraft:ender_pearl".into()),
                Some("minecraft:snowball".into()),
                Some("minecraft:fishing_rod".into()),
            ],
            hotbar_names: vec![
                vec!["Diamond Sword".into(), "Diamond Sword".into()],
                vec!["Ender Pearl".into(), "Ender Pearl".into()],
                vec!["Bridge Egg".into(), "Snowball".into()],
                vec!["Fishing Rod".into(), "Fishing Rod".into()],
            ],
            selected_slot: Some(0),
            accepts_input: true,
            ..MacroContext::default()
        }
    }
    fn press(engine: &mut MacroEngine, key: &str, now: u64, context: &MacroContext) {
        let effects = engine.input(key, InputAction::Press, InputOrigin::Physical, now, context);
        assert!(effects.is_empty(), "{effects:?}");
        engine.input(
            key,
            InputAction::Release,
            InputOrigin::Physical,
            now,
            context,
        );
    }

    #[test]
    fn hotbar_lookup_follows_find_hotbar_slot() {
        let context = context();
        for (name, slot) in [
            ("minecraft:snowball", Some(2)),
            ("SNOWBALL", Some(2)),
            ("bridge egg", Some(2)),
            ("ender pearl", Some(1)),
            ("ender_pearl", Some(1)),
            ("Fishing Rod", Some(3)),
            ("snow ball", None),
            ("stone", None),
        ] {
            assert_eq!(context.slot_for(name), slot, "{name}");
        }
        assert!(equals_ignore_case("ÉCLAIR", "éclair"));
    }

    #[test]
    fn kind_and_created_name_follow_macro_create() {
        let named = |name: &str| item(name, "KeyG", (100, 100)).kind();
        assert!(matches!(
            named("fishing rod 2"),
            MacroKind::Item {
                fishing_rod: true,
                ..
            }
        ));
        assert!(matches!(
            named("Fishing rod"),
            MacroKind::Item {
                fishing_rod: false,
                ..
            }
        ));
        assert!(matches!(named("/spawn"), MacroKind::Command { .. }));
        assert_eq!(Macro::created_name("fishing rod quick"), "fishing rod");
        assert_eq!(Macro::created_name("/home"), "/home");
    }

    #[test]
    fn item_macro_runs_the_reference_phases() {
        let mut engine = MacroEngine::default();
        engine
            .replace(vec![item("snowball", "KeyG", (100, 100))])
            .unwrap();
        let mut context = context();
        press(&mut engine, "KeyG", 0, &context);
        assert_eq!(
            engine.tick(50, &context),
            [
                MacroEffect::SelectHotbar { slot: 2 },
                MacroEffect::UseItem { down: true }
            ]
        );
        context.selected_slot = Some(2);
        assert_eq!(
            engine.tick(100, &context),
            [MacroEffect::UseItem { down: false }]
        );
        // Phase 2 without a double click moves on; phase 3 waits Y(delay) - 2.
        assert!(engine.tick(150, &context).is_empty());
        assert!(engine.tick(197, &context).is_empty());
        assert!(engine.tick(198, &context).is_empty());
        assert_eq!(
            engine.tick(250, &context),
            [MacroEffect::RestoreHotbar { slot: 0 }]
        );
        assert!(!engine.active());
    }

    #[test]
    fn held_use_button_skips_the_click() {
        let mut engine = MacroEngine::default();
        engine
            .replace(vec![item("snowball", "KeyG", (1, 1))])
            .unwrap();
        let mut context = context();
        context.use_held = true;
        press(&mut engine, "KeyG", 0, &context);
        assert_eq!(
            engine.tick(50, &context),
            [MacroEffect::SelectHotbar { slot: 2 }]
        );
        assert!(engine.tick(100, &context).is_empty());
        assert_eq!(
            engine.tick(150, &context),
            [MacroEffect::RestoreHotbar { slot: 0 }]
        );
    }

    #[test]
    fn double_click_waits_its_delay_or_a_ready_rod() {
        for (name, ready, second_at) in [("snowball", false, 200), ("fishing rod", true, 150)] {
            let mut engine = MacroEngine::default();
            let mut macro_ = item(name, "KeyG", (1, 1));
            macro_.double_click = true;
            engine.replace(vec![macro_]).unwrap();
            let mut context = context();
            press(&mut engine, "KeyG", 0, &context);
            assert!(
                engine
                    .tick(50, &context)
                    .contains(&MacroEffect::UseItem { down: true })
            );
            assert_eq!(
                engine.tick(100, &context),
                [MacroEffect::UseItem { down: false }]
            );
            context.fishing_rod_ready = ready;
            let mut now = 150;
            while now < second_at {
                assert!(engine.tick(now, &context).is_empty(), "{name} {now}");
                now += 50;
            }
            assert!(engine.tick(now, &context).is_empty());
            assert_eq!(
                engine.tick(now + 50, &context),
                [MacroEffect::UseItem { down: true }]
            );
        }
    }

    #[test]
    fn bound_macros_are_tried_in_order() {
        let mut engine = MacroEngine::default();
        engine
            .replace(vec![
                item("golden apple", "KeyG", (100, 100)),
                item("ender pearl", "KeyG", (100, 100)),
                item("snowball", "KeyG", (100, 100)),
            ])
            .unwrap();
        let context = context();
        press(&mut engine, "KeyG", 0, &context);
        assert_eq!(
            engine.tick(50, &context)[0],
            MacroEffect::SelectHotbar { slot: 1 }
        );
    }

    #[test]
    fn a_replacing_macro_keeps_the_first_original_slot() {
        let mut engine = MacroEngine::default();
        engine
            .replace(vec![
                item("snowball", "KeyG", (500, 500)),
                item("ender pearl", "KeyH", (1, 1)),
            ])
            .unwrap();
        let mut context = context();
        press(&mut engine, "KeyG", 0, &context);
        engine.tick(50, &context);
        context.selected_slot = Some(2);
        let effects = engine.input(
            "KeyH",
            InputAction::Press,
            InputOrigin::Physical,
            60,
            &context,
        );
        assert_eq!(effects, [MacroEffect::UseItem { down: false }]);
        assert_eq!(
            engine.tick(100, &context),
            [
                MacroEffect::SelectHotbar { slot: 1 },
                MacroEffect::UseItem { down: true }
            ]
        );
        context.selected_slot = Some(1);
        for now in [150, 200, 250] {
            engine.tick(now, &context);
        }
        assert_eq!(
            engine.tick(300, &context),
            [MacroEffect::RestoreHotbar { slot: 0 }]
        );
    }

    #[test]
    fn commands_send_once_without_one_slash() {
        let mut engine = MacroEngine::default();
        engine
            .replace(vec![item("//wand", "KeyG", (1, 1))])
            .unwrap();
        let context = context();
        press(&mut engine, "KeyG", 0, &context);
        assert_eq!(
            engine.tick(50, &context),
            [MacroEffect::Command {
                command: "/wand".into()
            }]
        );
        assert!(!engine.active());
    }

    #[test]
    fn chords_need_every_key_and_screens_block_new_presses() {
        let mut engine = MacroEngine::default();
        let mut macro_ = item("snowball", "ControlLeft", (1, 1));
        macro_.binds.push("KeyG".into());
        engine.replace(vec![macro_]).unwrap();
        let mut context = context();
        let physical = InputOrigin::Physical;
        context.accepts_input = false;
        engine.input("ControlLeft", InputAction::Press, physical, 0, &context);
        context.accepts_input = true;
        engine.input("KeyG", InputAction::Press, physical, 0, &context);
        assert!(engine.active());
        engine.cancel();
        engine.input("KeyG", InputAction::Release, physical, 0, &context);
        engine.input("ControlLeft", InputAction::Release, physical, 0, &context);
        engine.input("KeyG", InputAction::Press, physical, 0, &context);
        assert!(!engine.active());
    }
}
