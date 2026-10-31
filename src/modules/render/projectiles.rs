// Projectiles (`Projectiles`, with `IProjectile`, `Projectile`,
// `ArrowProjectile`, `PotionProjectile` and the `EnderPearlProjectileBridge`
// entity wrapper): each tick, every projectile in the air of an enabled kind
// is simulated forward until it hits a block or a living entity or falls out
// of the world, and its path is drawn through walls in the kind's colour at
// 1.5 px.
//
// The reference's loop: a first vertex at the projectile, then per step a
// ray from `p` to `p + m` (clipped by blocks), the living entities near the
// projectile's own box swept along `m` (grown by 1) tested with their boxes
// grown by 0.3 (closest wins), the move, a stop on a hit or below the world,
// drag and gravity, and a vertex. Differences, so the path is where this
// host's projectile actually goes: each kind's own physics (arrows and
// tridents move, then drag (0.6 in water, 0.99 for a trident) and gravity
// 0.05; pearls, eggs and snowballs (gravity 0.03) and potions (0.05) take
// gravity and drag (0.8 in water) before the move), the stop is the world's
// bottom less 64 instead of y -128, the vertices are each step's end (the
// reference's run one step ahead, `p + m` after the drag) and the last one
// is the hit point rather than the end of the step past it.
use super::super::super::core::settings::{Definition, Settings};
use super::world::{Line, Point};
use serde_json::Value;

pub const ID: &str = "render.projectiles";
/// The reference's line width.
pub const WIDTH: f32 = 1.5;
/// Steps simulated at most (a projectile left hanging still ends).
pub const MAX_STEPS: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Arrow,
    Trident,
    Pearl,
    Potion,
    Egg,
    Snowball,
}

impl Kind {
    /// The entity type this kind tracks, if any.
    pub fn from_entity(id: &str) -> Option<Self> {
        Some(match id.trim_start_matches("minecraft:") {
            "arrow" | "spectral_arrow" => Self::Arrow,
            "trident" => Self::Trident,
            "ender_pearl" => Self::Pearl,
            "splash_potion" | "lingering_potion" | "potion" => Self::Potion,
            "egg" => Self::Egg,
            "snowball" => Self::Snowball,
            _ => return None,
        })
    }
    /// The setting that shows this kind: tridents are arrows
    /// (`AbstractArrow`), as `EntityArrow` held both.
    fn setting(self) -> &'static str {
        match self {
            Self::Arrow | Self::Trident => "show-arrows",
            Self::Pearl => "show-pearls",
            Self::Potion => "show-potions",
            Self::Egg => "show-eggs",
            Self::Snowball => "show-snowballs",
        }
    }
    /// The kind's colour (ARGB).
    pub fn color(self) -> u32 {
        match self {
            Self::Arrow | Self::Trident => 0xffff_0000,
            Self::Pearl => 0xffad_0cff,
            Self::Potion => 0xffff_42f9,
            Self::Egg => 0xffff_ee9a,
            Self::Snowball => 0xffff_ffff,
        }
    }
    /// `getCollisionRadius` and `getCollisionHeight`.
    pub fn collision(self) -> (f64, f64) {
        match self {
            Self::Arrow | Self::Trident => (0.25, 0.5),
            _ => (0.125, 0.25),
        }
    }
    fn gravity(self) -> f64 {
        match self {
            Self::Arrow | Self::Trident | Self::Potion => 0.05,
            Self::Pearl | Self::Egg | Self::Snowball => 0.03,
        }
    }
}

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::boolean(&key("show-arrows"), "Show Arrows", true),
        Definition::boolean(&key("show-pearls"), "Show Pearls", true),
        Definition::boolean(&key("show-potions"), "Show Potions", false),
        Definition::boolean(&key("show-eggs"), "Show Eggs", false),
        Definition::boolean(&key("show-snowballs"), "Show Snowballs", false),
    ]
}

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

