// MurderFinder (`MurderFinder`, shown as "MurdererFinder"): every other
// player seen holding a murderer item is remembered until the world
// changes and listed under "Murderer List" at the top centre of the
// screen while in view. The reference's legacy ids 288, 396, 421, 398, 369,
// 75, 50 and 352 are the feather, golden carrot, name tag, carrot on a
// stick, blaze rod, redstone torch, torch and bone. The chat "Callout" is
// kept as settings only: the host has no chat-send path for modules yet.
use super::super::super::core::lifecycle::Epoch;
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{HeldItem, matches};
use super::super::super::targeting::entities::EntityClass;
use super::overlay::{Draw, Frame};
use serde_json::{Value, json};

pub const ID: &str = "render.murder-finder";

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    let list = |k: &str, label: &str, default: Value| {
        Definition::new(
            &key(k),
            label,
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            default,
        )
    };
    vec![
        Definition::boolean(&key("callout"), "Callout", false),
        Definition::number(&key("delay"), "Delay", 0.0, 3100.0, 5000.0, 1.0),
        list(
            "messages",
            "Messages",
            json!(["%s is the murderer!", "i saw that %s!"]),
        ),
        list(
            "murderer-items",
            "Murderer Items",
            json!([
                "swords",
                "shovels",
                "axes",
                "pickaxes",
                "feather",
                "golden_carrot",
                "name_tag",
                "carrot_on_a_stick",
                "blaze_rod",
                "redstone_torch",
                "torch",
                "bone"
            ]),
        ),
    ]
}

/// The suspects of the current world, by entity id, in the order seen.
#[derive(Debug, Default)]
pub struct MurderFinder {
    world: Option<Epoch>,
    suspects: Vec<i32>,
}

impl MurderFinder {
    /// `onTick`: forget suspects on a new world, then add every player
    /// holding a murderer item in the main hand.
    pub fn observe(&mut self, settings: &Settings, epoch: Epoch, frame: &Frame) {
        if self.world != Some(epoch) {
            self.world = Some(epoch);
            self.suspects.clear();
        }
        let items: Vec<String> = settings
            .get(&key("murderer-items"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        for e in &frame.entities {
            let id = e.view.handle.id;
            if e.view.class != EntityClass::Player || self.suspects.contains(&id) {
                continue;
            }
            let Some(held) = &e.equipment[0] else {
                continue;
            };
            let held = HeldItem {
                id: Some(held.id.trim_start_matches("minecraft:").to_owned()),
                slot: usize::MAX,
                ..HeldItem::default()
            };
            if matches(&items, &held) {
                self.suspects.push(id);
            }
        }
    }
    /// `onRender2D`: the list, of suspects still in the world.
    pub fn draws(&self, frame: &Frame) -> Vec<Draw> {
        if self.suspects.is_empty() {
            return Vec::new();
        }
        let x = (frame.projection.width / 2.0).floor() - 20.0;
        let mut out = vec![Draw::text(x, 15.0, "§nMurderer List", 0xffff_ffff)];
        let mut y = 25.0;
        for e in &frame.entities {
            if e.view.class == EntityClass::Player && self.suspects.contains(&e.view.handle.id) {
                out.push(Draw::text(x, y, e.view.name.clone(), 0xffff_ffff));
                y += 10.0;
            }
        }
        out
    }
}
