// Explosions (`Explosions`): around each primed TNT, a pulsing filled red
// sphere of the entity damage radius (8) and, with "Blast ring", a yellow
// wire sphere of the block damage radius (4). The pulse is the reference's:
// solid for five ticks, then fading in over five, by the entity's age.
// Both are depth-tested as the reference's (the fill in the name tag pass's
// translucent layer, without depth writes). Difference: the age is
// `80 - fuse`, the age of TNT primed with the default fuse.
use super::super::super::core::settings::{Definition, Settings};
use super::world::{Line, Point, Quad};
use serde_json::Value;

pub const ID: &str = "render.explosions";
const BLOCK_RADIUS: f64 = 4.0;
const ENTITY_RADIUS: f64 = 8.0;

/// One primed TNT this frame: its interpolated position and age in ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tnt {
    pub position: [f64; 3],
    pub age: i32,
}

pub fn definitions() -> Vec<Definition> {
    vec![Definition::boolean(
        &format!("{ID}.blast-ring"),
        "Blast ring",
        true,
    )]
}

/// The pulse alpha (0..=1) at `age` ticks and `partial` into the next.
pub fn pulse(age: i32, partial: f32) -> f32 {
    let progress = ((age.rem_euclid(5)) as f32 - partial + 1.0) / 5.0;
    if (age.div_euclid(5)) % 2 == 0 {
        1.0
    } else {
        progress * progress
    }
}

fn argb(alpha: f32, rgb: u32) -> u32 {
    let a = ((255.0 * alpha) as i32).clamp(0, 255) as u32;
    (a << 24) | (rgb & 0x00ff_ffff)
}

/// A point on a sphere about `c`: `stack` from the bottom pole, `slice`
/// round the Y axis, of `stacks` and `slices` steps.
fn sphere_point(c: [f64; 3], r: f64, stack: u32, stacks: u32, slice: u32, slices: u32) -> Point {
    let polar = std::f64::consts::PI * f64::from(stack) / f64::from(stacks);
    let azimuth = std::f64::consts::TAU * f64::from(slice) / f64::from(slices);
    Point([
        c[0] + r * polar.sin() * azimuth.cos(),
        c[1] - r * polar.cos(),
        c[2] + r * polar.sin() * azimuth.sin(),
    ])
}

/// The blast rings (24 x 24 wire spheres, as `GluSphere` DRAW_LINE).
pub fn lines(settings: &Settings, tnts: &[Tnt], partial: f32, out: &mut Vec<Line>) {
    let ring = settings
        .get(&format!("{ID}.blast-ring"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if !ring {
        return;
    }
    const N: u32 = 24;
    for t in tnts {
        let color = argb(pulse(t.age, partial) * 0.75, 0xffff00);
        let p = |stack, slice| sphere_point(t.position, BLOCK_RADIUS, stack, N, slice, N);
        for stack in 0..=N {
            for slice in 0..N {
                if stack > 0 && stack < N {
                    out.push(Line {
                        start: p(stack, slice),
                        end: p(stack, slice + 1),
                        color,
                        width: 1.0,
                    });
                }
                if stack < N {
                    out.push(Line {
                        start: p(stack, slice),
                        end: p(stack + 1, slice),
                        color,
                        width: 1.0,
                    });
                }
            }
        }
    }
}

/// The entity damage spheres (32 x 32, filled).
pub fn quads(tnts: &[Tnt], partial: f32, out: &mut Vec<Quad>) {
    const N: u32 = 32;
    for t in tnts {
        let color = argb(pulse(t.age, partial) * 0.4, 0xff0000);
        let p = |stack, slice| sphere_point(t.position, ENTITY_RADIUS, stack, N, slice, N);
        for stack in 0..N {
            for slice in 0..N {
                out.push(Quad {
                    corners: [
                        p(stack, slice),
                        p(stack, slice + 1),
                        p(stack + 1, slice + 1),
                        p(stack + 1, slice),
                    ],
                    color,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulses_like_the_reference() {
        assert_eq!(pulse(0, 0.5), 1.0);
        assert_eq!(pulse(4, 0.0), 1.0);
        assert!((pulse(5, 0.0) - 0.04).abs() < 1e-6);
        assert!((pulse(9, 0.0) - 1.0).abs() < 1e-6);
        assert_eq!(pulse(10, 0.3), 1.0);
        let mut quads = Vec::new();
        quads_for_one(&mut quads);
        assert_eq!(quads.len(), 32 * 32);
        let r = quads
            .iter()
            .flat_map(|q| q.corners)
            .map(|c| c.0[0].hypot(c.0[1]).hypot(c.0[2]));
        assert!(r.into_iter().all(|r| (r - 8.0).abs() < 1e-9));
    }

    fn quads_for_one(out: &mut Vec<Quad>) {
        quads(
            &[Tnt {
                position: [0.0; 3],
                age: 0,
            }],
            0.0,
            out,
        );
    }
}