/// The kinds the settings show.
pub fn shown(settings: &Settings) -> Vec<Kind> {
    use Kind::*;
    [Arrow, Trident, Pearl, Potion, Egg, Snowball]
        .into_iter()
        .filter(|k| {
            settings
                .get(&key(k.setting()))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .collect()
}

/// One projectile at the tick (`EnderPearlProjectileBridge`'s view).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub kind: Kind,
    pub position: [f64; 3],
    pub previous: [f64; 3],
    pub velocity: [f64; 3],
    pub in_water: bool,
    /// In the ground (`inGround`).
    pub stuck: bool,
}

impl Flight {
    /// `resolveProjectile`'s skip: in the ground or not moving sideways.
    pub fn in_air(&self) -> bool {
        !self.stuck
            && !(self.position[0] == self.previous[0] && self.position[2] == self.previous[2])
    }
}

/// An entity box `(min, max)` a projectile can hit.
pub type Target = ([f64; 3], [f64; 3]);

/// Where the segment `a`→`b` enters `(lo, hi)`, as a fraction of it.
pub fn intercept(a: [f64; 3], b: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> Option<f64> {
    let (mut near, mut far) = (0.0_f64, 1.0_f64);
    for i in 0..3 {
        let d = b[i] - a[i];
        if d.abs() < 1e-12 {
            if a[i] < lo[i] || a[i] > hi[i] {
                return None;
            }
            continue;
        }
        let (t0, t1) = ((lo[i] - a[i]) / d, (hi[i] - a[i]) / d);
        near = near.max(t0.min(t1));
        far = far.min(t0.max(t1));
        if near > far {
            return None;
        }
    }
    Some(near)
}

fn overlaps(a: Target, b: Target) -> bool {
    (0..3).all(|i| a.0[i] < b.1[i] && a.1[i] > b.0[i])
}

/// How a kind moves each tick on this host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Arrows and tridents move before their drag and gravity; thrown
    /// projectiles take them first.
    pub move_first: bool,
    pub drag: f64,
    pub gravity: f64,
}

impl Motion {
    pub fn of(flight: &Flight) -> Self {
        let move_first = matches!(flight.kind, Kind::Arrow | Kind::Trident);
        let drag = match (move_first, flight.in_water) {
            (true, true) if flight.kind == Kind::Trident => 0.99,
            (true, true) => 0.6,
            (false, true) => 0.8,
            (_, false) => 0.99,
        };
        Self {
            move_first,
            drag,
            gravity: flight.kind.gravity(),
        }
    }
    /// The tick's drag and gravity on the motion `m`.
    pub fn force(&self, m: &mut [f64; 3]) {
        if self.move_first {
            *m = m.map(|v| v * self.drag);
            m[1] -= self.gravity;
        } else {
            m[1] -= self.gravity;
            *m = m.map(|v| v * self.drag);
        }
    }
}

/// The simulated path of `flight`, first vertex at the projectile.
/// `block` is the host's block clip of a segment; `bottom` the world's
/// lowest block.
pub fn path(
    flight: &Flight,
    targets: &[Target],
    bottom: f64,
    block: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>,
) -> Vec<[f64; 3]> {
    let (radius, height) = flight.kind.collision();
    let motion = Motion::of(flight);
    let force = |m: &mut [f64; 3]| motion.force(m);
    let move_first = motion.move_first;
    let mut p = flight.position;
    let mut m = flight.velocity;
    let mut out = vec![p];
    if !move_first {
        force(&mut m);
    }
    for _ in 0..MAX_STEPS {
        let to = [p[0] + m[0], p[1] + m[1], p[2] + m[2]];
        let mut end = block(p, to);
        let segment_end = end.unwrap_or(to);
        // The broad phase: the projectile's box swept along the motion, grown
        // by 1.
        let own = (
            [p[0] - radius, p[1], p[2] - radius],
            [p[0] + radius, p[1] + height, p[2] + radius],
        );
        let swept = (
            [0, 1, 2].map(|i| own.0[i].min(own.0[i] + m[i]) - 1.0),
            [0, 1, 2].map(|i| own.1[i].max(own.1[i] + m[i]) + 1.0),
        );
        let mut best: Option<f64> = None;
        for t in targets.iter().filter(|t| overlaps(**t, swept)) {
            let (lo, hi) = (t.0.map(|v| v - 0.3), t.1.map(|v| v + 0.3));
            if let Some(f) = intercept(p, segment_end, lo, hi)
                && best.is_none_or(|b| f < b)
            {
                best = Some(f);
            }
        }
        if let Some(f) = best {
            end = Some([0, 1, 2].map(|i| p[i] + (segment_end[i] - p[i]) * f));
        }
        if let Some(hit) = end {
            out.push(hit);
            break;
        }
        p = to;
        if p[1] < bottom - 64.0 {
            break;
        }
        out.push(p);
        force(&mut m);
    }
    out
}

/// The paths of the shown projectiles as world lines.
pub fn lines(
    settings: &Settings,
    flights: &[Flight],
    targets: &[Target],
    bottom: f64,
    block: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>,
    out: &mut Vec<Line>,
) {
    let kinds = shown(settings);
    for f in flights
        .iter()
        .filter(|f| kinds.contains(&f.kind) && f.in_air())
    {
        let color = f.kind.color();
        let points = path(f, targets, bottom, block);
        out.extend(points.windows(2).map(|w| Line {
            start: Point(w[0]),
            end: Point(w[1]),
            color,
            width: WIDTH,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s
    }

    fn flight(kind: Kind, velocity: [f64; 3]) -> Flight {
        Flight {
            kind,
            position: [0.0, 100.0, 0.0],
            previous: [-velocity[0], 100.0 - velocity[1], -velocity[2]],
            velocity,
            in_water: false,
            stuck: false,
        }
    }

    fn no_blocks(_: [f64; 3], _: [f64; 3]) -> Option<[f64; 3]> {
        None
    }

    #[test]
    fn defaults_show_arrows_and_pearls_only() {
        let s = settings();
        assert_eq!(shown(&s), vec![Kind::Arrow, Kind::Trident, Kind::Pearl]);
        assert_eq!(
            Kind::from_entity("minecraft:ender_pearl"),
            Some(Kind::Pearl)
        );
        assert_eq!(Kind::from_entity("minecraft:experience_bottle"), None);
        assert_eq!(Kind::Pearl.color(), 0xffad_0cff);
        assert_eq!(Kind::Arrow.collision(), (0.25, 0.5));
    }

    #[test]
    fn an_arrow_falls_like_the_hosts_and_stops_at_a_floor() {
        // Move, then drag and gravity.
        let f = flight(Kind::Arrow, [1.0, 0.0, 0.0]);
        let mut floor = |a: [f64; 3], b: [f64; 3]| {
            (b[1] < 90.0).then(|| {
                let t = (a[1] - 90.0) / (a[1] - b[1]);
                [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
            })
        };
        let path = path(&f, &[], -64.0, &mut floor);
        assert_eq!(path[0], [0.0, 100.0, 0.0]);
        // p1 = (1, 100); m1 = (0.99, -0.05); p2 = (1.99, 99.95).
        assert_eq!(path[1], [1.0, 100.0, 0.0]);
        assert!((path[2][0] - 1.99).abs() < 1e-9 && (path[2][1] - 99.95).abs() < 1e-9);
        let last = *path.last().unwrap();
        assert!((last[1] - 90.0).abs() < 1e-9, "{last:?}");
        assert!(path.iter().all(|p| p[1] >= 90.0 - 1e-9));
    }

    #[test]
    fn a_pearl_takes_gravity_before_its_move() {
        let f = flight(Kind::Pearl, [0.0, 0.0, 1.0]);
        let path = path(&f, &[], 60.0, &mut no_blocks);
        // m = (0, -0.03, 1) * 0.99 before the first move.
        assert!((path[1][2] - 0.99).abs() < 1e-9);
        assert!((path[1][1] - (100.0 - 0.0297)).abs() < 1e-9);
        // It ends below the world's bottom less 64.
        assert!(path.last().unwrap()[1] < 60.0 - 64.0 + 5.0);
    }

    #[test]
    fn the_closest_entity_ends_the_path_at_its_grown_box() {
        let f = flight(Kind::Arrow, [1.0, 0.0, 0.0]);
        let near = ([3.0, 99.0, -0.3], [3.6, 100.8, 0.3]);
        let far = ([2.0, 99.0, 5.0], [2.6, 100.8, 5.6]);
        let path = path(&f, &[far, near], -64.0, &mut no_blocks);
        let last = *path.last().unwrap();
        assert!((last[0] - 2.7).abs() < 1e-9, "{last:?}");
        // A box not on the way is not hit.
        let path = super::path(&f, &[far], -64.0, &mut no_blocks);
        assert!(path.len() > 50);
    }

    #[test]
    fn only_shown_kinds_in_the_air_draw() {
        let s = settings();
        let mut arrow = flight(Kind::Arrow, [1.0, 0.0, 0.0]);
        let egg = flight(Kind::Egg, [1.0, 0.0, 0.0]);
        let mut out = Vec::new();
        let mut floor = |_: [f64; 3], b: [f64; 3]| (b[0] > 3.0).then_some(b);
        lines(&s, &[arrow, egg], &[], -64.0, &mut floor, &mut out);
        assert!(!out.is_empty());
        assert!(out.iter().all(|l| l.color == 0xffff_0000 && l.width == 1.5));
        // Stuck, or not moving sideways (straight down): nothing.
        arrow.stuck = true;
        let mut still = flight(Kind::Pearl, [0.0, -1.0, 0.0]);
        still.previous = [0.0, 101.0, 0.0];
        out.clear();
        lines(&s, &[arrow, still], &[], -64.0, &mut floor, &mut out);
        assert!(out.is_empty());
    }
}
