// Trajectories: the predicted flight of the held bow, throwable or trident,
// drawn through walls with the shared `world::predict` integrator. Launch
// speeds and gravity are the modern items' (`BowItem`, `SnowballItem`,
// `ThrowablePotionItem`, `ExperienceBottleItem`, `TridentItem`,
// `CrossbowItem`), and the shooter's known movement is added as
// `Projectile.shootFromRotation` does (its vertical part only off the ground).
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::targeting::entities::EntityView;
use super::world::{Impact, ImpactKind, Line, Point, outline_box, predict_with_drag};
use serde_json::{Value, json};

pub const ID: &str = "render.trajectories";

/// The host's block clip for one segment: the first hit point, if any.
pub type BlockClip<'a> = &'a mut dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>;

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

/// The arc from `eye` along `yaw`/`pitch`, stopped by `block` (the host's
/// block clip for a segment) or the nearest entity, plus a box at the
/// impact. The path takes the aiming colour while it ends on an entity,
/// and that entity's box the target colour.
#[allow(clippy::too_many_arguments)]
pub fn lines(
    settings: &Settings,
    launch: Launch,
    eye: [f64; 3],
    yaw: f32,
    pitch: f32,
    entities: &[EntityView],
    mut block: impl FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>,
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
    let start = Point([eye[0], eye[1] - 0.1, eye[2]]);
    let velocity: [f64; 3] =
        std::array::from_fn(|i| direction[i] * launch.speed + launch.shooter[i]);
    let mut struck = None;
    let prediction = predict_with_drag(
        start,
        velocity,
        launch.gravity,
        launch.drag,
        launch.steps,
        |a, b| {
            let block_hit = block(a.0, b.0);
            let limit = block_hit.map_or(1.0, |hit| {
                let full = super::super::super::targeting::entities::distance(a.0, b.0);
                if full < 1e-9 {
                    0.0
                } else {
                    super::super::super::targeting::entities::distance(a.0, hit) / full
                }
            });
            let nearest = entities
                .iter()
                .filter(|e| e.alive())
                .filter_map(|e| entity_hit(a.0, b.0, e).map(|t| (t, e)))
                .filter(|(t, _)| *t <= limit)
                .min_by(|x, y| x.0.total_cmp(&y.0));
            if let Some((t, e)) = nearest {
                struck = Some(e.bounds);
                let at = [0, 1, 2].map(|i| a.0[i] + (b.0[i] - a.0[i]) * t);
                return Some(Impact {
                    position: Point(at),
                    kind: ImpactKind::Entity,
                });
            }
            block_hit.map(|at| Impact {
                position: Point(at),
                kind: ImpactKind::Block,
            })
        },
    );
    let on_entity = matches!(
        prediction.impact,
        Some(Impact {
            kind: ImpactKind::Entity,
            ..
        })
    );
    let path = if on_entity {
        color("aiming-color", 0xffff_ffff)
    } else {
        color("trajectory-color", 0xffff_0000)
    };
    // The first point sits at the eye; skip it so the arc does not cross
    // the view.
    for pair in prediction.points.windows(2).skip(1) {
        out.push(Line {
            start: pair[0],
            end: pair[1],
            color: path,
            width: 1.5,
        });
    }
    if let Some(bounds) = struck {
        outline_box(
            out,
            Point(bounds.min),
            Point(bounds.max),
            color("target-color", 0xff00_00ff),
            1.5,
        );
    } else if let Some(impact) = prediction.impact {
        let [x, y, z] = impact.position.0;
        outline_box(
            out,
            Point([x - 0.15, y - 0.15, z - 0.15]),
            Point([x + 0.15, y + 0.15, z + 0.15]),
            path,
            1.5,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
