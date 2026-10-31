// MurderFinder (`MurderFinder`, shown as "MurdererFinder"): every other
// player seen holding a murderer item is remembered until the world
// changes and listed under "Murderer List" at the top centre of the
// screen while in view. The reference's legacy ids 288, 396, 421, 398, 369,
// 75, 50 and 352 are the feather, golden carrot, name tag, carrot on a
// stick, blaze rod, redstone torch, torch and bone. With "Callout" each new
// suspect queues one of the "Messages" at random ("%s" is the name), sent
// to chat one per "Delay" ms (`sendChatMessage`). The host's string lists
// have no per-entry toggle, so every message is a candidate, as with the
// reference's defaults (both enabled).
use super::super::super::core::lifecycle::Epoch;
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{HeldItem, matches};
use super::super::super::targeting::entities::EntityClass;
use super::super::rng::Rng;
use super::overlay::{Draw, Frame};
use serde_json::{Value, json};
use std::collections::VecDeque;

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

fn strings(settings: &Settings, k: &str) -> Vec<String> {
    settings
        .get(&key(k))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// The suspects of the current world, by entity id, in the order seen, and
/// the callouts still to send.
#[derive(Debug)]
pub struct MurderFinder {
    world: Option<Epoch>,
    suspects: Vec<i32>,
    pending: VecDeque<String>,
    /// When the last callout went out (`calloutTimer`), in frame ms.
    sent_at: Option<u64>,
    /// The callout to send to chat this frame.
    outbox: Option<String>,
    random: Rng,
}

impl Default for MurderFinder {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x6d75_7264, |d| d.as_nanos() as u64);
        Self {
            world: None,
            suspects: Vec::new(),
            pending: VecDeque::new(),
            sent_at: None,
            outbox: None,
            random: Rng::new(seed),
        }
    }
}

impl MurderFinder {
    /// `onTick`: forget suspects and callouts on a new world, send the
    /// oldest callout once "Delay" ms have passed since the last, then add
    /// every player holding a murderer item in the main hand, queueing a
    /// callout for each with "Callout" on.
    pub fn observe(&mut self, settings: &Settings, epoch: Epoch, frame: &Frame) {
        if self.world != Some(epoch) {
            self.world = Some(epoch);
            self.suspects.clear();
            self.pending.clear();
        }
        let delay = settings
            .get(&key("delay"))
            .and_then(Value::as_f64)
            .unwrap_or(3100.0) as u64;
        if !self.pending.is_empty()
            && self
                .sent_at
                .is_none_or(|t| frame.now_ms.saturating_sub(t) >= delay)
        {
            self.outbox = self.pending.pop_front();
            self.sent_at = Some(frame.now_ms);
        }
        let callout = settings
            .get(&key("callout"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let items = strings(settings, "murderer-items");
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
            if !matches(&items, &held) {
                continue;
            }
            self.suspects.push(id);
            let messages = strings(settings, "messages");
            if let Some(last) = messages.len().checked_sub(1).filter(|_| callout) {
                // `Math.round(size * random)`, clamped to the last.
                let i = ((messages.len() as f64 * self.random.double()).round() as usize).min(last);
                self.pending
                    .push_back(messages[i].replace("%s", &e.view.name));
            }
        }
    }
    /// The callout to send to chat this frame, once.
    pub fn take_callout(&mut self) -> Option<String> {
        self.outbox.take()
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

#[cfg(test)]
mod tests {
    use super::super::overlay::{Item, Living, Own, Projection};
    use super::*;
    use crate::core::lifecycle::Session;
    use crate::targeting::filter::tests::entity;

    fn settings() -> Settings {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s
    }

    fn holding(id: i32, name: &str, item: &str) -> Living {
        let mut view = entity(EntityClass::Player, name, [0.0; 3]);
        view.handle.id = id;
        let mut equipment: [Option<Item>; 6] = Default::default();
        equipment[0] = Some(Item {
            id: item.into(),
            count: 1,
            ..Item::default()
        });
        Living {
            view,
            max_health: 20.0,
            absorption: 0.0,
            display_name: None,
            creative: false,
            equipment,
            hurt: false,
        }
    }

    fn frame(now_ms: u64, entities: Vec<Living>) -> Frame {
        Frame {
            projection: Projection {
                camera: [0.0; 3],
                matrix: [0.0; 16],
                width: 400.0,
                height: 200.0,
            },
            entities,
            own: Own::default(),
            first_person: true,
            now_ms,
            spawners: Vec::new(),
            indicators: Vec::new(),
            bed_plates: Vec::new(),
        }
    }

    #[test]
    fn callouts_name_each_new_suspect_one_per_delay() {
        let mut s = settings();
        let epoch = Session::default().epoch();
        let mut finder = MurderFinder::default();
        let both = vec![
            holding(1, "Alex", "minecraft:iron_sword"),
            holding(2, "Sam", "minecraft:bone"),
            holding(3, "Kai", "minecraft:bread"),
        ];
        // Off: suspects are listed, nothing is said.
        finder.observe(&s, epoch, &frame(0, both.clone()));
        finder.observe(&s, epoch, &frame(5000, both.clone()));
        assert_eq!(finder.suspects, [1, 2]);
        assert_eq!(finder.take_callout(), None);

        s.set(&key("callout"), json!(true)).unwrap();
        s.set(&key("messages"), json!(["%s is the murderer!"]))
            .unwrap();
        let mut finder = MurderFinder::default();
        finder.observe(&s, epoch, &frame(0, both.clone()));
        // Queued this tick, sent from the next.
        assert_eq!(finder.take_callout(), None);
        finder.observe(&s, epoch, &frame(50, both.clone()));
        assert_eq!(
            finder.take_callout().as_deref(),
            Some("Alex is the murderer!")
        );
        assert_eq!(finder.take_callout(), None);
        finder.observe(&s, epoch, &frame(3000, both.clone()));
        assert_eq!(finder.take_callout(), None);
        finder.observe(&s, epoch, &frame(3150, both.clone()));
        assert_eq!(
            finder.take_callout().as_deref(),
            Some("Sam is the murderer!")
        );
        finder.observe(&s, epoch, &frame(9000, both));
        assert_eq!(finder.take_callout(), None);
    }

    #[test]
    fn callouts_pick_from_every_message() {
        let mut s = settings();
        s.set(&key("callout"), json!(true)).unwrap();
        s.set(&key("delay"), json!(0)).unwrap();
        let epoch = Session::default().epoch();
        let mut finder = MurderFinder {
            random: Rng::new(7),
            ..MurderFinder::default()
        };
        let mut said = std::collections::BTreeSet::new();
        for id in 0..64 {
            finder.observe(&s, epoch, &frame(0, vec![holding(id, "Alex", "feather")]));
            said.extend(finder.take_callout());
        }
        assert_eq!(
            said.into_iter().collect::<Vec<_>>(),
            ["Alex is the murderer!", "i saw that Alex!"]
        );
    }
}
