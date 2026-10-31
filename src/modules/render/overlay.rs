// Screen-space overlays: the frame's world-to-HUD projection and the draw
// commands the host paints under its HUD (ESP 2D, Health, Indicators,
// Arrows, BedPlates, the NameTags equipment row). Colours are ARGB;
// coordinates are HUD units (the host's GUI-scaled pixels).
use super::super::super::targeting::entities::EntityView;

/// The frame's camera and its clip transform for camera-relative points
/// (`projection * bob * view`, column-major), plus the HUD size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    pub camera: [f64; 3],
    pub matrix: [f32; 16],
    pub width: f32,
    pub height: f32,
}

impl Projection {
    /// Clip coordinates of a world point.
    pub fn clip(&self, p: [f64; 3]) -> [f32; 4] {
        let r = [0, 1, 2].map(|i| (p[i] - self.camera[i]) as f32);
        let m = &self.matrix;
        [0, 1, 2, 3].map(|row| m[row] * r[0] + m[4 + row] * r[1] + m[8 + row] * r[2] + m[12 + row])
    }
    /// The HUD point of a world point in front of the camera.
    pub fn project(&self, p: [f64; 3]) -> Option<[f32; 2]> {
        let c = self.clip(p);
        if c[3] <= 1e-4 {
            return None;
        }
        Some(self.screen(c))
    }
    fn screen(&self, c: [f32; 4]) -> [f32; 2] {
        [
            (c[0] / c[3] + 1.0) * 0.5 * self.width,
            (1.0 - c[1] / c[3]) * 0.5 * self.height,
        ]
    }
    /// The HUD rectangle `[x0, y0, x1, y1]` around a world box, when all of
    /// it is in front of the camera and some of it is on screen
    /// (`ProjectedEntityBounds`).
    pub fn project_box(&self, min: [f64; 3], max: [f64; 3]) -> Option<[f32; 4]> {
        let mut r = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for i in 0..8 {
            let p = [
                if i & 1 == 0 { min[0] } else { max[0] },
                if i & 2 == 0 { min[1] } else { max[1] },
                if i & 4 == 0 { min[2] } else { max[2] },
            ];
            let [x, y] = self.project(p)?;
            r = [r[0].min(x), r[1].min(y), r[2].max(x), r[3].max(y)];
        }
        let on_screen = r[2] >= 0.0 && r[0] <= self.width && r[3] >= 0.0 && r[1] <= self.height;
        on_screen.then_some(r)
    }
    /// The screen direction (unit, y down) from the screen centre toward a
    /// world point, also for points behind the camera (Arrows).
    pub fn direction(&self, p: [f64; 3]) -> Option<[f32; 2]> {
        let c = self.clip(p);
        // Unprojected and so not flipped behind the camera; scaled to the
        // screen's aspect like a projected point.
        let (x, y) = (c[0] * self.width, -c[1] * self.height);
        let length = x.hypot(y);
        (length > 1e-6).then(|| [x / length, y / length])
    }
    /// HUD units per block at a world point's depth, for billboards that
    /// face the camera (BedPlates): the projection's vertical scale (the
    /// length of its up row) over the point's distance along the view.
    pub fn units_per_block(&self, p: [f64; 3]) -> Option<f32> {
        let w = self.clip(p)[3];
        let m = &self.matrix;
        let up = (m[1] * m[1] + m[5] * m[5] + m[9] * m[9]).sqrt();
        (w > 1e-4).then(|| up / w * 0.5 * self.height)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
}

/// One item drawn as its model (the NameTags equipment row, BedPlates).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub id: String,
    pub count: u32,
    pub damage: u32,
    pub max_damage: u32,
    pub enchanted: bool,
}

/// A box behind text: the text's extent (one scaled line high) padded by
/// `pad`, filled, with an `edge`-thick border.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    pub fill: u32,
    pub pad: f32,
    pub edge: f32,
    pub edge_color: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Fill {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: u32,
    },
    Quad {
        corners: [[f32; 2]; 4],
        color: u32,
    },
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        color: u32,
    },
    /// Text with its top at `y` and its left edge or centre at `x`, scaled
    /// about that point, optionally over a bordered box.
    Text {
        x: f32,
        y: f32,
        scale: f32,
        text: String,
        color: u32,
        shadow: bool,
        align: Align,
        backdrop: Option<Backdrop>,
    },
    /// A 16-unit item model with its top left at `(x, y)`, scaled.
    Item {
        x: f32,
        y: f32,
        scale: f32,
        item: Item,
    },
    /// A draw turned `degrees` clockwise (y down) about `pivot`
    /// (Indicators' item).
    Turned {
        pivot: [f32; 2],
        degrees: f32,
        inner: Box<Draw>,
    },
}

