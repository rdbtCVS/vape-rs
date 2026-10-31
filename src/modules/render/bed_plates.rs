// BedPlates (`BedPlates`, with `BedPlateCountState` and
// `BedPlateBlockStateKey`): over each bed within 100 blocks (sideways) a
// plate, drawn through walls at a constant size, of the blocks defending it
// layer by layer, most common first, and optionally its distance.
//
// Every half second each bed's head is surveyed in three shells of radius
// 1..3 stretched over the foot: a block on the shell's sides (or its top,
// `offY == radius`), from the bed's level up to `radius` above, is in layer
// `radius`, or `radius + 1` when it is further than `radius` (sideways
// distance plus height) from both bed blocks; layer 4 is dropped. Each layer
// counts blocks by their clone item, sorted by count. The plate shows the
// distinct items of layers 1, 2 and 3, stopping at the first layer with no
// item, 18 units apart on a 22-unit black panel (8 more with the distance
// above), anchored a block above the head's centre and scaled 0.003 x
// distance per unit, so it stays the same size on screen.
//
// Differences: beds are found by a scan of the loaded blocks round the
// player once a second (the reference's per-version scans), and the plate is
// a square-cornered panel (the reference's rounded composite) with the
// distance in the host font at 0.7 (the reference's smooth font at 1.2)
// so it fits its 8-unit band. Ties in a layer keep the order the survey met
// them (the reference's `HashMap` order).
use super::super::super::core::settings::{Definition, Settings};
use super::overlay::{Align, Draw, Frame, Item};
use serde_json::Value;

pub const ID: &str = "render.bed-plates";
/// How far a bed may be, sideways, to keep its plate.
pub const RANGE: f64 = 100.0;
/// The panel's fill (the reference's `Color(0, 0, 0, 150)`).
pub const PANEL: u32 = 0x9600_0000;

pub fn definitions() -> Vec<Definition> {
    vec![Definition::boolean(
        &format!("{ID}.show-distance"),
        "Show distance",
        true,
    )]
}

/// The way a bed's head faces (from its foot): the foot is behind it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    North,
    South,
    West,
    East,
}

impl Facing {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "north" => Self::North,
            "south" => Self::South,
            "west" => Self::West,
            "east" => Self::East,
            _ => return None,
        })
    }
    /// From the head to the foot.
    fn foot(self) -> (i32, i32) {
        match self {
            Self::North => (0, 1),
            Self::South => (0, -1),
            Self::West => (1, 0),
            Self::East => (-1, 0),
        }
    }
}

/// One layer's items with their counts, most common first
/// (`BedPlateCountState.getSortedLayer`).
pub type Layer = Vec<(String, u32)>;

/// `updateCountStates` for one bed: layers 1, 2 and 3 round the head at
/// `head`, `item` being the clone item of a block (`None` for air or a
/// block without one).
pub fn survey(
    head: [i32; 3],
    facing: Facing,
    item: &mut dyn FnMut([i32; 3]) -> Option<String>,
) -> [Layer; 3] {
    let mut layers: [Layer; 3] = Default::default();
    let (fx, fz) = facing.foot();
    for radius in 1..4 {
        let (mut min_x, mut max_x, mut min_z, mut max_z) = (-radius, radius, -radius, radius);
        if fx > 0 {
            max_x += 1;
        } else if fx < 0 {
            min_x -= 1;
        }
        if fz > 0 {
            max_z += 1;
        } else if fz < 0 {
            min_z -= 1;
        }
        for dy in 0..=radius {
            for dx in min_x..=max_x {
                for dz in min_z..=max_z {
                    let side = dx == min_x || dx == max_x || dz == min_z || dz == max_z;
                    if !side && dy != radius {
                        continue;
                    }
                    let from_head = f64::from(dx).hypot(f64::from(dz)) + f64::from(dy);
                    let from_foot = f64::from(dx - fx).hypot(f64::from(dz - fz)) + f64::from(dy);
                    let r = f64::from(radius);
                    let layer = if from_head > r && from_foot > r {
                        radius + 1
                    } else {
                        radius
                    };
                    if layer >= 4 {
                        continue;
                    }
                    let Some(id) = item([head[0] + dx, head[1] + dy, head[2] + dz]) else {
                        continue;
                    };
                    let counts = &mut layers[layer as usize - 1];
                    match counts.iter_mut().find(|(i, _)| *i == id) {
                        Some((_, n)) => *n += 1,
                        None => counts.push((id, 1)),
                    }
                }
            }
        }
    }
    // `sortLayersByFrequency` (a stable sort).
    for layer in &mut layers {
        layer.sort_by(|a, b| b.1.cmp(&a.1));
    }
    layers
}

