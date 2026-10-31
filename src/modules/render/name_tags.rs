// NameTags: through-wall name tags for players, animals and mobs, replacing
// the host's own player tags while on. The label follows the reference
// `NameTags.buildNameString`: optional `[distance]`, `[C]` for creative
// players, the health in half-hearts coloured by its fraction, absorption
// ("Effects") in gold and the strength indicator. The host draws each tag as
// a host name tag (font, background) through walls at the returned scale and
// opacity. The equipment row is drawn on the HUD at the tag's projected size
// (`equipment_row`); enchantment glint and durability overlays are not.
use super::super::super::core::settings::{Definition, Settings};
use super::super::super::targeting::entities::EntityClass;
use serde_json::{Value, json};

pub const ID: &str = "render.name-tags";

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}
fn boolean(settings: &Settings, k: &str) -> bool {
    settings
        .get(&key(k))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
fn number(settings: &Settings, k: &str, default: f64) -> f64 {
    settings
        .get(&key(k))
        .and_then(Value::as_f64)
        .unwrap_or(default)
}

pub fn definitions() -> Vec<Definition> {
    let group = |prefix: &str, label: &str, on: bool, players: bool| {
        let render = key(prefix);
        let mut out = vec![Definition::boolean(&render, label, on)];
        let sub = |k: &str| key(&format!("{prefix}-{k}"));
        out.push(Definition::boolean(&sub("health"), "Health", false).when(&render, json!(true)));
        out.push(
            Definition::boolean(&sub("distance"), "Distance", false).when(&render, json!(true)),
        );
        out.push(Definition::boolean(&sub("effects"), "Effects", false).when(&render, json!(true)));
        out.push(
            Definition::number(&sub("max-distance"), "Max Distance", 0.0, 0.0, 250.0, 1.0)
                .when(&render, json!(true)),
        );
        if players {
            out.push(
                Definition::boolean(&key("equipment"), "Equipment", false)
                    .when(&render, json!(true)),
            );
            out.push(
                Definition::boolean(&key("strength-indicator"), "Strength Indicator", false)
                    .when(&render, json!(true)),
            );
            out.push(
                Definition::boolean(&key("calculate-effects"), "Calculate Effects", false)
                    .when(&render, json!(true)),
            );
        }
        out
    };
    let mut out = vec![
        Definition::boolean(&key("ignore-invisibles"), "Ignore Invisibles", false),
        Definition::boolean(&key("auto-scale"), "Auto Scale", true),
        Definition::number(&key("scale"), "Scale", 0.1, 1.0, 1.5, 0.1),
        Definition::number(&key("opacity"), "Opacity", 0.0, 1.0, 1.0, 0.1),
        Definition::boolean(&key("hide-bots"), "Hide bots", true),
    ];
    out.extend(group("players", "Render Players", true, true));
    out.extend(group("animals", "Render Animals", false, false));
    out.extend(group("mobs", "Render Mobs", false, false));
    out
}

/// One candidate entity, as the host sees it this frame.
#[derive(Clone, Debug)]
pub struct Subject {
    pub class: EntityClass,
    /// The display name (player name, custom name or type name).
    pub name: String,
    pub distance: f64,
    pub health: f32,
    pub max_health: f32,
    pub absorption: f32,
    pub invisible: bool,
    /// Eye-to-eye line of sight; a visible tag ignores Opacity.
    pub visible: bool,
    pub bot: bool,
    pub creative: bool,
    /// Held weapon plus worn armor scores (`ItemStackScoreUtil`).
    pub threat: f64,
}

/// A tag to draw: legacy-formatted text, the base colour for unformatted
/// runs, the scale relative to the host's own name tag and the opacity.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    pub text: String,
    pub color: u32,
    pub scale: f64,
    pub alpha: f64,
}

/// Which host player tags NameTags replaces (all of them while players render).
pub fn replaces_players(settings: &Settings) -> bool {
    boolean(settings, "players")
}

fn half(value: f64) -> String {
    let v = ((value + 0.25) / 0.5).floor() * 0.5;
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.1}")
    }
}

/// `NameTags.computeNameColor` (friend recolouring is applied by the caller).
fn base_color(subject: &Subject, friend: Option<u32>) -> u32 {
    if subject.invisible {
        return 0x0000_fffa;
    }
    friend.map_or(0xff_ffff, |c| c & 0xff_ffff)
}

