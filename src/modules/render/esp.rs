// ESP and Tracers. ESP's 3D mode is world-space box lines through walls;
// 2D projects each player's box onto the HUD (bounding box, health bar,
// name); Outline lends the host's glowing outline pass the ESP colour.
// Skeleton is not offered: the reference hides it on 1.17+ and the host has
// no per-bone pose access.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::targeting::entities::{EntityClass, EntityView, distance};
use super::super::super::targeting::filter::People;
use super::overlay::{Align, Backdrop, Draw, Frame};
use super::world::{Line, Point, outline_box};
use serde_json::{Value, json};

pub const ESP: &str = "render.esp";
pub const TRACERS: &str = "render.tracers";

fn boolean(settings: &Settings, id: &str, default: bool) -> bool {
    settings.get(id).and_then(Value::as_bool).unwrap_or(default)
}
fn color(settings: &Settings, id: &str, default: u32) -> u32 {
    settings
        .get(id)
        .and_then(Value::as_u64)
        .map_or(default, |c| c as u32)
}
fn range(settings: &Settings, id: &str, default: [f64; 2]) -> [f64; 2] {
    settings
        .get(id)
        .and_then(Value::as_array)
        .and_then(|r| Some([r.first()?.as_f64()?, r.get(1)?.as_f64()?]))
        .unwrap_or(default)
}

/// Friend/enemy recolouring (`FriendManager.recolorVisuals`/`friendColor`,
/// `EnemyManager.useColor`/`enemyColor`).
pub fn people_definitions() -> Vec<Definition> {
    vec![
        Definition::boolean("friends.recolor", "Recolor visuals", true),
        Definition::new(
            "friends.color",
            "Friends Color",
            Kind::Color,
            json!(0xff42_f489_u32),
        )
        .when("friends.recolor", json!(true)),
        Definition::boolean("enemies.use-color", "Use color", true),
        Definition::new(
            "enemies.color",
            "Enemies Color",
            Kind::Color,
            json!(0xfff4_4242_u32),
        )
        .when("enemies.use-color", json!(true)),
    ]
}

/// The friend or enemy colour for a player, when recolouring applies.
fn person_color(settings: &Settings, entity: &EntityView, people: &impl People) -> Option<u32> {
    if people.friend(&entity.name) && boolean(settings, "friends.recolor", true) {
        return Some(color(settings, "friends.color", 0xff42_f489));
    }
    if people.enemy(&entity.name) && boolean(settings, "enemies.use-color", true) {
        return Some(color(settings, "enemies.color", 0xfff4_4242));
    }
    // TargetFilter's "Recolor visuals" (`ClientSettings.resolveTeamColor`).
    people.team_color(entity).map(|c| 0xff00_0000 | c)
}

