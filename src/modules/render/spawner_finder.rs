// SpawnerFinder (`SpawnerFinder`): a label over each monster spawner whose
// mob is whitelisted, "[distance] Mob 刷怪笼" on a dark box, drawn through
// walls at the reference's nametag scale (0.0167 x max(2, distance / 5) x
// Scale per pixel) at the block's centre a quarter block below it. The
// whitelist matches the mob id or its English name; empty allows all.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::overlay::{Align, Backdrop, Draw, Frame};
use serde_json::{Value, json};

pub const ID: &str = "render.spawner-finder";

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::number(&key("scale"), "Scale", 0.1, 1.0, 1.5, 0.1),
        Definition::boolean(&key("show-distance"), "Show distance", true),
        Definition::new(
            &key("whitelist"),
            "Spawners",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!(["Zombie", "Skeleton", "Creeper", "Spider"]),
        ),
    ]
}

/// `zombie_villager` as "Zombie Villager".
pub fn display_name(id: &str) -> String {
    id.trim_start_matches("minecraft:")
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `SpawnerFinder.matchesSpawner`.
fn allowed(settings: &Settings, mob: &str) -> bool {
    let entries: Vec<&str> = settings
        .get(&key("whitelist"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|e| !e.is_empty() && !e.starts_with('!'))
        .collect();
    if entries.is_empty() {
        return true;
    }
    let id = mob.trim_start_matches("minecraft:");
    let name = display_name(id);
    entries.iter().any(|e| {
        let e = e.trim_start_matches("minecraft:");
        e.eq_ignore_ascii_case(id)
            || e.eq_ignore_ascii_case(&name)
            || e.replace(' ', "_").eq_ignore_ascii_case(id)
    })
}

pub fn draws(settings: &Settings, frame: &Frame) -> Vec<Draw> {
    let number = |k: &str, d: f64| settings.get(&key(k)).and_then(Value::as_f64).unwrap_or(d);
    let scale = number("scale", 1.0);
    let show_distance = settings
        .get(&key("show-distance"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let p = &frame.projection;
    let mut out = Vec::new();
    for (pos, mob) in &frame.spawners {
        if !allowed(settings, mob) {
            continue;
        }
        let anchor = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) - 0.25,
            f64::from(pos[2]) + 0.5,
        ];
        let o = frame.own.position;
        let distance = (0..3)
            .map(|i| (f64::from(pos[i]) - o[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        let pixel = 0.016_666_668 * (distance / 5.0).max(2.0) * scale;
        let (Some(a), Some(b)) = (
            p.project(anchor),
            p.project([anchor[0], anchor[1] + pixel, anchor[2]]),
        ) else {
            continue;
        };
        let s = (a[1] - b[1]).hypot(a[0] - b[0]);
        let mut text = String::new();
        if show_distance {
            text.push_str(&format!("§a[§f{}§a]§r ", distance as i64));
        }
        text.push_str(&display_name(mob));
        text.push_str(" 刷怪笼");
        out.push(Draw::Text {
            x: a[0],
            y: a[1] - 8.0 * s,
            scale: s,
            text,
            color: 0xffff_ffff,
            shadow: false,
            align: Align::Center,
            backdrop: Some(Backdrop {
                fill: 0x8014_1414,
                pad: 2.0 * s,
                edge: 0.0,
                edge_color: 0,
            }),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_whitelist() {
        assert_eq!(display_name("minecraft:cave_spider"), "Cave Spider");
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s.set(&key("whitelist"), json!(["Zombie", "cave spider"]))
            .unwrap();
        assert!(allowed(&s, "minecraft:zombie"));
        assert!(allowed(&s, "cave_spider"));
        assert!(!allowed(&s, "minecraft:blaze"));
        s.set(&key("whitelist"), json!([])).unwrap();
        assert!(allowed(&s, "minecraft:blaze"));
    }
}
