#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub position: [f64; 3],
    pub fps: f32,
    pub in_world: bool,
    pub show_hud: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CoordinatesLayout {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Eq, PartialEq)]
pub struct HudLine {
    pub label: String,
    pub value: String,
}

impl CoordinatesLayout {
    pub fn lines(self, position: [f64; 3]) -> Vec<HudLine> {
        if position.iter().any(|x| !x.is_finite()) {
            return Vec::new();
        }
        // Reference uses Java Math.round, including negative halfway values.
        let values = position.map(|x| ((x + 0.5).floor() as i64).to_string());
        match self {
            Self::Horizontal => vec![HudLine {
                label: String::new(),
                value: format!("X {}  Y {}  Z {}", values[0], values[1], values[2]),
            }],
            Self::Vertical => ["X", "Y", "Z"]
                .into_iter()
                .zip(values)
                .map(|(label, value)| HudLine {
                    label: label.into(),
                    value,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_follow_reference_rounding_at_negative_boundaries() {
        let lines = CoordinatesLayout::Horizontal.lines([-1.5, -0.5, 0.5]);
        assert_eq!(lines[0].value, "X -1  Y 0  Z 1");
        let vertical = CoordinatesLayout::Vertical.lines([-1.6, 64.2, 10.9]);
        assert_eq!(
            vertical
                .iter()
                .map(|l| l.value.as_str())
                .collect::<Vec<_>>(),
            ["-2", "64", "11"]
        );
    }

    #[test]
    fn invalid_positions_are_not_rendered() {
        assert!(
            CoordinatesLayout::Horizontal
                .lines([f64::NAN, 0.0, 0.0])
                .is_empty()
        );
    }
}
