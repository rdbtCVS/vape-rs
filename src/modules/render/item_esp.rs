// ItemESP: through-wall tags over dropped items (`ItemESP`, with
// `ItemESPGroup`). Each tag lists its stacks' display names, `xN` for counts
// above one, and the distance before the first line.
//
// The groups are kept between ticks (`onPostTick`): with "Group Items" off
// each item is its own group, rebuilt every tick. With it on, each group
// first drops its items that are gone or further than max(1.5, distance / 5)
// (the player's distance to its first item) from its centre and re-centres on
// the rest; groups whose centres are within max(1.5, their average distance /
// 5) merge; then each ungrouped item joins the first group whose centre is
// within max(1.5, its distance / 5) (a running average) or starts its own.
// The tag hangs 0.75 above the centre moved from the last tick's, and its
// distance and auto-scale follow the first item. Equal items in a group add
// their counts. "Whitelist Only" keeps ungrouped items matching "Allowed
// Items" (the shared item-limit entries). Turning "Group Items" over clears
// the groups, as does a new world (the reference's loading and menu
// screens).
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{HeldItem, matches};
use serde_json::{Value, json};

pub const ID: &str = "render.item-esp";

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::boolean(&key("distance"), "Distance", false),
        Definition::boolean(&key("group-items"), "Group Items", false),
        Definition::boolean(&key("auto-scale"), "Auto Scale", true),
        Definition::number(&key("scale"), "Scale", 0.1, 1.0, 1.5, 0.1),
        Definition::boolean(&key("whitelist-only"), "Whitelist Only", false),
        Definition::new(
            &key("allowed-items"),
            "Allowed Items",
            Kind::StringList {
                max_entries: 256,
                max_chars: 256,
            },
            json!([]),
        )
        .when(&key("whitelist-only"), json!(true)),
    ]
}

/// One dropped item at the tick.
#[derive(Clone, Debug)]
pub struct Dropped {
    /// The entity's id.
    pub id: u32,
    pub item: HeldItem,
    pub count: u32,
    pub max_stack: u32,
    /// The position at the tick, and before it.
    pub position: [f64; 3],
    pub previous: [f64; 3],
}

/// A tag: where it hangs, its lines (legacy text) and its scale relative to
/// the host's 0.025 name tag scale.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub at: [f64; 3],
    pub lines: Vec<String>,
    pub scale: f64,
}

fn radius(distance: f64) -> f64 {
    (distance / 5.0).max(1.5)
}