pub fn definitions() -> Vec<Definition> {
    let esp = |k: &str| format!("{ESP}.{k}");
    let tracer = |k: &str| format!("{TRACERS}.{k}");
    let distance = |k: &str, label: &str, check: &str| {
        Definition::new(
            &tracer(k),
            label,
            Kind::Range {
                min: 0.0,
                max: 256.0,
                step: 1.0,
            },
            json!([0.0, 32.0]),
        )
        .when(&tracer(check), json!(true))
    };
    vec![
        Definition::new(
            &esp("player-color"),
            "Player Color",
            Kind::Color,
            json!(0xff24_c164_u32),
        ),
        Definition::choice(&esp("mode"), "Mode", &["3D", "2D", "Outline"], "3D"),
        Definition::boolean(&esp("hitbox"), "Hitbox", false).when(&esp("mode"), json!("3D")),
        Definition::boolean(&esp("show-normal"), "Show Normal", false)
            .when(&esp("hitbox"), json!(true))
            .when(&esp("mode"), json!("3D")),
        Definition::boolean(&esp("bounding-box"), "Bounding Box", true)
            .when(&esp("mode"), json!("2D")),
        Definition::boolean(&esp("priority-only"), "Priority Only", false)
            .when(&esp("bounding-box"), json!(true))
            .when(&esp("mode"), json!("2D")),
        Definition::boolean(&esp("health-bar"), "Health Bar", false)
            .when(&esp("mode"), json!("2D")),
        Definition::boolean(&esp("name"), "Name", false).when(&esp("mode"), json!("2D")),
        Definition::boolean(&esp("use-displayname"), "Use Displayname", false)
            .when(&esp("name"), json!(true))
            .when(&esp("mode"), json!("2D")),
        Definition::boolean(&esp("show-background"), "Show Background", false)
            .when(&esp("name"), json!(true))
            .when(&esp("mode"), json!("2D")),
        Definition::boolean(&esp("invisibles"), "Invisibles", false),
        Definition::boolean(&esp("hide-bots"), "Hide Bots", false),
        Definition::boolean(&tracer("enemy-only"), "Enemy Only", false)
            .when(&tracer("players"), json!(true)),
        Definition::boolean(&tracer("invisibles"), "Invisibles", false),
        Definition::boolean(&tracer("color-by-distance"), "Color by distance", false),
        Definition::boolean(
            &tracer("highlight-focusing"),
            "Highlight if focusing",
            false,
        ),
        Definition::boolean(&tracer("players"), "Render Players", true),
        Definition::boolean(&tracer("player-distance-check"), "Distance Check", false)
            .when(&tracer("players"), json!(true)),
        distance(
            "player-distance",
            "Player Distance",
            "player-distance-check",
        ),
        Definition::new(
            &tracer("player-color"),
            "Player Color",
            Kind::Color,
            json!(0xff00_96ff_u32),
        )
        .when(&tracer("players"), json!(true)),
        Definition::boolean(&tracer("animals"), "Render Animals", false),
        Definition::boolean(&tracer("animal-distance-check"), "Distance Check", false)
            .when(&tracer("animals"), json!(true)),
        distance(
            "animal-distance",
            "Animal Distance",
            "animal-distance-check",
        ),
        Definition::new(
            &tracer("animal-color"),
            "Animal Color",
            Kind::Color,
            json!(0xffff_ffff_u32),
        )
        .when(&tracer("animals"), json!(true)),
        Definition::boolean(&tracer("mobs"), "Render Mobs", false),
        Definition::boolean(&tracer("mob-distance-check"), "Distance Check", false)
            .when(&tracer("mobs"), json!(true)),
        distance("mob-distance", "Mob Distance", "mob-distance-check"),
        Definition::new(
            &tracer("mob-color"),
            "Mob Color",
            Kind::Color,
            json!(0xffff_9a00_u32),
        )
        .when(&tracer("mobs"), json!(true)),
    ]
}

/// A line start meaning "one block in front of the frame's camera", which
/// the host resolves with the real (possibly third-person) camera.
pub const VIEW_CENTRE: Point = Point([f64::NAN; 3]);

fn mode(settings: &Settings) -> &str {
    settings
        .get(&format!("{ESP}.mode"))
        .and_then(Value::as_str)
        .unwrap_or("3D")
}

/// Whether ESP draws on the HUD overlay (2D mode); 3D boxes are world lines
/// and Outline is the host's glow.
pub fn draws_2d(settings: &Settings) -> bool {
    mode(settings) == "2D"
}

/// The ESP colour for a player: friend/enemy recolouring, else the setting.
fn player_color(settings: &Settings, entity: &EntityView, people: &impl People) -> u32 {
    person_color(settings, entity, people)
        .unwrap_or_else(|| color(settings, &format!("{ESP}.player-color"), 0xff24_c164))
}

/// Whether ESP draws this player at all (`ESP.isValidEntity`).
fn esp_shows(settings: &Settings, e: &EntityView, people: &impl People) -> bool {
    let key = |k: &str| format!("{ESP}.{k}");
    e.class == EntityClass::Player
        && e.alive()
        && (boolean(settings, &key("invisibles"), false) || !e.fully_invisible())
        && !(boolean(settings, &key("hide-bots"), false) && people.bot(e))
}

