// AntiDebuff (reference `AntiDebuff`): negative visual effects taken off the
// local player's client effect list every tick. Remove Nausea drops nausea;
// Remove Blindness drops blindness (the reference hides it from rendering
// only unless Remove Effects is on; here it leaves the client list, which also
// lifts the client's blind sprint block); Remove Slowness, with Remove
// Effects, drops slowness and its movement-speed modifier.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::Value;

pub const ID: &str = "render.anti-debuff";

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::boolean(&key("nausea"), "Remove Nausea", true),
        Definition::boolean(&key("blindness"), "Remove Blindness", true),
        Definition::boolean(&key("slowness"), "Remove Slowness", true),
        Definition::boolean(&key("effects"), "Remove Effects", false),
    ]
}

/// The effect ids (without namespace) to take off this tick.
pub fn removed(settings: &Settings) -> Vec<&'static str> {
    let on = |k: &str| {
        settings
            .get(&key(k))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let mut out = Vec::new();
    if on("nausea") {
        out.push("nausea");
    }
    if on("blindness") {
        out.push("blindness");
    }
    if on("slowness") && on("effects") {
        out.push("slowness");
    }
    out
}