/// The plate's items: each layer's in turn, without repeats, up to the first
/// layer with none.
pub fn shown(layers: &[Layer; 3]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for layer in layers {
        if layer.is_empty() {
            break;
        }
        for (id, _) in layer {
            if !out.contains(id) {
                out.push(id.clone());
            }
        }
    }
    out
}

/// A bed's plate this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Plate {
    pub head: [i32; 3],
    pub items: Vec<String>,
}

/// The host font's width of `text` at scale 1 (digits and `m` are 5 wide
/// with a 1 unit gap).
fn text_width(text: &str) -> f32 {
    (text.chars().count() * 6) as f32 - 1.0
}

pub fn draws(settings: &Settings, frame: &Frame, plates: &[Plate]) -> Vec<Draw> {
    let show_distance = settings
        .get(&format!("{ID}.show-distance"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let p = &frame.projection;
    let o = frame.own.position;
    let mut out = Vec::new();
    for plate in plates {
        let h = plate.head.map(f64::from);
        let base = [h[0] + 0.5, h[1], h[2] + 0.5];
        let distance = (0..3).map(|i| (base[i] - o[i]).powi(2)).sum::<f64>().sqrt();
        let anchor = [base[0], base[1] + 1.0, base[2]];
        let (Some(at), Some(per_block)) = (p.project(anchor), p.units_per_block(anchor)) else {
            continue;
        };
        let s = (0.016_666_668 * (distance / 5.0 * 0.3 * 3.0)) as f32 * per_block;
        let label = format!("{}m", distance as i64);
        let mut w = (plate.items.len().max(1) * 18 + 6) as f32;
        let mut ph = 22.0;
        let mut py = -ph;
        if show_distance {
            w = w.max(text_width(&label) * 0.7 + 10.0);
            py -= 8.0;
            ph += 8.0;
        }
        let px = -w / 2.0;
        let point = |x: f32, y: f32| [at[0] + x * s, at[1] + y * s];
        let [x0, y0] = point(px, py - 1.0);
        out.push(Draw::fill(x0, y0, w * s, (ph + 1.0) * s, PANEL));
        for (i, id) in plate.items.iter().enumerate() {
            let [x, y] = point(px + 4.0 + 18.0 * i as f32, py + ph - 20.0);
            out.push(Draw::Item {
                x,
                y,
                scale: s,
                item: Item {
                    id: id.clone(),
                    count: 1,
                    ..Item::default()
                },
            });
        }
        if show_distance {
            let [x, y] = point(0.0, py + 2.0);
            out.push(Draw::Text {
                x,
                y,
                scale: 0.7 * s,
                text: label,
                color: 0xffff_ffff,
                shadow: false,
                align: Align::Center,
                backdrop: None,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::overlay::{Own, Projection};
    use super::*;
    use serde_json::json;

    /// A bed facing north with its head at the origin, its foot at z 1, in
    /// a wool shell (radius 1) with an end stone (radius 2) and obsidian
    /// (radius 3) cover: every block is the material of its shell.
    fn defended(at: [i32; 3]) -> Option<String> {
        let [x, y, z] = at;
        let outside = |r: i32| x < -r || x > r || z < -r || z > r + 1 || y > r;
        let id = if !outside(0) && y == 0 {
            return None;
        } else if !outside(1) {
            "minecraft:white_wool"
        } else if !outside(2) {
            "minecraft:end_stone"
        } else {
            "minecraft:obsidian"
        };
        Some(id.to_owned())
    }

    #[test]
    fn the_survey_splits_shells_into_layers_by_distance_from_both_halves() {
        let layers = survey([0, 0, 0], Facing::North, &mut defended);
        let layer = |l: &[(&str, u32)]| -> Layer {
            l.iter().map(|(i, n)| (format!("minecraft:{i}"), *n)).collect()
        };
        // Radius 1 (10 sides at the bed's level, its 3 x 4 top): a block
        // more than 1 from both halves (the corners, most of the top) is in
        // layer 2, and so outnumbers radius 2's 8 nearest blocks there; the
        // rest of radius 2 is in layer 3 with radius 3's nearest 8.
        assert_eq!(layers[0], layer(&[("white_wool", 8)]));
        assert_eq!(layers[1], layer(&[("white_wool", 14), ("end_stone", 8)]));
        assert_eq!(layers[2], layer(&[("end_stone", 58), ("obsidian", 8)]));
        // The head and foot are not in any layer.
        let mut seen = Vec::new();
        survey([0, 0, 0], Facing::North, &mut |p| {
            seen.push(p);
            None
        });
        assert!(!seen.contains(&[0, 0, 0]) && !seen.contains(&[0, 0, 1]));
        // Facing east, the shells stretch one further west.
        let mut seen = Vec::new();
        survey([5, 0, 5], Facing::East, &mut |p| {
            seen.push(p);
            None
        });
        let xs = seen.iter().map(|p| p[0]);
        assert_eq!((xs.clone().min(), xs.max()), (Some(1), Some(8)));
    }

    #[test]
    fn shown_items_stop_at_the_first_empty_layer() {
        let wool = || ("minecraft:white_wool".to_owned(), 3);
        let stone = || ("minecraft:end_stone".to_owned(), 5);
        let layers = [vec![wool()], vec![stone(), wool()], vec![]];
        assert_eq!(shown(&layers), ["minecraft:white_wool", "minecraft:end_stone"]);
        let layers = [vec![], vec![stone()], vec![wool()]];
        assert!(shown(&layers).is_empty());
    }

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
                camera: [0.5, 1.0, 0.5],
                matrix: m,
                width: 400.0,
                height: 200.0,
            },
            entities: Vec::new(),
            own: Own {
                position: [0.5, 0.0, 0.5],
                ..Own::default()
            },
            first_person: true,
            now_ms: 0,
            spawners: Vec::new(),
            indicators: Vec::new(),
            bed_plates: Vec::new(),
        }
    }

    #[test]
    fn a_plate_keeps_its_size_and_lays_out_the_reference_panel() {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        let f = frame();
        let plate = |z: i32| Plate {
            head: [0, 0, z],
            items: vec!["minecraft:white_wool".into(), "minecraft:end_stone".into()],
        };
        let size = |out: &[Draw]| match out[0] {
            Draw::Fill { w, h, .. } => (w, h),
            _ => panic!("{out:?}"),
        };
        let near = draws(&s, &f, &[plate(-10)]);
        let far = draws(&s, &f, &[plate(-40)]);
        assert_eq!(near.len(), 4);
        let ((w0, h0), (w1, h1)) = (size(&near), size(&far));
        // 2 x 18 + 6 wide, 22 + 8 + 1 high, the same on screen at any range.
        assert!((w0 / h0 - 42.0 / 31.0).abs() < 1e-3);
        assert!((w0 - w1).abs() / w0 < 0.01 && (h0 - h1).abs() / h0 < 0.01);
        let label = near.iter().find_map(|d| match d {
            Draw::Text { text, align, .. } => Some((text.clone(), *align)),
            _ => None,
        });
        assert_eq!(label, Some(("10m".to_owned(), Align::Center)));
        // Without the distance, the panel is 22 + 1 high and has no label.
        let mut s = s.clone();
        s.set(&format!("{ID}.show-distance"), json!(false)).unwrap();
        let out = draws(&s, &f, &[plate(-10)]);
        assert_eq!(out.len(), 3);
        let (w, h) = size(&out);
        assert!((w / h - 42.0 / 23.0).abs() < 1e-3);
        // Behind the camera: nothing.
        assert!(draws(&s, &f, &[plate(10)]).is_empty());
    }
}