/// `ESPOutline`: the glowing-outline colour for an entity in Outline mode.
pub fn outline_color(settings: &Settings, e: &EntityView, people: &impl People) -> Option<u32> {
    (mode(settings) == "Outline" && esp_shows(settings, e, people))
        .then(|| player_color(settings, e, people) & 0x00ff_ffff)
}

/// The reference health-bar colour steps.
pub fn health_color(fraction: f32) -> u32 {
    let (r, g) = if fraction >= 0.9 {
        (0.0, 1.0)
    } else if fraction >= 0.75 {
        (0.9, 1.0)
    } else if fraction >= 0.5 {
        (1.0, 1.0)
    } else if fraction >= 0.25 {
        (1.0, 0.5)
    } else {
        (1.0, 0.0)
    };
    0xff00_0000 | (((r * 255.0) as u32) << 16) | (((g * 255.0) as u32) << 8)
}

fn with_alpha(color: u32, alpha: f32) -> u32 {
    ((alpha.clamp(0.0, 1.0) * 255.0) as u32) << 24 | (color & 0x00ff_ffff)
}

/// `ESP2D.drawPendingBounds`: per player, its projected box (two dark
/// frames around a coloured one), a health bar left of it and its name
/// above, all in HUD units under the game HUD.
pub fn esp_2d(
    settings: &Settings,
    frame: &Frame,
    people: &impl People,
    aliases: &[(String, String)],
) -> Vec<Draw> {
    let key = |k: &str| format!("{ESP}.{k}");
    let mut out = Vec::new();
    if mode(settings) != "2D" {
        return out;
    }
    let bounding = boolean(settings, &key("bounding-box"), true);
    let priority_only = boolean(settings, &key("priority-only"), false);
    let health_bar = boolean(settings, &key("health-bar"), false);
    let name = boolean(settings, &key("name"), false);
    let display = boolean(settings, &key("use-displayname"), false);
    let background = boolean(settings, &key("show-background"), false);
    for living in &frame.entities {
        let e = &living.view;
        if !esp_shows(settings, e, people) {
            continue;
        }
        let offset = [0, 1, 2].map(|i| e.position[i] - e.tick_position[i]);
        let at = |p: [f64; 3]| [0, 1, 2].map(|i| p[i] + offset[i]);
        let Some(r) = frame
            .projection
            .project_box(at(e.bounds.min), at(e.bounds.max))
        else {
            continue;
        };
        let color = player_color(settings, e, people);
        let friend = people.friend(&e.name);
        let priority = friend || people.enemy(&e.name);
        let [left, top, right, bottom] = r;
        if bounding && (!priority_only || priority) {
            let dark = with_alpha(0, 0.4 * (color >> 24) as f32 / 255.0);
            Draw::frame(&mut out, r, 1.0, dark);
            Draw::frame(
                &mut out,
                [left + 1.0, top + 1.0, right - 1.0, bottom - 1.0],
                1.0,
                dark,
            );
            Draw::frame(
                &mut out,
                [left + 0.5, top + 0.5, right - 0.5, bottom - 0.5],
                1.0,
                color,
            );
        }
        let health = e.health + living.absorption;
        if health_bar && health >= 0.0 && living.max_health > 0.0 {
            let fraction = (health / living.max_health).min(1.0);
            out.push(Draw::Quad {
                corners: [
                    [left - 2.0, bottom - 0.5],
                    [left - 2.0, top + 0.5],
                    [left - 4.0, top + 0.5],
                    [left - 4.0, bottom - 0.5],
                ],
                color: 0x6600_0000,
            });
            let bar_top = top + (bottom - top - 1.0) * fraction;
            out.push(Draw::Quad {
                corners: [
                    [left - 2.5, bar_top],
                    [left - 2.5, top + 1.0],
                    [left - 3.5, top + 1.0],
                    [left - 3.5, bar_top],
                ],
                color: health_color(fraction),
            });
        }
        if name {
            let mut label = match (&living.display_name, display && !priority) {
                (Some(d), true) => d.clone(),
                _ => e.name.clone(),
            };
            if display {
                label.retain(|c| c == '\u{a7}' || c == '^' || c.is_ascii());
            }
            if friend
                && let Some((_, alias)) = aliases
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(&e.name))
            {
                label.clone_from(alias);
            }
            let centre = left + (right - left) / 2.0;
            let backdrop = background.then_some(Backdrop {
                fill: if priority { color } else { 0x5f00_0000 },
                pad: 1.5,
                edge: if living.hurt { 1.5 } else { 0.5 },
                edge_color: if living.hurt {
                    0xc8ff_0000
                } else {
                    0x6600_0000
                },
            });
            out.push(Draw::Text {
                x: centre,
                y: top - 8.0,
                scale: 0.9,
                text: label,
                color: if priority { 0xffff_ffff } else { color },
                shadow: true,
                align: Align::Center,
                backdrop,
            });
        }
    }
    out
}

