// NoSlowdown (reference `NoSlowdown`, Category.HIDDEN): the item-use
// slowdown is skipped while using an item (optionally only whitelisted
// ones), except in water and after a knockback until back on the ground.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use serde_json::{Value, json};

pub const ID: &str = "blatant.no-slowdown";

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    vec![
        Definition::boolean(&key("limit-items"), "Limit Items", false),
        Definition::new(
            &key("whitelist"),
            "Whitelisted",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!(["swords"]),
        )
        .when(&key("limit-items"), json!(true)),
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub limit_items: bool,
    pub whitelist: Vec<String>,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        Self {
            limit_items: get("limit-items").and_then(Value::as_bool).unwrap_or(false),
            whitelist: get("whitelist")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        }
    }
}

/// One pre-motion of host state.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    pub on_ground: bool,
    pub in_water: bool,
    /// An item in use slows the move this tick.
    pub slowed: bool,
    pub held: HeldItem,
}

#[derive(Debug, Default)]
pub struct NoSlowdown {
    pending_velocity: bool,
}

impl NoSlowdown {
    pub fn reset(&mut self) {
        self.pending_velocity = false;
    }
    /// An explosion, or a motion packet for the local player.
    pub fn knockback(&mut self) {
        self.pending_velocity = true;
    }
    /// Whether this tick's move skips the slowdown.
    pub fn tick(&mut self, o: &Options, t: &Tick) -> bool {
        if self.pending_velocity {
            if t.on_ground {
                self.pending_velocity = false;
            }
            return false;
        }
        !t.in_water && t.slowed && (!o.limit_items || item_limit::matches(&o.whitelist, &t.held))
    }
}
