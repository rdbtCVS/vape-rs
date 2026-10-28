use super::{CoordinatesLayout, HudPanel};

#[derive(Clone, Debug)]
pub enum Content {
    Lines,
    Coordinates {
        layout: CoordinatesLayout,
        position: [f64; 3],
        yaw: f32,
        biome: String,
    },
    Compass {
        yaw: f32,
    },
}
#[derive(Clone, Debug)]
pub enum Command {
    Text {
        x: f32,
        y: f32,
        scale: f32,
        text: String,
        color: u32,
    },
    Fill {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: u32,
    },
    Triangle {
        points: [[f32; 2]; 3],
        color: u32,
    },
}
#[derive(Debug)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    pub commands: Vec<Command>,
}
/// Minecraft yaw zero faces south; the reference strip uses north = zero.
pub fn heading(yaw: f32) -> f32 {
    (yaw + 180.0).rem_euclid(360.0)
}
pub fn biome_title(id: &str) -> String {
    id.strip_prefix("minecraft:")
        .unwrap_or(id)
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
impl Layout {
    fn text(&mut self, x: f32, y: f32, scale: f32, text: impl Into<String>, color: u32) {
        self.commands.push(Command::Text {
            x,
            y,
            scale,
            text: text.into(),
            color,
        });
    }
    fn fill(&mut self, x: f32, y: f32, width: f32, height: f32, color: u32) {
        self.commands.push(Command::Fill {
            x,
            y,
            width,
            height,
            color,
        });
    }
    fn arrow(&mut self, x: f32, y: f32, positive: bool) {
        self.fill(x - 4.0, y - 4.0, 8.0, 8.0, 0x91252426);
        let points = if positive {
            [[x - 2.2, y + 0.5], [x, y - 1.5], [x + 1.7, y + 0.5]]
        } else {
            [[x - 2.2, y - 1.0], [x + 1.7, y - 1.0], [x, y + 1.0]]
        };
        self.commands.push(Command::Triangle {
            points,
            color: if positive { 0xff06a17e } else { 0xffe35656 },
        });
    }
}
impl HudPanel {
    pub fn layout(&self, measure: impl Fn(&str) -> f32) -> Layout {
        let mut output = Layout {
            width: self.min_width,
            height: self.min_height,
            commands: Vec::new(),
        };
        let color = self.color;
        match &self.content {
            Content::Lines => {
                if output.width == 0.0 {
                    output.width = self
                        .lines
                        .iter()
                        .map(|line| measure(&line.value) * 1.2 + 8.0)
                        .fold(0.0, f32::max);
                }
                output.height = output.height.max(self.lines.len() as f32 * 12.0 + 4.0);
                for (i, line) in self.lines.iter().enumerate() {
                    let text = if line.label.is_empty() {
                        line.value.clone()
                    } else {
                        format!("{} {}", line.label, line.value)
                    };
                    let x = (output.width - measure(&text) * 1.2) / 2.0;
                    let y = (output.height - 9.0 * 1.2) / 2.0 + i as f32 * 12.0;
                    output.text(x.floor(), y.floor(), 1.2, text, color);
                }
            }
            Content::Coordinates {
                layout,
                position,
                yaw,
                biome,
            } => {
                let values = position.map(|v| ((v + 0.5).floor() as i64).to_string());
                let heading = heading(*yaw);
                let positive = [
                    heading > 0.0 && heading < 180.0,
                    false,
                    heading > 90.0 && heading < 270.0,
                ];
                if *layout == CoordinatesLayout::Vertical {
                    let row = output.height / 4.0;
                    for i in 0..3 {
                        let label = ["X", "Y", "Z"][i];
                        output.text(8.0, row * (i as f32 + 0.5), 0.7, label, color);
                        output.text(
                            8.0 + measure(label) * 0.7 + 2.5,
                            row * (i as f32 + 0.5) - 2.5,
                            1.1,
                            &values[i],
                            color,
                        );
                        if i != 1 {
                            output.arrow(
                                output.width - 12.0,
                                row * (i as f32 + 0.5) + if i == 0 { 1.5 } else { 0.0 },
                                positive[i],
                            );
                        }
                        output.fill(
                            8.0,
                            row * (i as f32 + 1.0) + 1.5,
                            output.width - 15.0,
                            1.0,
                            0x33ffffff,
                        );
                    }
                    let y = row * 3.5 - 2.5;
                    output.text(8.0, y, 0.7, "BIOME:", color);
                    output.text(
                        8.0 + measure("BIOME:") * 0.7 + 4.0,
                        y,
                        0.7,
                        biome,
                        0xffa7a5a7,
                    );
                } else {
                    let half = output.height / 2.0;
                    let mut x = 10.0_f32;
                    for i in 0..3 {
                        let label = ["X", "Y", "Z"][i];
                        output.text(x.floor(), half / 2.0 + 2.5, 0.75, label, color);
                        x += measure(label) * 0.75 + 2.5;
                        output.text(x.floor(), half / 2.0, 1.2, &values[i], color);
                        x += (5.0 + measure("0") * 1.2 * values[i].len() as f32).max(22.0);
                        if i != 1 {
                            output.arrow(x, half / 2.0 + 4.0, positive[i]);
                            x += 10.0;
                        }
                        if i != 2 {
                            output.fill(x.floor(), (half / 2.0).floor(), 1.2, 8.0, 0x33ffffff);
                            x += 10.0;
                        }
                    }
                    output.width = output.width.max(x + 12.0);
                    output.text(10.0, half * 1.5 - 2.5, 0.75, "BIOME:", color);
                    output.text(
                        10.0 + measure("BIOME: ") * 0.75,
                        half * 1.5 - 2.5,
                        0.75,
                        biome,
                        0xffa7a5a7,
                    );
                }
            }
            Content::Compass { yaw } => {
                let heading = heading(*yaw);
                let start = heading - 70.0;
                for tick in ((start / 5.0).ceil() as i32)..=((heading + 70.0) / 5.0).floor() as i32
                {
                    let degree = tick * 5;
                    let normalized = degree.rem_euclid(360);
                    let x = (degree as f32 - start) * (output.width + 4.0) / 140.0;
                    let major = normalized % 45 == 0;
                    output.fill(
                        x,
                        output.height / 2.0 - 6.0,
                        if major { 1.5 } else { 1.0 },
                        if major { 10.0 } else { 8.0 },
                        color,
                    );
                    if normalized % 15 == 0 {
                        let text = if major {
                            ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][(normalized / 45) as usize]
                                .into()
                        } else {
                            normalized.to_string()
                        };
                        output.text(
                            x - measure(&text) * 0.35,
                            output.height / 2.0,
                            0.7,
                            text,
                            if major { color } else { 0xffa7a5a7 },
                        );
                    }
                }
                let x = output.width / 2.0;
                output.commands.push(Command::Triangle {
                    points: [[x - 3.0, 0.0], [x + 3.0, 0.0], [x, 5.0]],
                    color,
                });
                let text = format!("{:.0}", heading);
                output.text(x - measure(&text) * 0.35, -8.0, 0.7, text, color);
            }
        }
        output
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heading_wraps_and_biome_names_are_readable() {
        assert_eq!(
            [
                heading(-180.0),
                heading(-90.0),
                heading(0.0),
                heading(90.0),
                heading(540.0)
            ],
            [0.0, 90.0, 180.0, 270.0, 0.0]
        );
        assert_eq!(
            biome_title("minecraft:old_growth_birch_forest"),
            "Old Growth Birch Forest"
        );
    }
    fn panel(content: Content, width: f32, height: f32) -> HudPanel {
        HudPanel {
            module: "test",
            name: "test",
            enabled: true,
            min_width: width,
            min_height: height,
            x: 0.0,
            y: 0.0,
            scale: 1.0,
            color: 0xffffffff,
            background: true,
            lines: Vec::new(),
            content,
        }
    }
    #[test]
    fn compass_tracks_live_cardinals_through_north_wrap() {
        for (yaw, cardinal) in [
            (-180.0, "N"),
            (-90.0, "E"),
            (0.0, "S"),
            (90.0, "W"),
            (179.0, "N"),
        ] {
            let layout =
                panel(Content::Compass { yaw }, 308.0, 30.0).layout(|s| s.len() as f32 * 6.0);
            let center = layout
                .commands
                .iter()
                .filter_map(|c| match c {
                    Command::Text { x, text, scale, .. } if text == cardinal => {
                        Some(*x + text.len() as f32 * 6.0 * scale / 2.0)
                    }
                    _ => None,
                })
                .next()
                .unwrap();
            assert!((center - 154.0).abs() < 5.0, "{yaw}: {center}");
        }
    }
    #[test]
    fn coordinates_include_live_biome_and_two_direction_arrows() {
        for layout in [CoordinatesLayout::Horizontal, CoordinatesLayout::Vertical] {
            let output = panel(
                Content::Coordinates {
                    layout,
                    position: [-1.5, 64.0, 10.0],
                    yaw: -90.0,
                    biome: "Forest".into(),
                },
                70.0,
                if layout == CoordinatesLayout::Vertical {
                    90.0
                } else {
                    35.0
                },
            )
            .layout(|s| s.len() as f32 * 6.0);
            assert_eq!(
                output
                    .commands
                    .iter()
                    .filter(|c| matches!(c, Command::Triangle { .. }))
                    .count(),
                2
            );
            let texts = output
                .commands
                .iter()
                .filter_map(|c| match c {
                    Command::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(texts.contains(&"Forest"));
            assert!(texts.contains(&"-1"));
            assert!(output.commands.iter().all(|c| match c {
                Command::Text { x, y, scale, .. } =>
                    x.is_finite() && y.is_finite() && scale.is_finite(),
                _ => true,
            }));
        }
    }
}