/// The camera for one frame: the interpolated eye and the unit view vector.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: [f64; 3],
    pub look: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}

/// `ESP3D.onRender3D`: a box around each other player's bounding box, grown
/// by the HitBoxes expansion with "Hitbox" on, plus the true box in the
/// inverse colour with "Show Normal".
pub fn esp(
    settings: &Settings,
    entities: &[EntityView],
    people: &impl People,
    hitbox_expansion: Option<f64>,
    out: &mut Vec<Line>,
) {
    let key = |k: &str| format!("{ESP}.{k}");
    if mode(settings) != "3D" {
        return;
    }
    let hide_bots = boolean(settings, &key("hide-bots"), false);
    let hitbox = boolean(settings, &key("hitbox"), false);
    let show_normal = boolean(settings, &key("show-normal"), false);
    let invisibles = boolean(settings, &key("invisibles"), false);
    let base = color(settings, &key("player-color"), 0xff24_c164);
    for e in entities {
        if e.class != EntityClass::Player
            || !e.alive()
            || (!invisibles && e.fully_invisible())
            || (hide_bots && people.bot(e))
        {
            continue;
        }
        let color = person_color(settings, e, people).unwrap_or(base);
        let offset = [0, 1, 2].map(|i| e.position[i] - e.tick_position[i]);
        let grow = if hitbox {
            hitbox_expansion.unwrap_or(0.0)
        } else {
            0.0
        };
        let corner =
            |p: [f64; 3], sign: f64| Point([0, 1, 2].map(|i| p[i] + offset[i] + sign * grow));
        outline_box(
            out,
            corner(e.bounds.min, -1.0),
            corner(e.bounds.max, 1.0),
            color,
            1.5,
        );
        if grow > 0.0 && show_normal {
            let inverse = 0x4600_0000 | (!color & 0x00ff_ffff);
            let at = |p: [f64; 3]| Point([0, 1, 2].map(|i| p[i] + offset[i]));
            outline_box(out, at(e.bounds.min), at(e.bounds.max), inverse, 1.5);
        }
    }
}

