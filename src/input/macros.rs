// Local, bounded macro scheduling.  This deliberately emits game effects
// instead of performing OS automation, packet IO, or synthetic input.
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};

const MAX_MACROS: usize = 256;
const MAX_NAME_CHARS: usize = 128;
const MAX_BIND_KEYS: usize = 3;
const MAX_KEY_CHARS: usize = 64;

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
    pub fn kind(&self) -> MacroKind {
        if self.name.starts_with('/') {
            MacroKind::Command {
                text: self.name.clone(),
            }
        } else {
            MacroKind::Item {
                fishing_rod: self.name.to_ascii_lowercase().starts_with("fishing rod"),
                name: self.name.clone(),
            }
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
    /// Host-normalized registry/display names for slots 0–8; empty slots use None.
    pub hotbar: Vec<Option<String>>,
    pub selected_slot: Option<u8>,
    /// Fishing-rod double-clicks may finish their wait early when the host
    /// observes a hook/entity state equivalent to the reference bridge.
    pub fishing_rod_ready: bool,
    pub accepts_input: bool,
}
impl MacroContext {
    fn slot_for(&self, name: &str) -> Option<u8> {
        self.hotbar
            .iter()
            .take(9)
            .position(|item| {
                item.as_ref().is_some_and(|item| {
                    let normalized = |text: &str| {
                        text.rsplit(':')
                            .next()
                            .unwrap_or(text)
                            .replace('_', " ")
                            .to_lowercase()
                    };
                    normalized(item) == normalized(name)
                })
            })
            .map(|slot| slot as u8)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacroEffect {
    /// A normal in-game chat message for the host chat path only.
    Chat { text: String },
    /// An in-game command, without its leading slash, for the host command path.
    Command { command: String },
    /// Choose a hotbar slot through the host's ordinary selection path.
    SelectHotbar { slot: u8 },
    /// Set the host's normal use-item key state. The host must never feed this
    /// emitted state back to `input` as a physical macro trigger.
    UseItem { down: bool },
}

#[derive(Clone, Debug)]
enum Active {
    Command {
        text: String,
    },
    Item {
        name: String,
        fishing_rod: bool,
        delay: Delay,
        double_click: bool,
        double_click_delay: Delay,
        phase: u8,
        original_slot: Option<u8>,
        target_slot: u8,
        due_ms: u64,
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
        let mut names = HashSet::new();
        for macro_ in &macros {
            macro_.validate()?;
            if !names.insert(macro_.name.to_ascii_lowercase()) {
                return Err("macro names must be unique ignoring case".into());
            }
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
    pub fn input(
        &mut self,
        key: &str,
        action: InputAction,
        origin: InputOrigin,
        now_ms: u64,
        context: &MacroContext,
    ) -> Vec<MacroEffect> {
        if action == InputAction::FocusLost {
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
                if !context.accepts_input || !self.pressed.insert(key.into()) {
                    return Vec::new();
                }
                let pressed = &self.pressed;
                let candidate = self
                    .macros
                    .iter()
                    .find(|macro_| {
                        macro_.binds.iter().any(|bound| bound == key)
                            && macro_.binds.iter().all(|k| pressed.contains(k))
                    })
                    .cloned();
                candidate.map_or_else(Vec::new, |macro_| self.start(macro_, now_ms, context))
            }
            InputAction::FocusLost => unreachable!(),
        }
    }
    fn start(&mut self, macro_: Macro, now_ms: u64, context: &MacroContext) -> Vec<MacroEffect> {
        let inherited = match self.active.take() {
            Some(Active::Item {
                original_slot,
                use_down,
                ..
            }) => {
                let mut effects = Vec::new();
                if use_down {
                    effects.push(MacroEffect::UseItem { down: false });
                }
                (original_slot, effects)
            }
            Some(_) | None => (None, Vec::new()),
        };
        let (original_slot, effects) = inherited;
        match macro_.kind() {
            MacroKind::Command { text } => self.active = Some(Active::Command { text }),
            MacroKind::Item { name, fishing_rod } => {
                let Some(target_slot) = context.slot_for(&name) else {
                    return effects;
                };
                self.active = Some(Active::Item {
                    name,
                    fishing_rod,
                    delay: macro_.delay,
                    double_click: macro_.double_click,
                    double_click_delay: macro_.double_click_delay,
                    phase: 0,
                    original_slot: original_slot.or(context.selected_slot),
                    target_slot,
                    due_ms: now_ms,
                    use_down: false,
                    second_started: false,
                });
            }
        }
        effects
    }
    pub fn tick(&mut self, now_ms: u64, context: &MacroContext) -> Vec<MacroEffect> {
        let Some(active) = self.active.take() else {
            return Vec::new();
        };
        let mut effects = Vec::new();
        match active {
            Active::Command { text } => effects.push(MacroEffect::Command {
                command: text.trim_start_matches('/').into(),
            }),
            mut item @ Active::Item { .. } => {
                let Active::Item {
                    ref name,
                    fishing_rod,
                    ref delay,
                    double_click,
                    ref double_click_delay,
                    ref mut phase,
                    original_slot,
                    target_slot,
                    ref mut due_ms,
                    ref mut use_down,
                    ref mut second_started,
                } = item
                else {
                    unreachable!()
                };
                if context.slot_for(&name) != Some(target_slot) {
                    return self.finish_item(original_slot, *use_down, effects);
                }
                match *phase {
                    0 => {
                        effects.push(MacroEffect::SelectHotbar { slot: target_slot });
                        effects.push(MacroEffect::UseItem { down: true });
                        *use_down = true;
                        *phase = 1;
                    }
                    1 => {
                        effects.push(MacroEffect::UseItem { down: false });
                        *use_down = false;
                        *due_ms = now_ms + u64::from(self.random_delay(double_click_delay.clone()));
                        *phase = 2;
                    }
                    2 => {
                        if !*second_started
                            && double_click
                            && (now_ms >= *due_ms || (fishing_rod && context.fishing_rod_ready))
                        {
                            *second_started = true;
                            *phase = 0;
                        } else if *second_started || !double_click {
                            *due_ms = now_ms + u64::from(self.random_delay(delay.clone()));
                            *phase = 3;
                        }
                    }
                    3 if now_ms >= *due_ms => {
                        return self.finish_item(original_slot, *use_down, effects);
                    }
                    _ => {}
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
            effects.push(MacroEffect::SelectHotbar { slot });
        }
        effects
    }
    fn random_delay(&mut self, delay: Delay) -> u16 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        delay.min_ms + (self.random as u16 % (delay.max_ms - delay.min_ms + 1))
    }
    /// Call on disable, GUI/context loss, profile replacement, world/session
    /// transition and panic. It clears held keys and releases/restores any
    /// engine-owned action.
    pub fn cancel(&mut self) -> Vec<MacroEffect> {
        self.pressed.clear();
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
