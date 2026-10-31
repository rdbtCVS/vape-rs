// Show target for KillAura and SilentAura: the reference's target box
// (`RenderUtil.k`, a filled box 0.15 wider than the entity on each side,
// drawn without depth testing) and SilentAura's ring
// (`BufferedRenderPrimitives.drawCylinderSides`: 50 gradient quads around
// the entity, opaque at the feet and clear at the top, depth tested but not
// writing depth, unculled).
use super::super::render::world::{Point, Quad, fill_box};

/// The box's margin around the entity (`RenderUtil.k`'s `0.15`).
pub const BOX_MARGIN: f64 = 0.15;
/// The ring's segment count (`GuiRenderPrimitives.R(..., 50.0f, ...)`).
pub const RING_SEGMENTS: u32 = 50;
/// The ring radius for players and armour stands; others use their width.
pub const PLAYER_RING_RADIUS: f64 = 0.7;

/// A shown target at the frame's interpolated position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    pub id: i32,
    /// The interpolated feet position.
    pub feet: [f64; 3],
    pub width: f64,
    pub height: f64,
    /// A player or armour stand (`EntityPlayer`/`EntityArmorStand`).
    pub player: bool,
}

/// A quad with one ARGB colour per corner (the ring's vertical gradient).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadedQuad {
    pub corners: [Point; 4],
    pub colors: [u32; 4],
}

/// `RenderUtil.k(entity, ..., null, color, partialTicks)`: the fill only.
pub fn target_box(out: &mut Vec<Quad>, s: &Shown, color: u32) {
    let half = s.width / 2.0 + BOX_MARGIN;
    let [x, y, z] = s.feet;
    fill_box(
        out,
        Point([x - half, y, z - half]),
        Point([x + half, y + s.height, z + half]),
        color,
    );
}

/// `drawCylinderSides(x, y, z, 50, radius, height, color)`: each quad runs
/// from the current angle back to the previous one, transparent on top.
pub fn ring(out: &mut Vec<ShadedQuad>, s: &Shown, color: u32) {
    let radius = if s.player {
        PLAYER_RING_RADIUS
    } else {
        s.width
    };
    let [x, y, z] = s.feet;
    let (bottom, top) = (y, y + s.height);
    let clear = color & 0x00ff_ffff;
    let at = |n: u32| {
        let angle = std::f64::consts::TAU * f64::from(n) / f64::from(RING_SEGMENTS);
        (x + radius * angle.cos(), z + radius * angle.sin())
    };
    let mut previous = at(0);
    for n in 1..=RING_SEGMENTS {
        let current = at(n);
        out.push(ShadedQuad {
            corners: [
                Point([current.0, top, current.1]),
                Point([previous.0, top, previous.1]),
                Point([previous.0, bottom, previous.1]),
                Point([current.0, bottom, current.1]),
            ],
            colors: [clear, clear, color, color],
        });
        previous = current;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_is_the_entity_widened_by_the_margin() {
        let mut out = Vec::new();
        let s = Shown {
            id: 1,
            feet: [10.0, 64.0, -5.0],
            width: 0.6,
            height: 1.95,
            player: false,
        };
        target_box(&mut out, &s, 0x64ff_0000);
        assert_eq!(out.len(), 6);
        let xs: Vec<f64> = out
            .iter()
            .flat_map(|q| q.corners.iter().map(|c| c.0[0]))
            .collect();
        let min = xs.iter().copied().fold(f64::MAX, f64::min);
        let max = xs.iter().copied().fold(f64::MIN, f64::max);
        assert!((min - 9.55).abs() < 1e-9 && (max - 10.45).abs() < 1e-9);
        assert!(out.iter().all(|q| q.color == 0x64ff_0000));
    }

    #[test]
    fn the_ring_fades_upwards_and_uses_the_player_radius() {
        let mut out = Vec::new();
        let s = Shown {
            id: 2,
            feet: [0.0, 70.0, 0.0],
            width: 0.6,
            height: 1.8,
            player: true,
        };
        ring(&mut out, &s, 0x32ff_c870);
        assert_eq!(out.len(), RING_SEGMENTS as usize);
        for q in &out {
            assert_eq!(
                q.colors,
                [0x00ff_c870, 0x00ff_c870, 0x32ff_c870, 0x32ff_c870]
            );
            for c in &q.corners {
                let r = (c.0[0].powi(2) + c.0[2].powi(2)).sqrt();
                assert!((r - 0.7).abs() < 1e-5, "{r}");
            }
            assert!((q.corners[0].0[1] - 71.8).abs() < 1e-5);
            assert_eq!(q.corners[2].0[1], 70.0);
        }
        // A mob's ring is as wide as the mob.
        out.clear();
        ring(
            &mut out,
            &Shown {
                player: false,
                width: 1.4,
                ..s
            },
            1,
        );
        let c = out[0].corners[3].0;
        assert!(((c[0].powi(2) + c[2].powi(2)).sqrt() - 1.4).abs() < 1e-5);
    }
}