/// `Tracers.onRender`: a line from just in front of the eye to each
/// selected entity's eye height. Players, mobs (monsters) and animals (other
/// living entities) each have a toggle, a colour and an optional distance
/// range. "Color by distance" shades red (near) to green (far) around the
/// median distance, and "Highlight if focusing" draws players looking at
/// the viewer (within 5 degrees) thicker and outlined, others thinner.
pub fn tracers(
    settings: &Settings,
    camera: &Camera,
    entities: &[EntityView],
    people: &impl People,
    out: &mut Vec<Line>,
) {
    let key = |k: &str| format!("{TRACERS}.{k}");
    let invisibles = boolean(settings, &key("invisibles"), false);
    let enemy_only = boolean(settings, &key("enemy-only"), false);
    let mut selected: Vec<(&EntityView, u32, f64)> = Vec::new();
    for e in entities {
        if !e.alive() || (!invisibles && e.fully_invisible()) {
            continue;
        }
        let d = distance(camera.eye, e.position);
        let (group, default) = match e.class {
            EntityClass::Player => ("player", 0xff00_96ff),
            EntityClass::Monster => ("mob", 0xffff_9a00),
            _ => ("animal", 0xffff_ffff),
        };
        let plural = match group {
            "player" => "players",
            "mob" => "mobs",
            _ => "animals",
        };
        if !boolean(settings, &key(plural), group == "player") {
            continue;
        }
        if e.class == EntityClass::Player {
            // Bots never get a tracer; "Enemy Only" keeps valid targets
            // (`isValidTarget`: not a friend or teammate). The reference's
            // "Enemies List Only" is never registered, so stays off.
            if people.bot(e) {
                continue;
            }
            let wanted = !people.friend(&e.name) && !people.teammate(e);
            if enemy_only && !wanted {
                continue;
            }
        }
        if boolean(settings, &key(&format!("{group}-distance-check")), false) {
            let [lo, hi] = range(settings, &key(&format!("{group}-distance")), [0.0, 32.0]);
            if d < lo || d > hi {
                continue;
            }
        }
        let color = (e.class == EntityClass::Player)
            .then(|| person_color(settings, e, people))
            .flatten()
            .unwrap_or_else(|| color(settings, &key(&format!("{group}-color")), default));
        selected.push((e, color, d));
    }
    if selected.is_empty() {
        return;
    }
    if boolean(settings, &key("color-by-distance"), false) {
        let mut distances: Vec<f64> = selected.iter().map(|s| s.2).collect();
        distances.sort_by(f64::total_cmp);
        let median = distances[distances.len() / 2].max(1.0);
        for s in &mut selected {
            let t = (s.2 / (median * 2.0)).clamp(0.0, 1.0);
            let red = ((1.0 - t) * 255.0) as u32;
            let green = (t * 255.0) as u32;
            s.1 = (s.1 & 0xff00_0000) | (red << 16) | (green << 8);
        }
    }
    let focusing =
        |e: &EntityView| e.class == EntityClass::Player && looking_at(e, camera.eye) < 5.0;
    let highlight = boolean(settings, &key("highlight-focusing"), false)
        && selected.iter().any(|s| focusing(s.0));
    // The reference starts at the camera plus its look vector, so tracers
    // converge on the screen centre in every perspective. `VIEW_CENTRE` asks
    // the on-top pass, which has the frame's real camera, to fill that in.
    let _ = camera;
    let start = VIEW_CENTRE;
    // Farthest first, so nearer tracers draw on top.
    selected.sort_by(|a, b| b.2.total_cmp(&a.2));
    for (e, color, _) in selected {
        // A player's team colour replaces every other tracer colour.
        let color = people.team_color(e).map_or(color, |c| 0xff00_0000 | c);
        let eye_height = (e.bounds.max[1] - e.bounds.min[1]) * 0.85;
        let end = Point([e.position[0], e.position[1] + eye_height, e.position[2]]);
        let (width, outlined) = if !highlight {
            (1.0, false)
        } else if focusing(e) {
            (3.0, true)
        } else {
            (0.75, false)
        };
        if outlined {
            out.push(Line {
                start,
                end,
                color: 0xff00_0000,
                width: width * 1.5,
            });
        }
        out.push(Line {
            start,
            end,
            color,
            width,
        });
    }
}

