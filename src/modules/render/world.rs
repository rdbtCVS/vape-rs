// Host-independent world-space line geometry and projectile prediction.
// Coordinates are absolute world coordinates; the host must apply its camera transform.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point(pub [f64; 3]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub start: Point,
    pub end: Point,
    pub color: u32,
    pub width: f32,
}

/// A filled world quad (camera-independent corners, ARGB), drawn through
/// walls and from both sides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub corners: [Point; 4],
    pub color: u32,
}

/// Append the six faces of a box.
pub fn fill_box(quads: &mut Vec<Quad>, min: Point, max: Point, color: u32) {
    let ([x0, y0, z0], [x1, y1, z1]) = (min.0, max.0);
    let faces = [
        [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
        [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
        [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]],
        [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
        [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]],
    ];
    quads.extend(faces.map(|f| Quad {
        corners: f.map(Point),
        color,
    }));
}

/// Append the twelve edges of a box. The host owns depth testing and clipping.
pub fn outline_box(lines: &mut Vec<Line>, min: Point, max: Point, color: u32, width: f32) {
    let [x0, y0, z0] = min.0;
    let [x1, y1, z1] = max.0;
    let corners = [
        Point([x0, y0, z0]),
        Point([x1, y0, z0]),
        Point([x1, y0, z1]),
        Point([x0, y0, z1]),
        Point([x0, y1, z0]),
        Point([x1, y1, z0]),
        Point([x1, y1, z1]),
        Point([x0, y1, z1]),
    ];
    for (a, b) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        lines.push(Line {
            start: corners[a],
            end: corners[b],
            color,
            width,
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImpactKind {
    Block,
    Entity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub position: Point,
    pub kind: ImpactKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prediction {
    pub points: Vec<Point>,
    pub impact: Option<Impact>,
}

/// Simulate the reference's drag-then-gravity update. The caller performs actual
/// host collision queries and returns the nearest impact on each segment.
/// A finite step cap prevents unloaded worlds or invalid host data from hanging a frame.
pub fn predict(
    start: Point,
    velocity: [f64; 3],
    gravity: f64,
    max_steps: usize,
    collision: impl FnMut(Point, Point) -> Option<Impact>,
) -> Prediction {
    predict_with_drag(
        start,
        velocity,
        gravity,
        f64::from(0.99_f32),
        max_steps,
        collision,
    )
}

/// [`predict`] with the per-tick velocity factor given: 1.0 for a
/// crossbow's firework, which flies straight without drag.
pub fn predict_with_drag(
    start: Point,
    velocity: [f64; 3],
    gravity: f64,
    drag: f64,
    max_steps: usize,
    mut collision: impl FnMut(Point, Point) -> Option<Impact>,
) -> Prediction {
    let mut result = Prediction {
        points: vec![start],
        impact: None,
    };
    let mut position = start;
    let mut velocity = velocity;
    if !start.0.iter().chain(velocity.iter()).all(|v| v.is_finite())
        || !gravity.is_finite()
        || !drag.is_finite()
    {
        return result;
    }
    for _ in 0..max_steps.min(4096) {
        let next = Point(std::array::from_fn(|i| position.0[i] + velocity[i]));
        if !next.0.iter().all(|v| v.is_finite()) {
            break;
        }
        if let Some(hit) = collision(position, next) {
            result.points.push(hit.position);
            result.impact = Some(hit);
            break;
        }
        result.points.push(next);
        position = next;
        for value in &mut velocity {
            *value *= drag;
        }
        velocity[1] -= gravity;
        if position.0[1] < -128.0 {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_has_twelve_edges_with_shared_style() {
        let mut lines = Vec::new();
        outline_box(
            &mut lines,
            Point([0.0; 3]),
            Point([1.0; 3]),
            0xff112233,
            2.0,
        );
        assert_eq!(lines.len(), 12);
        assert!(
            lines.iter().all(|line| line.color == 0xff112233
                && line.width == 2.0
                && line.start != line.end)
        );
    }

    #[test]
    fn projectile_updates_drag_and_gravity_after_motion() {
        let result = predict(
            Point([0.0; 3]),
            [1.0, 1.0, 0.0],
            f64::from(0.05_f32),
            2,
            |_, _| None,
        );
        assert_eq!(result.points[1], Point([1.0, 1.0, 0.0]));
        assert!((result.points[2].0[0] - 1.99).abs() < 1e-6);
        assert!((result.points[2].0[1] - 1.94).abs() < 1e-6);
    }

    #[test]
    fn collision_ends_at_hit_and_stops_queries() {
        let hit = Impact {
            position: Point([0.5, 0.0, 0.0]),
            kind: ImpactKind::Entity,
        };
        let mut calls = 0;
        let result = predict(Point([0.0; 3]), [1.0, 0.0, 0.0], 0.03, 100, |_, _| {
            calls += 1;
            Some(hit)
        });
        assert_eq!(calls, 1);
        assert_eq!(result.points, vec![Point([0.0; 3]), hit.position]);
        assert_eq!(result.impact, Some(hit));
    }

    #[test]
    fn invalid_values_and_step_cap_cannot_loop_forever() {
        assert_eq!(
            predict(
                Point([0.0; 3]),
                [f64::NAN, 0.0, 0.0],
                0.0,
                100,
                |_, _| panic!()
            )
            .points
            .len(),
            1
        );
        assert_eq!(
            predict(Point([0.0; 3]), [0.0; 3], 0.0, usize::MAX, |_, _| None)
                .points
                .len(),
            4097
        );
    }
}
