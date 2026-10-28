use super::{CoordinatesLayout, HudPanel};

#[derive(Clone, Debug)]
pub struct ArmorPiece {
    pub slot: usize,
    pub max_damage: u32,
    pub damage: u32,
    pub preview: bool,
    pub broken: bool,
}
impl ArmorPiece {
    pub fn percent(&self) -> Option<u32> {
        (self.max_damage > 0).then(|| {
            ((self.max_damage.saturating_sub(self.damage) as f64 / self.max_damage as f64 * 100.0)
                .ceil() as u32)
                .clamp(1, 100)
        })
    }
}
#[derive(Clone, Debug)]
pub struct PotionView {
    pub id: String,
    pub name: String,
    pub duration: i32,
    pub maximum: i32,
    pub negative: bool,
    pub amplifier: i32,
    pub color: u32,
}
impl PotionView {
    pub fn preview(id: &str, duration: i32, negative: bool) -> Self {
        Self {
            id: id.into(),
            name: biome_title(id),
            duration,
            maximum: 10000,
            negative,
            amplifier: 0,
            color: 0xff06a17e,
        }
    }
}
#[derive(Clone, Debug)]
pub enum Content {
    Lines,
    Clock {
        time: super::clock::ClockTime,
        digital: bool,
        show_date: bool,
        twenty_four: bool,
    },
    Keystrokes {
        keys: [bool; 5],
        mouse: [bool; 2],
        cps: [u16; 2],
        arrows: bool,
        icons: bool,
        space: bool,
        cps_only: bool,
    },
    Armor {
        pieces: Vec<ArmorPiece>,
        compact: bool,
    },
    Potions {
        effects: Vec<PotionView>,
    },
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
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        color: u32,
    },
    Arc {
        center: [f32; 2],
        radius: f32,
        start: f32,
        end: f32,
        width: f32,
        color: u32,
    },
    ArmorIcon {
        slot: usize,
        x: f32,
        y: f32,
        preview: bool,
    },
    Sprite {
        id: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
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
    // A single drawn key combines explicit rectangle geometry, state and font metrics.
    #[allow(clippy::too_many_arguments)]
    fn key(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        pressed: bool,
        measure: &impl Fn(&str) -> f32,
    ) {
        self.fill(x, y, w, h, if pressed { 0xffd1d1d1 } else { 0x99252426 });
        self.text(
            x + (w - measure(label) * 0.7) / 2.0,
            y + (h - 6.3) / 2.0,
            0.7,
            label,
            if pressed { 0xff1a191a } else { 0xffd1d1d1 },
        );
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
            Content::Clock {
                time,
                digital,
                show_date,
                twenty_four,
            } => {
                let hour = time.hour_text(*twenty_four);
                let minute = time.minute_text();
                if *digital {
                    let scale = 2.3;
                    output.text(30.0 - measure(&hour) * scale, 2.0, scale, hour, color);
                    output.fill(34.0, 14.0, 2.0, 2.0, color);
                    output.fill(34.0, 20.0, 2.0, 2.0, color);
                    output.text(39.0, 2.0, scale, &minute, color);
                    if !*twenty_four {
                        output.text(
                            39.0 + measure(&minute) * scale,
                            18.0,
                            1.0,
                            time.meridiem(),
                            color,
                        );
                    }
                    if *show_date {
                        let text = time.date(true);
                        let x = output.width - measure(&text) - 6.0;
                        output.text(x, 7.0, 1.0, text, color);
                        let day = time.weekday_text();
                        output.text(output.width - measure(day) - 6.0, 17.0, 1.0, day, color);
                    }
                } else {
                    let center = [output.width / 2.0, output.height / 2.0];
                    for tick in 0..24 {
                        if [8, 9, 10, 14, 15, 16, 20, 21, 22].contains(&tick) {
                            continue;
                        }
                        let a = (tick as f32 * 15.0 - 90.0).to_radians();
                        output.fill(
                            center[0] + a.cos() * 25.0 - 0.5,
                            center[1] + a.sin() * 25.0,
                            1.0,
                            1.0,
                            color,
                        );
                    }
                    let a =
                        (time.hour as f32 * 30.0 + time.minute as f32 / 2.0 - 90.0).to_radians();
                    output.commands.push(Command::Line {
                        from: center,
                        to: [center[0] + a.cos() * 26.0, center[1] + a.sin() * 26.0],
                        width: 1.8,
                        color: 0xff06a17e,
                    });
                    output.text(28.0 - measure(&hour) * 2.3, 5.0, 2.3, hour, color);
                    output.text(
                        output.width - measure(&minute) * 2.3 - 5.0,
                        output.height - 27.0,
                        2.3,
                        minute,
                        color,
                    );
                    output.text(10.0, output.height - 23.0, 0.85, time.weekday_text(), color);
                    output.text(10.0, output.height - 15.0, 0.85, time.date(false), color);
                    if !*twenty_four {
                        output.text(
                            output.width - measure(time.meridiem()) * 0.85 - 5.0,
                            5.0,
                            0.85,
                            time.meridiem(),
                            color,
                        );
                    }
                }
            }
            Content::Keystrokes {
                keys,
                mouse,
                cps,
                arrows,
                icons,
                space,
                cps_only,
            } => {
                if *cps_only {
                    let text = format!("{} | {} CPS", cps[0], cps[1]);
                    output.text((75.0 - measure(&text) * 0.9) / 2.0, 6.0, 0.9, text, color);
                } else {
                    let ky = if *icons { 4.0 } else { 2.0 };
                    for (index, x, y, label, arrow) in [
                        (0, 19.0, ky - 2.0, "W", "↑"),
                        (1, 0.0, ky + 19.0, "A", "←"),
                        (2, 19.0, ky + 19.0, "S", "↓"),
                        (3, 38.0, ky + 19.0, "D", "→"),
                    ] {
                        output.key(
                            x,
                            y,
                            17.0,
                            17.0,
                            if *arrows { arrow } else { label },
                            keys[index],
                            &measure,
                        );
                    }
                    if *space {
                        output.key(0.0, ky + 39.5, 55.25, 11.0, "—", keys[4], &measure);
                    }
                    let mx = if *icons { 61.0 } else { 0.0 };
                    let my = if *icons {
                        ky - 6.0
                    } else {
                        ky + 40.0 + if *space { 14.0 } else { 0.0 }
                    };
                    if *icons {
                        output.key(mx - 2.0, my + 2.0, 19.0, 22.0, "L", mouse[0], &measure);
                        output.key(mx + 18.0, my + 2.0, 19.0, 22.0, "R", mouse[1], &measure);
                        output.fill(mx + 17.0, my + 7.0, 2.0, 8.0, color);
                    } else {
                        output.key(mx, my, 26.35, 16.0, "LMB", mouse[0], &measure);
                        output.key(mx + 28.35, my, 26.35, 16.0, "RMB", mouse[1], &measure);
                    }
                    let text = format!("{} | {} CPS", cps[0], cps[1]);
                    let w = if *icons { 40.0 } else { 55.0 };
                    output.text(
                        mx + (w - measure(&text) * 0.65) / 2.0,
                        my + if *icons { 25.0 } else { 19.0 },
                        0.65,
                        text,
                        color,
                    );
                }
            }
            Content::Armor { pieces, compact } => {
                let step = if *compact { 24.0 } else { 20.0 };
                for (i, piece) in pieces.iter().enumerate() {
                    let y = 2.0 + i as f32 * step;
                    let percent = piece.percent();
                    let ratio = percent.unwrap_or(100) as f32 / 100.0;
                    if piece.broken || ratio < 0.1 {
                        output.fill(
                            0.0,
                            y,
                            output.width,
                            step,
                            if piece.broken { 0x78fa3239 } else { 0x32fa3239 },
                        );
                    }
                    if piece.broken {
                        output.text(5.0, y + 3.0, 1.1, "!", 0xfffa3239);
                    } else {
                        output.commands.push(Command::ArmorIcon {
                            slot: piece.slot,
                            x: if *compact { 5.0 } else { 4.0 },
                            y: y + if *compact { 0.0 } else { 2.0 },
                            preview: piece.preview,
                        });
                    }
                    if let Some(percent) = percent {
                        let c = if ratio > 0.5 {
                            0xff06a17e
                        } else if ratio > 0.2 {
                            0xffe0b84a
                        } else {
                            0xfffa3239
                        };
                        let (x, bar_y, w) = if *compact {
                            (3.0, y + 17.0, 20.0)
                        } else {
                            output.text(
                                28.0,
                                y + 5.0,
                                0.85,
                                format!("{}%", if piece.broken { 0 } else { percent }),
                                color,
                            );
                            (28.0, y + 13.0, 20.0)
                        };
                        output.fill(x, bar_y, w, 1.5, 0x99000000);
                        output.fill(x, bar_y, w * if piece.broken { 0.0 } else { ratio }, 1.5, c);
                    }
                    if *compact && i + 1 < pieces.len() {
                        output.fill(9.0, y + 23.0, 8.0, 1.0, 0x33ffffff);
                    }
                    if !*compact {
                        output.fill(22.0, y + 4.0, 1.5, 12.0, 0x33ffffff);
                    }
                }
            }
            Content::Potions { effects } => {
                output.width = effects
                    .iter()
                    .map(|e| measure(&e.name) * 0.85 + 40.0)
                    .fold(40.0, f32::max)
                    .max(output.width);
                for (i, effect) in effects.iter().enumerate() {
                    let y = 2.0 + i as f32 * 30.0;
                    let ratio = if effect.duration < 0 {
                        1.0
                    } else {
                        effect.duration as f32 / effect.maximum.max(1) as f32
                    }
                    .clamp(0.0, 1.0);
                    let c = if ratio > 0.5 {
                        0xff06a17e
                    } else if ratio > 0.25 {
                        0xffe0b84a
                    } else {
                        0xfffa3239
                    };
                    output.commands.push(Command::Arc {
                        center: [16.0, y + 16.0],
                        radius: 11.0,
                        start: 0.0,
                        end: 360.0,
                        width: 1.8,
                        color: 0xc8000000,
                    });
                    output.commands.push(Command::Arc {
                        center: [16.0, y + 16.0],
                        radius: 11.0,
                        start: -90.0,
                        end: -90.0 + 360.0 * ratio,
                        width: 2.0,
                        color: c,
                    });
                    output.commands.push(Command::Sprite {
                        id: format!("mob_effect/{}", effect.id),
                        x: 9.0,
                        y: y + 9.0,
                        width: 14.0,
                        height: 14.0,
                    });
                    output.fill(30.0, y + 10.0, 1.5, 11.5, 0x33ffffff);
                    output.text(35.0, y + 7.0, 0.85, &effect.name, color);
                    let duration = if effect.duration < 0 {
                        "∞".into()
                    } else {
                        let seconds = effect.duration / 20;
                        format!("{}:{:02}", seconds / 60, seconds % 60)
                    };
                    output.width = output.width.max(40.0 + measure(&duration) * 0.85);
                    output.text(35.0, y + 17.0, 0.85, duration, c);
                }
            }
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
