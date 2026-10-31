// Indicators (`Indicators`): a marker on an ellipse around the screen centre
// pointing at each projectile that is off screen or behind you, with the
// projectile's item turned to face it and optionally its distance.
//
// Each projectile is simulated toward you: thrown ones (arrows, pearls,
// potions, eggs, snowballs) until they hit a block or you or fall out of the
// world, fireballs 100 straight steps. Only you are a target (your box grown
// by 0.3, behind the reference's broad phase of the projectile's box swept
// and grown by 1). The result is the closest the projectile comes to your
// feet, or 0 when it hits you. "Always" shows every one, "Threat" those
// coming within 5 (thrown) or 12 (fireballs) and fades them with distance,
// "Hit Only" those that hit you. A direct hit (0 for thrown, under 3 for
// fireballs, every one in Hit Only) is drawn 1.5 times larger with a bold
// red distance.
//
// Differences: thrown projectiles move with this host's physics per kind
// (the reference's loop moves everything with drag then gravity 0.05) and
// stop at the world's bottom less 64 or after 1000 steps instead of y -128;
// the simulation runs once a tick rather than every frame (the projectiles
// only move per tick); the radius is a fraction of the HUD's half size, as
// Arrows'. The reference's `exo` arrow image is a Vape asset: the marker is
// a filled triangle of its footprint (12 wide, outward of the point) with a
// text-style drop shadow (a quarter-bright copy one unit down and right).
use super::super::super::core::settings::{Definition, Kind as SettingKind, Settings};
use super::overlay::{Align, Draw, Frame, Item};
use super::projectiles::{self, Flight, Motion, Target};
use serde_json::{Value, json};

pub const ID: &str = "render.indicators";
pub const MODES: [&str; 3] = ["Always", "Threat", "Hit Only"];
/// The steps a fireball is followed for.
pub const FIREBALL_STEPS: usize = 100;

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::choice(&key("alert-type"), "Alert Type", &MODES, "Threat"),
        Definition::new(
            &key("uncommon-projectile-color"),
            "Uncommon Projectile Color",
            SettingKind::Color,
            json!(0xffff_0000_u32),
        ),
        Definition::boolean(&key("show-arrows"), "Show Arrows", true),
        Definition::boolean(&key("show-pearls"), "Show Pearls", false),
        Definition::boolean(&key("show-potions"), "Show Potions", false),
        Definition::boolean(&key("show-eggs"), "Show Eggs", true),
        Definition::boolean(&key("show-snowballs"), "Show Snowballs", true),
        Definition::boolean(&key("show-fireballs"), "Show Fireballs", true),
        Definition::number(&key("radius-scale"), "Radius Scale", 0.0, 0.15, 1.0, 0.05),
        Definition::boolean(&key("show-distance"), "Show Distance", false),
    ]
}

/// What a projectile is to Indicators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Thrown(projectiles::Kind),
    /// `EntityFireball`: this host's hurting projectiles; `large` is
    /// `EntityLargeFireball` (a ghast's).
    Fireball {
        large: bool,
    },
}

impl Kind {
    pub fn from_entity(id: &str) -> Option<Self> {
        projectiles::Kind::from_entity(id)
            .map(Self::Thrown)
            .or_else(|| match id.trim_start_matches("minecraft:") {
                "fireball" => Some(Self::Fireball { large: true }),
                "small_fireball" | "dragon_fireball" | "wither_skull" | "wind_charge"
                | "breeze_wind_charge" => Some(Self::Fireball { large: false }),
                _ => None,
            })
    }
    fn setting(self) -> &'static str {
        use projectiles::Kind::*;
        match self {
            Self::Thrown(Arrow | Trident) => "show-arrows",
            Self::Thrown(Pearl) => "show-pearls",
            Self::Thrown(Potion) => "show-potions",
            Self::Thrown(Egg) => "show-eggs",
            Self::Thrown(Snowball) => "show-snowballs",
            Self::Fireball { .. } => "show-fireballs",
        }
    }
    /// `getIndicatorItem`.
    pub fn item(self) -> &'static str {
        use projectiles::Kind::*;
        match self {
            Self::Thrown(Arrow | Trident) => "minecraft:arrow",
            Self::Thrown(Pearl) => "minecraft:ender_pearl",
            Self::Thrown(Potion) => "minecraft:potion",
            Self::Thrown(Egg) => "minecraft:egg",
            Self::Thrown(Snowball) => "minecraft:snowball",
            Self::Fireball { .. } => "minecraft:fire_charge",
        }
    }
    /// `getIndicatorColor` (ARGB, opaque).
    fn color(self, uncommon: u32) -> u32 {
        match self {
            Self::Thrown(k) => k.color(),
            Self::Fireball { large: true } => 0xffff_6d00,
            Self::Fireball { large: false } => uncommon,
        }
    }
    fn fireball(self) -> bool {
        matches!(self, Self::Fireball { .. })
    }
}

