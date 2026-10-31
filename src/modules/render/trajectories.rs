// Trajectories: the predicted flight of the held bow, throwable or trident,
// drawn through walls, stepped as `Trajectories.onRender3D` does (see
// `lines`) and ended with the reference's impact cross. Launch
// speeds and gravity are the modern items' (`BowItem`, `SnowballItem`,
// `ThrowablePotionItem`, `ExperienceBottleItem`, `TridentItem`,
// `CrossbowItem`), and the shooter's known movement is added as
// `Projectile.shootFromRotation` does (its vertical part only off the ground).
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::targeting::entities::EntityView;
use super::world::{Line, Point};
use serde_json::{Value, json};

pub const ID: &str = "render.trajectories";

/// The host's block clip for one segment: the first hit point and the hit
/// face's outward normal, if any.
pub type BlockClip<'a> = &'a mut dyn FnMut([f64; 3], [f64; 3]) -> Option<([f64; 3], [i32; 3])>;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    vec![
        Definition::new(
            &key("aiming-color"),
            "Aiming Color",
            Kind::Color,
            json!(0xffff_ffff_u32),
        ),
        Definition::boolean(&key("ghost-bow-charge"), "Ghost Bow Charge", false),
        Definition::new(
            &key("target-color"),
            "Target Color",
            Kind::Color,
            json!(0xff00_00ff_u32),
        ),
        Definition::new(
            &key("trajectory-color"),
            "Trajectory Color",
            Kind::Color,
            json!(0xffff_0000_u32),
        ),
    ]
}

/// How a held item launches: speed, gravity and pitch offset in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub speed: f64,
    pub gravity: f64,
    pub pitch_offset: f32,
    /// The per-tick velocity factor (0.99 in air; 1.0 for a firework).
    pub drag: f64,
    /// Ticks simulated (a firework's flight is short-lived).
    pub steps: usize,
    /// The shooter's movement added to the launch velocity.
    pub shooter: [f64; 3],
}

/// What a held crossbow is loaded with (`charged_projectiles`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Loaded {
    #[default]
    Empty,
    Arrow,
    Firework,
}

impl Launch {
    /// Adds the shooter's known movement, its vertical part only while
    /// airborne (`Projectile.shootFromRotation`).
    pub fn with_shooter(mut self, movement: [f64; 3], on_ground: bool) -> Self {
        self.shooter = [
            movement[0],
            if on_ground { 0.0 } else { movement[1] },
            movement[2],
        ];
        self
    }
}

/// `BowItem.getPowerForTime`.
fn bow_power(ticks: u32) -> f64 {
    let t = f64::from(ticks) / 20.0;
    ((t * t + t * 2.0) / 3.0).min(1.0)
}

/// The launch for `item` used for `use_ticks` (None when not in use). A
/// bow needs at least 0.1 power, as `BowItem.releaseUsing`; "Ghost Bow
/// Charge" shows a full-power shot while the bow is not drawn.
/// A crossbow shoots only when loaded: an arrow at 3.15, a firework at
/// 1.6 in a straight line (`CrossbowItem.getShootingPower`,
/// `FireworkRocketEntity` shot at an angle); "Ghost Bow Charge" shows an
/// unloaded crossbow's arrow, as the reference does.
pub fn launch(
    item: &str,
    use_ticks: Option<u32>,
    ghost_bow: bool,
    loaded: Loaded,
) -> Option<Launch> {
    let item = item.trim_start_matches("minecraft:");
    let throw = |speed, gravity, pitch_offset| Launch {
        speed,
        gravity,
        pitch_offset,
        drag: f64::from(0.99_f32),
        steps: 300,
        shooter: [0.0; 3],
    };
    Some(match item {
        "bow" => {
            let power = match use_ticks {
                Some(ticks) => bow_power(ticks),
                None if ghost_bow => 1.0,
                None => return None,
            };
            if power < 0.1 {
                return None;
            }
            throw(power * 3.0, 0.05, 0.0)
        }
        "crossbow" => match loaded {
            Loaded::Arrow => throw(3.15, 0.05, 0.0),
            Loaded::Firework => Launch {
                drag: 1.0,
                steps: 40,
                ..throw(1.6, 0.0, 0.0)
            },
            Loaded::Empty if ghost_bow => throw(3.15, 0.05, 0.0),
            Loaded::Empty => return None,
        },
        "snowball" | "egg" | "ender_pearl" => throw(1.5, 0.03, 0.0),
        "splash_potion" | "lingering_potion" => throw(0.5, 0.05, -20.0),
        "experience_bottle" => throw(0.7, 0.07, -20.0),
        "trident" if use_ticks.is_some_and(|t| t >= 10) => throw(2.5, 0.05, 0.0),
        _ => return None,
    })
}