/// Degrees between `e`'s view and the direction from it to `target`
/// (`RotationUtil.M`), using the snapshot's yaw and pitch when present.
fn looking_at(e: &EntityView, target: [f64; 3]) -> f64 {
    let Some((yaw, pitch)) = e.look else {
        return f64::INFINITY;
    };
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let view = [
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    ];
    let eye_height = (e.bounds.max[1] - e.bounds.min[1]) * 0.85;
    let from = [e.position[0], e.position[1] + eye_height, e.position[2]];
    let to = [0, 1, 2].map(|i| target[i] - from[i]);
    let length = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
    if length < 1e-6 {
        return 0.0;
    }
    let dot = (0..3).map(|i| view[i] * to[i] / length).sum::<f64>();
    dot.clamp(-1.0, 1.0).acos().to_degrees()
}

#[cfg(test)]
mod tests {
    use super::super::super::super::targeting::filter::NameLists;
    use super::super::super::super::targeting::filter::tests::entity;
    use super::*;

    fn settings() -> Settings {
        let mut s = Settings::default();
        for d in definitions().into_iter().chain(people_definitions()) {
            s.define(d).unwrap();
        }
        s
    }

    /// `Tracers.collectEntities`' entries (`RenderEntityContextEntry`: an
    /// entity with its colour): players by default; mobs and animals once
    /// turned on, each in its colour; friends and enemies recoloured; all
    /// from the frame's view centre to the eye height.
    #[test]
    fn tracer_entries_take_their_group_or_person_colour() {
        let mut s = settings();
        let camera = Camera {
            eye: [0.0, 1.62, 0.0],
            look: [0.0, 0.0, 1.0],
            yaw: 0.0,
            pitch: 0.0,
        };
        let entities = [
            entity(EntityClass::Player, "Stranger", [0.0, 0.0, 4.0]),
            entity(EntityClass::Player, "Pal", [1.0, 0.0, 6.0]),
            entity(EntityClass::Player, "Rival", [2.0, 0.0, 8.0]),
            entity(EntityClass::Monster, "", [3.0, 0.0, 10.0]),
            entity(EntityClass::Peaceful, "", [4.0, 0.0, 12.0]),
        ];
        let people = NameLists {
            friends: vec!["pal".into()],
            enemies: vec!["rival".into()],
        };
        let traced = |s: &Settings| {
            let mut out = Vec::new();
            tracers(s, &camera, &entities, &people, &mut out);
            assert!(out.iter().all(|l| l.start.0.iter().all(|v| v.is_nan())));
            // Farthest first; the end is at 0.85 of the box's height.
            out.iter().map(|l| (l.end.0, l.color)).collect::<Vec<_>>()
        };
        let eye = |x: f64, z: f64| [x, 1.8 * 0.85, z];
        assert_eq!(
            traced(&s),
            [
                (eye(2.0, 8.0), 0xfff4_4242),
                (eye(1.0, 6.0), 0xff42_f489),
                (eye(0.0, 4.0), 0xff00_96ff),
            ]
        );
        s.set(&format!("{TRACERS}.mobs"), json!(true)).unwrap();
        s.set(&format!("{TRACERS}.animals"), json!(true)).unwrap();
        s.set("friends.recolor", json!(false)).unwrap();
        let all = traced(&s);
        assert_eq!(all[0], (eye(4.0, 12.0), 0xffff_ffff));
        assert_eq!(all[1], (eye(3.0, 10.0), 0xffff_9a00));
        assert_eq!(all[3], (eye(1.0, 6.0), 0xff00_96ff));
        // "Enemy Only" drops friends; a distance range drops mobs outside it.
        s.set(&format!("{TRACERS}.enemy-only"), json!(true))
            .unwrap();
        s.set(&format!("{TRACERS}.mob-distance-check"), json!(true))
            .unwrap();
        s.set(&format!("{TRACERS}.mob-distance"), json!([0.0, 8.0]))
            .unwrap();
        let ends: Vec<_> = traced(&s).into_iter().map(|(end, _)| end).collect();
        assert_eq!(ends, [eye(4.0, 12.0), eye(2.0, 8.0), eye(0.0, 4.0)]);
    }
}