/// One projectile at the tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projectile {
    pub kind: Kind,
    pub position: [f64; 3],
    pub previous: [f64; 3],
    pub velocity: [f64; 3],
    pub in_water: bool,
    pub stuck: bool,
}

impl Projectile {
    fn flight(&self, kind: projectiles::Kind) -> Flight {
        Flight {
            kind,
            position: self.position,
            previous: self.previous,
            velocity: self.velocity,
            in_water: self.in_water,
            stuck: self.stuck,
        }
    }
}

/// The local player: its box and feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Me {
    pub bounds: Target,
    pub feet: [f64; 3],
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

/// Whether the segment `a`→`b` (from a projectile of half width `radius`
/// and `height`, moving `m`) meets the player, by the reference's broad and
/// narrow phases.
fn hits(me: &Me, a: [f64; 3], b: [f64; 3], m: [f64; 3], radius: f64, height: f64) -> bool {
    let own = (
        [a[0] - radius, a[1], a[2] - radius],
        [a[0] + radius, a[1] + height, a[2] + radius],
    );
    let swept = (
        [0, 1, 2].map(|i| own.0[i].min(own.0[i] + m[i]) - 1.0),
        [0, 1, 2].map(|i| own.1[i].max(own.1[i] + m[i]) + 1.0),
    );
    let near = (0..3).all(|i| me.bounds.0[i] < swept.1[i] && me.bounds.1[i] > swept.0[i]);
    near && projectiles::intercept(
        a,
        b,
        me.bounds.0.map(|v| v - 0.3),
        me.bounds.1.map(|v| v + 0.3),
    )
    .is_some()
}

/// `simulateThrownProjectile` and `simulateArrow`: the closest the
/// projectile comes to the player's feet over its simulated flight, or 0
/// when it hits the player.
pub fn closest(
    p: &Projectile,
    me: &Me,
    bottom: f64,
    block: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>,
) -> f64 {
    let mut closest = 1000.0_f64;
    let mut x = p.position;
    let mut m = p.velocity;
    let (motion, (radius, height), steps) = match p.kind {
        Kind::Thrown(k) => {
            let flight = p.flight(k);
            (
                Some(Motion::of(&flight)),
                k.collision(),
                projectiles::MAX_STEPS,
            )
        }
        Kind::Fireball { large } => (
            None,
            if large { (0.5, 1.0) } else { (0.15625, 0.3125) },
            FIREBALL_STEPS,
        ),
    };
    if let Some(motion) = motion
        && !motion.move_first
    {
        motion.force(&mut m);
    }
    for _ in 0..steps {
        let to = [x[0] + m[0], x[1] + m[1], x[2] + m[2]];
        let wall = block(x, to);
        let end = wall.unwrap_or(to);
        let me_hit = hits(me, x, end, m, radius, height);
        x = to;
        closest = closest.min(distance(me.feet, x));
        if me_hit {
            return 0.0;
        }
        if wall.is_some() {
            break;
        }
        if let Some(motion) = motion {
            if x[1] < bottom - 64.0 {
                break;
            }
            motion.force(&mut m);
        }
    }
    closest
}