/// Where the segment `a`→`b` first enters `e`'s box grown by 0.3
/// (`ProjectileUtil.getEntityHitResult`), as a fraction of the segment.
fn entity_hit(a: [f64; 3], b: [f64; 3], e: &EntityView) -> Option<f64> {
    let (mut near, mut far) = (0.0_f64, 1.0_f64);
    for i in 0..3 {
        let (lo, hi) = (e.bounds.min[i] - 0.3, e.bounds.max[i] + 0.3);
        let d = b[i] - a[i];
        if d.abs() < 1e-12 {
            if a[i] < lo || a[i] > hi {
                return None;
            }
            continue;
        }
        let (t0, t1) = ((lo - a[i]) / d, (hi - a[i]) / d);
        near = near.max(t0.min(t1));
        far = far.min(t0.max(t1));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// The axis of the face of `e`'s grown box nearest `hit` (x, then z, then y
/// on a tie, as the reference picks the cross's rotation).
fn entity_face(hit: [f64; 3], e: &EntityView) -> usize {
    let d = |i: usize| {
        (hit[i] - (e.bounds.min[i] - 0.3))
            .abs()
            .min((hit[i] - (e.bounds.max[i] + 0.3)).abs())
    };
    let (x, y, z) = (d(0), d(1), d(2));
    let nearest = y.min(x.min(z));
    if nearest == x {
        0
    } else if nearest == z {
        2
    } else {
        1
    }
}

/// The reference's impact marker (`drawImpactMarker`): four 0.25 arms from
/// `at`, at width 1, in the plane across `axis`.
fn cross(out: &mut Vec<Line>, at: [f64; 3], axis: usize, color: u32) {
    for i in (0..3).filter(|&i| i != axis) {
        for arm in [-0.25, 0.25] {
            let mut end = at;
            end[i] += arm;
            out.push(Line {
                start: Point(end),
                end: Point(at),
                color,
                width: 1.0,
            });
        }
    }
}

/// The arc from `eye` along `yaw`/`pitch`, as `Trajectories.onRender3D`
/// steps it: each tick's segment is clipped by `block` (the host's block
/// clip, with the hit face's normal) and then by the nearest entity, and the
/// projectile moves its full step either way, so the path and the cross end
/// one step past the impact (`TrajectoriesProjectileRenderState` segments
/// join the points after each step). The path takes the trajectory colour
/// while it ends on an entity and the aiming colour otherwise; the cross,
/// turned across the face it hit, takes the target colour on an entity and
/// the aiming colour on a block.
#[allow(clippy::too_many_arguments)]
pub fn lines(
    settings: &Settings,
    launch: Launch,
    eye: [f64; 3],
    yaw: f32,
    pitch: f32,
    entities: &[EntityView],
    mut block: impl FnMut([f64; 3], [f64; 3]) -> Option<([f64; 3], [i32; 3])>,
    out: &mut Vec<Line>,
) {
    let color = |k: &str, d: u32| {
        settings
            .get(&format!("{ID}.{k}"))
            .and_then(Value::as_u64)
            .map_or(d, |c| c as u32)
    };
    let (y, p) = (
        f64::from(yaw).to_radians(),
        f64::from(pitch + launch.pitch_offset).to_radians(),
    );
    let direction = [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()];
    let mut at = [eye[0], eye[1] - 0.1, eye[2]];
    let mut motion: [f64; 3] =
        std::array::from_fn(|i| direction[i] * launch.speed + launch.shooter[i]);
    if !at.iter().chain(&motion).all(|v| v.is_finite()) {
        return;
    }
    let distance = super::super::super::targeting::entities::distance;
    let mut points = Vec::new();
    // The impact's point, the axis the cross lies across, and whether it is
    // on an entity.
    let mut hit: Option<([f64; 3], usize, bool)> = None;
    for _ in 0..launch.steps.min(4096) {
        let end: [f64; 3] = std::array::from_fn(|i| at[i] + motion[i]);
        if !end.iter().all(|v| v.is_finite()) {
            break;
        }
        let mut limit = 1.0;
        if let Some((point, face)) = block(at, end) {
            let full = distance(at, end);
            limit = if full < 1e-9 {
                0.0
            } else {
                distance(at, point) / full
            };
            let axis = face.iter().position(|&n| n != 0).unwrap_or(1);
            hit = Some((point, axis, false));
        }
        let nearest = entities
            .iter()
            .filter(|e| e.alive())
            .filter_map(|e| entity_hit(at, end, e).map(|t| (t, e)))
            .filter(|(t, _)| *t <= limit)
            .min_by(|x, y| x.0.total_cmp(&y.0));
        if let Some((t, e)) = nearest {
            let point = std::array::from_fn(|i| at[i] + (end[i] - at[i]) * t);
            hit = Some((point, entity_face(point, e), true));
        }
        at = end;
        points.push(Point(at));
        for v in &mut motion {
            *v *= launch.drag;
        }
        motion[1] -= launch.gravity;
        if hit.is_some() || at[1] < -128.0 {
            break;
        }
    }
    let aiming = color("aiming-color", 0xffff_ffff);
    let path = if matches!(hit, Some((_, _, true))) {
        color("trajectory-color", 0xffff_0000)
    } else {
        aiming
    };
    for pair in points.windows(2) {
        out.push(Line {
            start: pair[0],
            end: pair[1],
            color: path,
            width: 1.5,
        });
    }
    let (axis, marker) = match hit {
        Some((_, axis, true)) => (axis, color("target-color", 0xff00_00ff)),
        Some((_, axis, false)) => (axis, aiming),
        None => (1, path),
    };
    if !points.is_empty() {
        cross(out, at, axis, marker);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targeting::entities::EntityClass;
    use crate::targeting::filter::tests::entity;

    /// A snowball thrown level along +z from an eye at 1.72: its points
    /// after each tick are z 1.5, 2.985 and 4.455.
    fn throw(
        entities: &[EntityView],
        block: impl FnMut([f64; 3], [f64; 3]) -> Option<([f64; 3], [i32; 3])>,
    ) -> Vec<Line> {
        let snowball = launch("snowball", None, false, Loaded::Empty).unwrap();
        let mut out = Vec::new();
        lines(
            &Settings::default(),
            snowball,
            [0.0, 1.72, 0.0],
            0.0,
            0.0,
            entities,
            block,
            &mut out,
        );
        out
    }

    /// The cross's colour, centre and the axes its arms run along.
    fn cross_of(out: &[Line]) -> (u32, [f64; 3], Vec<usize>) {
        let arms: Vec<_> = out.iter().filter(|l| l.width == 1.0).collect();
        assert_eq!(arms.len(), 4, "{out:?}");
        let mut axes: Vec<usize> = arms
            .iter()
            .map(|l| (0..3).find(|&i| l.start.0[i] != l.end.0[i]).unwrap())
            .collect();
        axes.dedup();
        (arms[0].color, arms[0].end.0, axes)
    }

    #[test]
    fn an_entity_hit_turns_the_path_red_and_the_cross_blue_across_its_face() {
        let target = entity(EntityClass::Player, "a", [0.0, 0.0, 5.0]);
        let out = throw(std::slice::from_ref(&target), |_, _| None);
        let path: Vec<_> = out.iter().filter(|l| l.width == 1.5).collect();
        // Points after ticks 1..3; the third step enters the grown box (z
        // 4.4) and still runs its full length.
        assert_eq!(path.len(), 2);
        assert!(path.iter().all(|l| l.color == 0xffff_0000));
        assert!((path[0].start.0[2] - 1.5).abs() < 1e-6);
        let (color, at, axes) = cross_of(&out);
        assert_eq!(color, 0xff00_00ff);
        assert!((at[2] - path[1].end.0[2]).abs() < 1e-12 && at[2] > 4.4);
        // Nearest the box's z face: the cross stands across z.
        assert_eq!(axes, [0, 1]);
    }

    #[test]
    fn a_block_hit_keeps_the_aiming_colour_and_stops_entities_behind_it() {
        let behind = entity(EntityClass::Player, "a", [0.0, 0.0, 5.0]);
        let wall = |a: [f64; 3], b: [f64; 3]| {
            (a[2] < 3.0 && b[2] >= 3.0).then(|| {
                let t = (3.0 - a[2]) / (b[2] - a[2]);
                ([0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t), [0, 0, -1])
            })
        };
        let out = throw(std::slice::from_ref(&behind), wall);
        assert!(
            out.iter()
                .filter(|l| l.width == 1.5)
                .all(|l| l.color == 0xffff_ffff)
        );
        let (color, at, axes) = cross_of(&out);
        // One full step past the wall, across its face.
        assert_eq!(color, 0xffff_ffff);
        assert!(at[2] > 4.4 && axes == [0, 1]);
        // The ground's top face lays the cross flat.
        let ground =
            |a: [f64; 3], b: [f64; 3]| (b[2] > 2.0).then_some(([a[0], 1.5, a[2]], [0, 1, 0]));
        assert_eq!(cross_of(&throw(&[], ground)).2, [0, 2]);
    }

    #[test]
    fn crossbows_shoot_only_when_loaded() {
        assert_eq!(
            launch("minecraft:crossbow", None, false, Loaded::Empty),
            None
        );
        let arrow = launch("minecraft:crossbow", None, false, Loaded::Arrow).unwrap();
        assert!((arrow.speed - 3.15).abs() < 1e-9 && arrow.gravity == 0.05);
        let rocket = launch("crossbow", None, false, Loaded::Firework).unwrap();
        assert!(rocket.gravity == 0.0 && rocket.drag == 1.0 && rocket.steps == 40);
        assert!(launch("crossbow", None, true, Loaded::Empty).is_some());
    }

    #[test]
    fn shooter_motion_drops_vertical_on_ground() {
        let l = launch("snowball", None, false, Loaded::Empty).unwrap();
        assert_eq!(
            l.with_shooter([0.2, -0.08, 0.1], true).shooter,
            [0.2, 0.0, 0.1]
        );
        assert_eq!(
            l.with_shooter([0.2, 0.3, 0.1], false).shooter,
            [0.2, 0.3, 0.1]
        );
    }
}
