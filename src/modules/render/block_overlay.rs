// Block Overlay (`BlockOverlayHudModule`): the block under the crosshair
// filled with Overlay Color and edged with Outline Color (a fixed 2 px),
// both depth-tested, its bounds grown by 0.002 against z-fighting. The
// bounds are the union of the host's outline boxes for the block (the
// reference's `getShape(...).bounds()`), so a stair or a fence gets its
// whole shape's box, not a unit cube.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::world::{Line, Point, Quad, fill_box, outline_box};
use serde_json::{Value, json};

pub const ID: &str = "hud.block-overlay";
pub const OVERLAY_COLOR: &str = "hud.block-overlay.overlay-color";
pub const OUTLINE_COLOR: &str = "hud.block-overlay.outline-color";
/// The reference's bounds expansion.
pub const EXPAND: f64 = 0.002;
/// The reference's fixed line width.
pub const WIDTH: f32 = 2.0;

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::new(
            OVERLAY_COLOR,
            "Overlay Color",
            Kind::Color,
            json!(0x5fff_0000_u32),
        ),
        Definition::new(
            OUTLINE_COLOR,
            "Outline Color",
            Kind::Color,
            json!(0xc8ff_0000_u32),
        ),
    ]
}

fn color(settings: &Settings, key: &str, default: u32) -> u32 {
    settings
        .get(key)
        .and_then(Value::as_u64)
        .map_or(default, |c| c as u32)
}

/// The drawn bounds `(min, max)` of a block's outline boxes
/// (`[x0, y0, z0, x1, y1, z1]` each), or `None` for an empty shape.
pub fn bounds(boxes: &[[f64; 6]]) -> Option<([f64; 3], [f64; 3])> {
    let first = boxes.first()?;
    let mut min = [first[0], first[1], first[2]];
    let mut max = [first[3], first[4], first[5]];
    for b in &boxes[1..] {
        for i in 0..3 {
            min[i] = min[i].min(b[i]);
            max[i] = max[i].max(b[i + 3]);
        }
    }
    Some((min.map(|v| v - EXPAND), max.map(|v| v + EXPAND)))
}

/// The outline's twelve depth-tested edges.
pub fn lines(settings: &Settings, bounds: ([f64; 3], [f64; 3]), out: &mut Vec<Line>) {
    outline_box(
        out,
        Point(bounds.0),
        Point(bounds.1),
        color(settings, OUTLINE_COLOR, 0xc8ff_0000),
        WIDTH,
    );
}

/// The fill's six depth-tested faces.
pub fn quads(settings: &Settings, bounds: ([f64; 3], [f64; 3]), out: &mut Vec<Quad>) {
    fill_box(
        out,
        Point(bounds.0),
        Point(bounds.1),
        color(settings, OVERLAY_COLOR, 0x5fff_0000),
    );
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

    #[test]
    fn bounds_are_the_union_grown_against_z_fighting() {
        assert_eq!(bounds(&[]), None);
        // A bottom stair: the slab and the back step.
        let stair = [
            [0.0, 0.0, 0.0, 1.0, 0.5, 1.0],
            [0.0, 0.5, 0.5, 1.0, 1.0, 1.0],
        ];
        let (min, max) = bounds(&stair).unwrap();
        assert_eq!(min, [-0.002; 3]);
        assert_eq!(max, [1.002; 3]);
        let (min, max) = bounds(&[[3.0, 64.0, -2.0, 3.5, 64.5, -1.5]]).unwrap();
        assert!((min[1] - 63.998).abs() < 1e-9 && (max[0] - 3.502).abs() < 1e-9);
    }

    #[test]
    fn draws_twelve_edges_and_six_faces_in_the_reference_colours() {
        let s = settings();
        let b = bounds(&[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]).unwrap();
        let (mut l, mut q) = (Vec::new(), Vec::new());
        lines(&s, b, &mut l);
        quads(&s, b, &mut q);
        assert_eq!(l.len(), 12);
        assert!(l.iter().all(|l| l.color == 0xc8ff_0000 && l.width == 2.0));
        assert_eq!(q.len(), 6);
        assert!(q.iter().all(|q| q.color == 0x5fff_0000));
    }
}