fn boolean(settings: &Settings, k: &str) -> bool {
    settings
        .get(&key(k))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Whether the settings follow `p` at all (`findProjectileType` and the
/// fireball range): an enabled kind, thrown ones in the air and moving
/// sideways, fireballs within 1000 of `feet`.
pub fn followed(settings: &Settings, p: &Projectile, feet: [f64; 3]) -> bool {
    if !boolean(settings, p.kind.setting()) {
        return false;
    }
    match p.kind {
        Kind::Fireball { .. } => distance(p.position, feet) <= 1000.0,
        Kind::Thrown(k) => p.flight(k).in_air(),
    }
}

/// A followed projectile this frame, simulated at the tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub kind: Kind,
    /// At the partial tick.
    pub position: [f64; 3],
    /// From the player's feet (`getDistanceToEntity`).
    pub distance: f64,
    /// The simulated closest approach (0: hits you).
    pub closest: f64,
}

/// Turns a draw about `pivot` by `degrees` (clockwise, y down).
fn turned(pivot: [f32; 2], degrees: f32, inner: Draw) -> Draw {
    Draw::Turned {
        pivot,
        degrees,
        inner: Box::new(inner),
    }
}

pub fn draws(settings: &Settings, frame: &Frame, shots: &[Shot]) -> Vec<Draw> {
    let mode = settings
        .get(&key("alert-type"))
        .and_then(Value::as_str)
        .unwrap_or("Threat");
    let hit_only = mode == "Hit Only";
    let threat = mode == "Threat";
    let uncommon = settings
        .get(&key("uncommon-projectile-color"))
        .and_then(Value::as_u64)
        .map_or(0xffff_0000, |c| c as u32);
    let radius = settings
        .get(&key("radius-scale"))
        .and_then(Value::as_f64)
        .unwrap_or(0.15) as f32;
    let show_distance = boolean(settings, "show-distance");
    let p = &frame.projection;
    let (cx, cy) = (p.width / 2.0, p.height / 2.0);
    let (rx, ry) = (cx * radius, cy * radius);
    let mut out = Vec::new();
    for shot in shots {
        // `shouldTrackEntity`: Hit Only keeps those that hit you.
        if hit_only && shot.closest != 0.0 {
            continue;
        }
        let (direct, reach) = match (hit_only, shot.kind.fireball()) {
            (true, _) => (true, 0.0),
            (false, false) => (shot.closest == 0.0, 5.0),
            (false, true) => (shot.closest < 3.0, 12.0),
        };
        if threat && shot.closest > reach {
            continue;
        }
        let on_screen = p
            .project(shot.position)
            .is_some_and(|[x, y]| x > 0.0 && y > 0.0 && x < p.width && y < p.height);
        if on_screen {
            continue;
        }
        // Straight behind (or ahead, already on screen), the reference's
        // angle is `atan2(0, 0)` turned by 180: left of centre.
        let [dx, dy] = p.direction(shot.position).unwrap_or([-1.0, 0.0]);
        let r = rx * ry / ((ry * dx).hypot(rx * dy)).max(1e-6);
        let point = [cx + dx * r, cy + dy * r];
        let angle = dy.atan2(dx).to_degrees();
        let fade = if !direct && threat {
            ((shot.distance * 2.0) as u32).min(200)
        } else {
            0
        };
        let scale = if direct { 1.5 } else { 1.0 };
        // The item, turned so its top faces the centre.
        out.push(turned(
            point,
            angle - 90.0,
            Draw::Item {
                x: point[0] - 8.0 * scale,
                y: point[1] - 20.0 * scale,
                scale,
                item: Item {
                    id: shot.kind.item().to_owned(),
                    count: 1,
                    ..Item::default()
                },
            },
        ));
        if show_distance && fade < 200 {
            let text_scale = 0.5 * scale;
            out.push(Draw::Text {
                x: point[0],
                y: point[1] - 4.0 * text_scale,
                scale: text_scale,
                text: format!(
                    "{}{}m",
                    if direct { "\u{a7}l" } else { "" },
                    shot.distance as i64
                ),
                color: if direct {
                    0xffff_0000
                } else {
                    0x00ff_ffff | ((255 - fade) << 24)
                },
                shadow: true,
                align: Align::Center,
                backdrop: None,
            });
        }
        // The marker: outward of the point, the image's 12 wide footprint.
        let rgb = shot.kind.color(uncommon) & 0x00ff_ffff;
        let alpha = (255 - fade) << 24;
        let (ox, oy) = (dx, dy);
        let (nx, ny) = (-dy, dx);
        let at = |along: f32, side: f32, shift: f32| {
            [
                point[0] + (ox * along + nx * side) * scale + shift,
                point[1] + (oy * along + ny * side) * scale + shift,
            ]
        };
        let shadow = ((rgb >> 2) & 0x003f_3f3f) | alpha;
        for (shift, color) in [(1.0, shadow), (0.0, rgb | alpha)] {
            let tip = at(11.0, 0.0, shift);
            out.push(Draw::Quad {
                corners: [tip, tip, at(1.0, -6.0, shift), at(1.0, 6.0, shift)],
                color,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::overlay::{Own, Projection};
    use super::*;

    fn settings() -> Settings {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s
    }

    fn set(s: &mut Settings, k: &str, v: Value) {
        s.set(&key(k), v).unwrap();
    }

    /// A camera at the origin looking down -Z with a 90 degree FOV.
    fn frame() -> Frame {
        let (near, far) = (0.05f32, 100.0f32);
        let mut m = [0.0f32; 16];
        m[0] = 1.0;
        m[5] = 1.0;
        m[10] = (far + near) / (near - far);
        m[11] = -1.0;
        m[14] = 2.0 * far * near / (near - far);
        Frame {
            projection: Projection {
                camera: [0.0; 3],
                matrix: m,
                width: 400.0,
                height: 200.0,
            },
            entities: Vec::new(),
            own: Own::default(),
            first_person: true,
            now_ms: 0,
            spawners: Vec::new(),
            indicators: Vec::new(),
            bed_plates: Vec::new(),
        }
    }

    fn me() -> Me {
        Me {
            bounds: ([-0.3, 0.0, -0.3], [0.3, 1.8, 0.3]),
            feet: [0.0; 3],
        }
    }

    fn no_blocks(_: [f64; 3], _: [f64; 3]) -> Option<[f64; 3]> {
        None
    }

    fn arrow_at(position: [f64; 3], velocity: [f64; 3]) -> Projectile {
        Projectile {
            kind: Kind::Thrown(projectiles::Kind::Arrow),
            position,
            previous: [0, 1, 2].map(|i| position[i] - velocity[i]),
            velocity,
            in_water: false,
            stuck: false,
        }
    }

    #[test]
    fn defaults_and_kinds_follow_the_reference() {
        let s = settings();
        let d = definitions();
        assert_eq!(d.len(), 10);
        assert_eq!(s.get(&key("alert-type")), Some(&json!("Threat")));
        let radius = s.get(&key("radius-scale")).and_then(Value::as_f64);
        assert!(
            radius.is_some_and(|r| (r - 0.15).abs() < 1e-9),
            "{radius:?}"
        );
        let pearl = Projectile {
            kind: Kind::Thrown(projectiles::Kind::Pearl),
            ..arrow_at([5.0, 1.0, 0.0], [-1.0, 0.0, 0.0])
        };
        assert!(!followed(&s, &pearl, [0.0; 3]), "pearls are off by default");
        assert!(followed(
            &s,
            &arrow_at([5.0, 1.0, 0.0], [-1.0, 0.0, 0.0]),
            [0.0; 3]
        ));
        // Straight down or stuck is not in the air; fireballs need not move.
        let mut still = arrow_at([5.0, 1.0, 0.0], [0.0, -1.0, 0.0]);
        assert!(!followed(&s, &still, [0.0; 3]));
        still.kind = Kind::Fireball { large: true };
        assert!(followed(&s, &still, [0.0; 3]));
        assert!(!followed(&s, &still, [2000.0, 0.0, 0.0]));
        assert_eq!(
            Kind::from_entity("minecraft:fireball"),
            Some(Kind::Fireball { large: true })
        );
        assert_eq!(
            Kind::from_entity("minecraft:wither_skull"),
            Some(Kind::Fireball { large: false })
        );
        assert_eq!(
            Kind::Fireball { large: true }.color(0xff00_ff00),
            0xffff_6d00
        );
        assert_eq!(
            Kind::Fireball { large: false }.color(0xff00_ff00),
            0xff00_ff00
        );
        assert_eq!(
            Kind::Fireball { large: false }.item(),
            "minecraft:fire_charge"
        );
    }

    #[test]
    fn an_arrow_at_you_hits_and_one_past_you_comes_close() {
        let toward = arrow_at([10.0, 1.0, 0.0], [-2.0, 0.0, 0.0]);
        assert_eq!(closest(&toward, &me(), -64.0, &mut no_blocks), 0.0);
        // Four blocks to the side: no hit, closest about 4 (at feet height).
        let past = arrow_at([10.0, 1.0, 4.0], [-2.0, 0.0, 0.0]);
        let c = closest(&past, &me(), -64.0, &mut no_blocks);
        assert!(c > 3.9 && c < 4.3, "{c}");
        // A wall between stops it short.
        let mut wall = |a: [f64; 3], b: [f64; 3]| (b[0] < 5.0).then_some([5.0, a[1], a[2]]);
        let c = closest(&toward, &me(), -64.0, &mut wall);
        assert!(c > 0.0 && c < 6.0, "{c}");
    }

    #[test]
    fn fireballs_fly_straight_for_a_hundred_steps() {
        let mut ball = arrow_at([0.0, 1.0, 50.0], [0.0, 0.0, -1.0]);
        ball.kind = Kind::Fireball { large: false };
        assert_eq!(closest(&ball, &me(), -64.0, &mut no_blocks), 0.0);
        // Too slow to arrive: its closest is where it is after 100 steps.
        ball.velocity = [0.0, 0.0, -0.2];
        let c = closest(&ball, &me(), -64.0, &mut no_blocks);
        assert!((c - (30.0_f64.powi(2) + 1.0).sqrt()).abs() < 1e-6, "{c}");
    }

    fn shot(position: [f64; 3], closest: f64) -> Shot {
        Shot {
            kind: Kind::Thrown(projectiles::Kind::Arrow),
            position,
            distance: (0..3)
                .map(|i| position[i] * position[i])
                .sum::<f64>()
                .sqrt(),
            closest,
        }
    }

    #[test]
    fn modes_choose_which_shots_draw() {
        let mut s = settings();
        let f = frame();
        let behind_near = shot([0.0, 0.0, 6.0], 0.0);
        let behind_miss = shot([0.0, 0.0, 6.0], 8.0);
        let ahead = shot([0.0, 0.0, -6.0], 0.0);
        // Threat: within 5 only; on screen never.
        assert!(!draws(&s, &f, &[behind_near]).is_empty());
        assert!(draws(&s, &f, &[behind_miss]).is_empty());
        assert!(draws(&s, &f, &[ahead]).is_empty());
        set(&mut s, "alert-type", json!("Always"));
        assert!(!draws(&s, &f, &[behind_miss]).is_empty());
        set(&mut s, "alert-type", json!("Hit Only"));
        assert!(draws(&s, &f, &[behind_miss]).is_empty());
        assert!(!draws(&s, &f, &[behind_near]).is_empty());
    }

    #[test]
    fn a_marker_sits_on_the_ellipse_with_its_item_turned_and_a_bold_hit_label() {
        let mut s = settings();
        set(&mut s, "show-distance", json!(true));
        let f = frame();
        // Straight behind and to the right: the marker is right of centre.
        let out = draws(&s, &f, &[shot([6.0, 0.0, 0.0], 0.0)]);
        let Some(Draw::Turned {
            pivot,
            degrees,
            inner,
        }) = out.first()
        else {
            panic!("{out:?}");
        };
        // HUD half size 200 x 100 at 0.15: the ellipse is 30 x 15.
        assert!((pivot[0] - 230.0).abs() < 1e-3 && (pivot[1] - 100.0).abs() < 1e-3);
        assert!(degrees.abs() - 90.0 < 1e-3);
        let Draw::Item { scale, item, .. } = &**inner else {
            panic!();
        };
        assert_eq!((*scale, item.id.as_str()), (1.5, "minecraft:arrow"));
        let label = out.iter().find_map(|d| match d {
            Draw::Text { text, color, .. } => Some((text.clone(), *color)),
            _ => None,
        });
        assert_eq!(label, Some(("\u{a7}l6m".to_owned(), 0xffff_0000)));
        // Shadow, then the marker in the arrow's red, tip outward.
        let quads: Vec<_> = out
            .iter()
            .filter_map(|d| match d {
                Draw::Quad { corners, color } => Some((*corners, *color)),
                _ => None,
            })
            .collect();
        assert_eq!(quads.len(), 2);
        assert_eq!(quads[1].1, 0xffff_0000);
        assert!(quads[1].0[0][0] > 230.0 + 15.0);
        // Threat and not a direct hit: faded by distance, label not bold.
        let out = draws(&s, &f, &[shot([30.0, 0.0, 0.0], 4.0)]);
        let label = out.iter().find_map(|d| match d {
            Draw::Text { text, color, .. } => Some((text.clone(), *color)),
            _ => None,
        });
        assert_eq!(label, Some(("30m".to_owned(), 0x00ff_ffff | (195 << 24))));
    }
}