/// The tag for `subject`, if NameTags shows it. `own_threat` is the local
/// player's weapon plus armor score; `friend` the friend's alias and colour.
pub fn tag(
    settings: &Settings,
    subject: &Subject,
    own_threat: f64,
    friend: Option<(&str, Option<u32>)>,
) -> Option<Tag> {
    if boolean(settings, "hide-bots") && subject.bot {
        return None;
    }
    if boolean(settings, "ignore-invisibles") && subject.invisible {
        return None;
    }
    let group = match subject.class {
        EntityClass::Player => "players",
        EntityClass::Peaceful | EntityClass::Ambient => "animals",
        EntityClass::Monster => "mobs",
        EntityClass::Other => return None,
    };
    if !boolean(settings, group) {
        return None;
    }
    let max = number(settings, &format!("{group}-max-distance"), 0.0);
    if max != 0.0 && subject.distance > max {
        return None;
    }
    let player = subject.class == EntityClass::Player;
    let mut text = match friend {
        Some((alias, _)) if player => alias.to_owned(),
        _ => format!("§a§r{}", subject.name),
    };
    if boolean(settings, &format!("{group}-distance")) {
        text = format!("§a[§f{}§a]§r {text}", subject.distance as i64);
    }
    if player && subject.creative {
        text = format!("§a[C] §r{text}");
    }
    let health = f64::from(subject.health) / 2.0;
    let max_health = (f64::from(subject.max_health) / 2.0).max(f64::EPSILON);
    let percent = 100.0 * health / max_health;
    let code = if percent > 75.0 {
        '2'
    } else if percent > 50.0 {
        'e'
    } else if percent > 25.0 {
        '6'
    } else {
        '4'
    };
    if boolean(settings, &format!("{group}-health")) {
        text = format!("{text} §{code}{}", half(health));
    }
    if boolean(settings, &format!("{group}-effects")) && subject.absorption > 0.0 {
        text = format!("{text} §6{}", half(f64::from(subject.absorption)));
    }
    if player && boolean(settings, "strength-indicator") {
        let label = if own_threat > subject.threat {
            "§a+"
        } else if own_threat < subject.threat {
            "§c-"
        } else {
            "§e="
        };
        text = format!("{text} {label}");
    }
    let scale = number(settings, "scale", 1.0);
    // 0.0333 * Scale, or 0.0167 * max(2, distance / 5) * Scale with Auto
    // Scale, relative to the host's 0.025 name tag scale.
    let size = if boolean(settings, "auto-scale") {
        0.016_666_667 * (subject.distance / 5.0).max(2.0) * scale
    } else {
        0.033_333_35 * scale
    };
    let alpha = if subject.visible {
        1.0
    } else {
        number(settings, "opacity", 1.0).clamp(0.0, 1.0)
    };
    Some(Tag {
        text,
        color: base_color(subject, friend.and_then(|(_, c)| c)),
        scale: size / 0.025,
        alpha,
    })
}

/// World units per tag pixel (`0.0333 * Scale`, or with Auto Scale
/// `0.0167 * max(2, distance / 5) * Scale`).
fn pixel(settings: &Settings, distance: f64) -> f64 {
    let scale = number(settings, "scale", 1.0);
    if boolean(settings, "auto-scale") {
        0.016_666_667 * (distance / 5.0).max(2.0) * scale
    } else {
        0.033_333_35 * scale
    }
}

/// The players' equipment row (`NameTags.renderEquipment`): main hand,
/// helmet to boots, then off hand, 16 tag pixels each at 1.1x, centred 26
/// pixels above the tag, drawn on the HUD at the tag's projected size.
pub fn equipment_row(
    settings: &Settings,
    frame: &super::overlay::Frame,
    people: &impl super::super::super::targeting::filter::People,
) -> Vec<super::overlay::Draw> {
    let mut out = Vec::new();
    if !boolean(settings, "players") || !boolean(settings, "equipment") {
        return out;
    }
    let max = number(settings, "players-max-distance", 0.0);
    let p = &frame.projection;
    for living in &frame.entities {
        let e = &living.view;
        if e.class != EntityClass::Player || e.health <= 0.0 {
            continue;
        }
        if (boolean(settings, "hide-bots") && people.bot(e))
            || (boolean(settings, "ignore-invisibles") && e.invisible)
        {
            continue;
        }
        let distance = (0..3)
            .map(|i| (e.position[i] - frame.own.position[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if max != 0.0 && distance > max {
            continue;
        }
        let q = &living.equipment;
        let items: Vec<_> = [&q[0], &q[2], &q[3], &q[4], &q[5], &q[1]]
            .into_iter()
            .flatten()
            .collect();
        if items.is_empty() {
            continue;
        }
        let height = e.bounds.max[1] - e.bounds.min[1];
        let anchor = [e.position[0], e.position[1] + height + 0.4, e.position[2]];
        let unit = pixel(settings, distance) * 1.1;
        let (Some(a), Some(b)) = (
            p.project(anchor),
            p.project([anchor[0], anchor[1] + unit, anchor[2]]),
        ) else {
            continue;
        };
        let s = (a[1] - b[1]).hypot(a[0] - b[0]);
        let width = items.len() as f32 * 16.0 * s;
        let y = a[1] - 26.0 * s;
        for (i, item) in items.into_iter().enumerate() {
            out.push(super::overlay::Draw::Item {
                x: a[0] - width / 2.0 + i as f32 * 16.0 * s,
                y,
                scale: s,
                item: item.clone(),
            });
        }
    }
    out
}
