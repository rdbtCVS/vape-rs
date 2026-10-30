// ItemESP: through-wall tags over dropped items (`ItemESP.renderGroupLabel`).
// Each tag lists its stacks' display names, `xN` for counts above one, and the
// distance before the first line. "Group Items" merges items within
// max(1.5, distance / 5) of a group, then merges groups within that radius of
// each other, as `onPostTick`/`mergeNearbyGroups` do; the grouping is rebuilt
// from the frame's items rather than kept between ticks. "Whitelist Only"
// keeps items matching "Allowed Items" (the shared item-limit entries).
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

/// One dropped item this frame.
#[derive(Clone, Debug)]
pub struct Dropped {
    pub item: HeldItem,
    pub count: u32,
    pub max_stack: u32,
    /// Interpolated position.
    pub position: [f64; 3],
    pub distance: f64,
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

pub fn labels(settings: &Settings, items: &[Dropped]) -> Vec<Label> {
    let get = |k: &str| {
        settings
            .get(&key(k))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let allowed: Vec<String> = settings
        .get(&key("allowed-items"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let whitelist = get("whitelist-only");
    let shown = items
        .iter()
        .filter(|d| !whitelist || matches(&allowed, &d.item));
    // Groups: the representative item (first added) and members.
    let mut groups: Vec<Vec<&Dropped>> = Vec::new();
    for item in shown {
        if get("group-items")
            && let Some(group) = groups
                .iter_mut()
                .find(|g| gap(item.position, g[0].position) <= radius(item.distance))
        {
            group.push(item);
            continue;
        }
        groups.push(vec![item]);
    }
    if get("group-items") {
        let mut i = 0;
        while i < groups.len() {
            let mut j = i + 1;
            while j < groups.len() {
                let (a, b) = (groups[i][0], groups[j][0]);
                if gap(a.position, b.position) <= radius((a.distance + b.distance) / 2.0) {
                    let merged = groups.remove(j);
                    groups[i].extend(merged);
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
    let scale = settings
        .get(&key("scale"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    groups
        .into_iter()
        .map(|group| {
            let lead = group[0];
            // `ItemESPGroup.rebuildStacks`: equal items add their counts.
            let mut stacks: Vec<(String, u32, u32)> = Vec::new();
            for d in &group {
                match stacks.iter_mut().find(|s| s.0 == d.item.display_name) {
                    Some(s) => s.1 += d.count,
                    None => stacks.push((d.item.display_name.clone(), d.count, d.max_stack)),
                }
            }
            let lines = stacks
                .iter()
                .enumerate()
                .map(|(i, (name, count, max))| {
                    let mut line = name.clone();
                    if get("distance") && i == 0 {
                        line = format!("§a[§f{}§a]§r {line}", lead.distance as i64);
                    }
                    if *max > 1 && *count > 1 {
                        line = format!("{line}§r x{count}");
                    }
                    line
                })
                .collect();
            let size = if get("auto-scale") {
                0.016_666_667 * (lead.distance / 5.0).max(2.0) * scale
            } else {
                0.033_333_35 * scale
            };
            Label {
                at: [lead.position[0], lead.position[1] + 0.75, lead.position[2]],
                lines,
                scale: size / 0.025,
            }
        })
        .collect()
}