fn gap(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn flag(settings: &Settings, k: &str) -> bool {
    settings
        .get(&key(k))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// `ItemESPGroup`: its items as last seen (the first is the representative)
/// and its centre at the tick and before it.
#[derive(Clone, Debug)]
struct Group {
    members: Vec<Dropped>,
    current: [f64; 3],
    previous: [f64; 3],
}

impl Group {
    /// The constructor and its first `addItem`.
    fn new(item: &Dropped) -> Self {
        Self {
            members: vec![item.clone()],
            current: item.position,
            previous: item.previous,
        }
    }
    /// `addItem`: the centre moves by a running average.
    fn add(&mut self, item: &Dropped) {
        let n = self.members.len() as f64;
        self.current =
            std::array::from_fn(|i| (self.current[i] * n + item.position[i]) / (n + 1.0));
        self.members.push(item.clone());
    }
    fn recentre(&mut self) {
        let n = self.members.len() as f64;
        self.current =
            std::array::from_fn(|i| self.members.iter().map(|m| m.position[i]).sum::<f64>() / n);
    }
}

/// ItemESP's groups between ticks (`ItemESP.groups`).
#[derive(Clone, Debug, Default)]
pub struct Groups {
    groups: Vec<Group>,
    /// "Group Items" as the groups were built.
    grouping: bool,
}

impl Groups {
    pub const fn new() -> Self {
        Self {
            groups: Vec::new(),
            grouping: false,
        }
    }

    pub fn clear(&mut self) {
        self.groups.clear();
    }

    /// `onPostTick`: `items` are the world's dropped items at the tick, `feet`
    /// the player's position.
    pub fn tick(&mut self, settings: &Settings, items: &[Dropped], feet: [f64; 3]) {
        let grouping = flag(settings, "group-items");
        if grouping != self.grouping {
            self.groups.clear();
            self.grouping = grouping;
        }
        let allowed: Vec<String> = settings
            .get(&key("allowed-items"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let whitelist = flag(settings, "whitelist-only");
        let wanted = |d: &&Dropped| !whitelist || matches(&allowed, &d.item);
        if !grouping {
            self.groups = items.iter().filter(wanted).map(Group::new).collect();
            return;
        }
        let find = |id: u32| items.iter().find(|d| d.id == id);
        // `ItemESPGroup.update`.
        for group in &mut self.groups {
            group.previous = group.current;
            let lead = &group.members[0];
            let lead_at = find(lead.id).map_or(lead.position, |d| d.position);
            let (most, centre) = (radius(gap(feet, lead_at)), group.current);
            group.members.retain_mut(|m| match find(m.id) {
                Some(d) if gap(d.position, centre) <= most => {
                    *m = d.clone();
                    true
                }
                _ => false,
            });
            if !group.members.is_empty() {
                group.recentre();
            }
        }
        self.groups.retain(|g| !g.members.is_empty());
        // `mergeNearbyGroups`.
        let mut i = 0;
        while i < self.groups.len() {
            let mut j = i + 1;
            while j < self.groups.len() {
                let (a, b) = (&self.groups[i], &self.groups[j]);
                let average =
                    (gap(feet, a.members[0].position) + gap(feet, b.members[0].position)) / 2.0;
                if gap(a.current, b.current) <= radius(average) {
                    let other = self.groups.remove(j);
                    let first = &mut self.groups[i];
                    for m in other.members {
                        if !first.members.iter().any(|f| f.id == m.id) {
                            first.members.push(m);
                        }
                    }
                    first.recentre();
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        let mut grouped: std::collections::HashSet<u32> = self
            .groups
            .iter()
            .flat_map(|g| g.members.iter().map(|m| m.id))
            .collect();
        for item in items.iter().filter(wanted) {
            if !grouped.insert(item.id) {
                continue;
            }
            let within = radius(gap(feet, item.position));
            match self
                .groups
                .iter_mut()
                .find(|g| gap(item.position, g.current) <= within)
            {
                Some(group) => group.add(item),
                None => self.groups.push(Group::new(item)),
            }
        }
    }

    /// The tags at `partial` of the tick, `feet` being the player's position.
    pub fn labels(&self, settings: &Settings, feet: [f64; 3], partial: f64) -> Vec<Label> {
        let scale = settings
            .get(&key("scale"))
            .and_then(Value::as_f64)
            .unwrap_or(1.0);
        self.groups
            .iter()
            .map(|group| {
                let lead = &group.members[0];
                let distance = gap(feet, lead.position);
                // `rebuildStacks`: with grouping, equal items add their counts.
                let mut stacks: Vec<(&Dropped, u32)> = Vec::new();
                for d in &group.members {
                    let same = |s: &&mut (&Dropped, u32)| match (&s.0.item.id, &d.item.id) {
                        (Some(a), Some(b)) => a == b,
                        _ => s.0.item.display_name == d.item.display_name,
                    };
                    match stacks.iter_mut().find(|s| self.grouping && same(s)) {
                        Some(s) => s.1 += d.count,
                        None => stacks.push((d, d.count)),
                    }
                }
                let lines = stacks
                    .iter()
                    .enumerate()
                    .map(|(i, (d, count))| {
                        let mut line = d.item.display_name.clone();
                        if flag(settings, "distance") && i == 0 {
                            line = format!("§a[§f{}§a]§r {line}", distance as i64);
                        }
                        if d.max_stack > 1 && *count > 1 {
                            line = format!("{line}§r x{count}");
                        }
                        line
                    })
                    .collect();
                let size = if flag(settings, "auto-scale") {
                    0.016_666_667 * (distance / 5.0).max(2.0) * scale
                } else {
                    0.033_333_35 * scale
                };
                let at: [f64; 3] = std::array::from_fn(|i| {
                    group.previous[i] + (group.current[i] - group.previous[i]) * partial
                });
                Label {
                    at: [at[0], at[1] + 0.75, at[2]],
                    lines,
                    scale: size / 0.025,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(pairs: &[(&str, Value)]) -> Settings {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        for (k, v) in pairs {
            s.set(&key(k), v.clone()).unwrap();
        }
        s
    }

    fn dropped(id: u32, item: &str, count: u32, at: [f64; 3]) -> Dropped {
        Dropped {
            id,
            item: HeldItem {
                id: Some(item.to_owned()),
                display_name: item.replace('_', " "),
                ..HeldItem::default()
            },
            count,
            max_stack: 64,
            position: at,
            previous: at,
        }
    }

    const FEET: [f64; 3] = [0.0, 0.0, 0.0];

    #[test]
    fn without_grouping_each_item_is_tagged_where_it_moves() {
        let s = settings(&[("distance", json!(true)), ("auto-scale", json!(false))]);
        let mut groups = Groups::default();
        let mut moving = dropped(1, "diamond", 3, [0.0, 0.0, 10.0]);
        moving.previous = [0.0, 0.0, 8.0];
        let items = [moving, dropped(2, "diamond", 1, [0.0, 0.0, 10.5])];
        groups.tick(&s, &items, FEET);
        let labels = groups.labels(&s, FEET, 0.5);
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].at, [0.0, 0.75, 9.0]);
        assert_eq!(labels[0].lines, ["§a[§f10§a]§r diamond§r x3"]);
        assert_eq!(labels[1].lines, ["§a[§f10§a]§r diamond"]);
        assert!((labels[0].scale - 0.033_333_35 / 0.025).abs() < 1e-9);
    }

    #[test]
    fn grouped_items_keep_their_group_and_centre_between_ticks() {
        let s = settings(&[("group-items", json!(true))]);
        let mut groups = Groups::default();
        let items = |z: f64| {
            [
                dropped(1, "diamond", 2, [0.0, 0.0, 20.0]),
                dropped(2, "diamond", 3, [0.0, 0.0, 21.0]),
                dropped(3, "emerald", 1, [1.0, 0.0, z]),
                // Beyond 1.5 (and 20 / 5 = 4) of the group: its own.
                dropped(4, "stick", 1, [0.0, 0.0, 40.0]),
            ]
        };
        groups.tick(&s, &items(20.0), FEET);
        let labels = groups.labels(&s, FEET, 1.0);
        assert_eq!(labels.len(), 2);
        // Equal items add their counts; the centre is the running average.
        assert_eq!(labels[0].lines, ["diamond§r x5", "emerald"]);
        let at = labels[0].at;
        assert!((at[0] - 1.0 / 3.0).abs() < 1e-9 && (at[2] - 61.0 / 3.0).abs() < 1e-9);
        // Auto Scale follows the first item: 0.0167 x max(20 / 5, 2).
        assert!((labels[0].scale - 0.016_666_667 * 4.0 / 0.025).abs() < 1e-9);
        // The emerald drifts 5 away: it leaves at the next tick and starts its
        // own group, while the tag of the rest moves from the old centre.
        groups.tick(&s, &items(26.0), FEET);
        let labels = groups.labels(&s, FEET, 0.0);
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0].lines, ["diamond§r x5"]);
        assert!((labels[0].at[2] - 61.0 / 3.0).abs() < 1e-9);
        assert_eq!(groups.labels(&s, FEET, 1.0)[0].at, [0.0, 0.75, 20.5]);
        assert_eq!(labels[2].lines, ["emerald"]);
        // Back within reach of the group's centre, the two groups merge.
        groups.tick(&s, &items(21.0), FEET);
        let labels = groups.labels(&s, FEET, 1.0);
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].lines, ["diamond§r x5", "emerald"]);
        // Picked up: gone at the next tick.
        groups.tick(&s, &items(21.0)[3..], FEET);
        assert_eq!(groups.labels(&s, FEET, 1.0).len(), 1);
    }

    #[test]
    fn the_whitelist_and_a_grouping_change_rebuild_the_groups() {
        let mut s = settings(&[
            ("group-items", json!(true)),
            ("whitelist-only", json!(true)),
            ("allowed-items", json!(["diamond"])),
        ]);
        let mut groups = Groups::default();
        let items = [
            dropped(1, "diamond", 1, [0.0, 0.0, 5.0]),
            dropped(2, "diamond", 1, [0.5, 0.0, 5.0]),
            dropped(3, "dirt", 1, [0.0, 0.0, 5.5]),
        ];
        groups.tick(&s, &items, FEET);
        assert_eq!(groups.labels(&s, FEET, 1.0)[0].lines, ["diamond§r x2"]);
        // Without grouping each item is its own tag, its count alone.
        s.set(&key("group-items"), json!(false)).unwrap();
        groups.tick(&s, &items, FEET);
        let labels = groups.labels(&s, FEET, 1.0);
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].lines, ["diamond"]);
        groups.clear();
        assert!(groups.labels(&s, FEET, 1.0).is_empty());
    }
}