impl Draw {
    pub fn fill(x: f32, y: f32, w: f32, h: f32, color: u32) -> Self {
        Self::Fill { x, y, w, h, color }
    }
    pub fn text(x: f32, y: f32, text: impl Into<String>, color: u32) -> Self {
        Self::Text {
            x,
            y,
            scale: 1.0,
            text: text.into(),
            color,
            shadow: true,
            align: Align::Left,
            backdrop: None,
        }
    }
    /// A rectangle's four sides, `width` thick, inside `r`.
    pub fn frame(out: &mut Vec<Self>, r: [f32; 4], width: f32, color: u32) {
        let [x0, y0, x1, y1] = r;
        out.push(Self::fill(x0, y0, x1 - x0, width, color));
        out.push(Self::fill(x0, y1 - width, x1 - x0, width, color));
        out.push(Self::fill(
            x0,
            y0 + width,
            width,
            y1 - y0 - 2.0 * width,
            color,
        ));
        out.push(Self::fill(
            x1 - width,
            y0 + width,
            width,
            y1 - y0 - 2.0 * width,
            color,
        ));
    }
}

/// A living entity this frame, as the overlay modules see it.
#[derive(Clone, Debug)]
pub struct Living {
    pub view: EntityView,
    pub max_health: f32,
    pub absorption: f32,
    /// The player list's display name, when it has one.
    pub display_name: Option<String>,
    /// A player in creative mode (the player list's game mode).
    pub creative: bool,
    /// Main hand, off hand, head, chest, legs, feet.
    pub equipment: [Option<Item>; 6],
    /// Hurt this tick (`hurtTime > 0`).
    pub hurt: bool,
}

/// The local player this frame.
#[derive(Clone, Debug, Default)]
pub struct Own {
    pub health: f32,
    pub absorption: f32,
    pub position: [f64; 3],
    pub yaw: f32,
    pub hurt: bool,
}

/// Everything the overlay modules draw from in one frame.
#[derive(Clone, Debug)]
pub struct Frame {
    pub projection: Projection,
    pub entities: Vec<Living>,
    pub own: Own,
    pub first_person: bool,
    pub now_ms: u64,
    /// Monster spawners with their mob id (SpawnerFinder).
    pub spawners: Vec<([i32; 3], String)>,
    /// The projectiles Indicators follows.
    pub indicators: Vec<super::indicators::Shot>,
    /// The beds BedPlates surveyed.
    pub bed_plates: Vec<super::bed_plates::Plate>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camera at the origin looking down -Z with a 90 degree FOV.
    fn straight() -> Projection {
        let f = 1.0f32; // 1 / tan(45 deg)
        let (near, far) = (0.05f32, 100.0f32);
        let mut m = [0.0f32; 16];
        m[0] = f;
        m[5] = f;
        m[10] = (far + near) / (near - far);
        m[11] = -1.0;
        m[14] = 2.0 * far * near / (near - far);
        Projection {
            camera: [0.0; 3],
            matrix: m,
            width: 200.0,
            height: 200.0,
        }
    }

    #[test]
    fn projects_points_and_boxes() {
        let p = straight();
        assert_eq!(p.project([0.0, 0.0, -5.0]), Some([100.0, 100.0]));
        let [x, y] = p.project([1.0, 1.0, -2.0]).unwrap();
        assert!((x - 150.0).abs() < 1e-3 && (y - 50.0).abs() < 1e-3);
        assert_eq!(p.project([0.0, 0.0, 5.0]), None);
        let r = p.project_box([-1.0, -1.0, -4.0], [1.0, 1.0, -2.0]).unwrap();
        assert!((r[0] - 50.0).abs() < 1e-3 && (r[3] - 150.0).abs() < 1e-3);
        assert_eq!(p.project_box([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]), None);
        assert_eq!(p.project_box([50.0, 0.0, -2.0], [51.0, 1.0, -1.0]), None);
        let d = p.direction([0.0, 0.0, 5.0]);
        assert!(d.is_none());
        let d = p.direction([3.0, 0.0, 5.0]).unwrap();
        assert!(
            (d[0] - 1.0).abs() < 1e-5,
            "behind and right stays right: {d:?}"
        );
    }
}
