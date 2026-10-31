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
    /// `ArmorStatusItemComponent.getDurabilityPercent`: none without
    /// durability, 1 at exactly no durability left, 100 past it.
    pub fn percent(&self) -> Option<u32> {
        let maximum = self.max_damage as f32;
        let remaining = maximum - self.damage as f32;
        (self.max_damage > 0).then(|| {
            if remaining == 0.0 {
                1
            } else if remaining < 0.0 {
                100
            } else {
                (remaining / maximum * 100.0).ceil() as u32
            }
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
    /// The vanilla scoreboard sidebar moved to the panel; the host draws it.
    Scoreboard {
        width: f32,
        height: f32,
    },
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
    /// Painted by the host from `text_gui::layout`; it needs live text
    /// measurement, so the generic command layout is empty.
    TextGui(super::text_gui::View),
    /// Painted by the host from `target_info::layout`, like the Text GUI.
    TargetInfo(super::target_info::View),
    /// One left-aligned line at (5, 5.5), like `ReachDisplayHudFrame`.
    Label {
        text: String,
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
/// This is the reference's own wrap, so a yaw of 180 reads 360, not 0.
pub fn heading(yaw: f32) -> f32 {
    let mut heading = yaw % 360.0;
    if heading < -180.0 {
        heading += 360.0;
    }
    if heading > 180.0 {
        heading -= 360.0;
    }
    heading + 180.0
}
// Vape theme colours (`ThemeColors`): B, I, d, Y, Z, A and i.
const GREEN: u32 = 0xff058669;
const ORANGE: u32 = 0xffec812c;
const RED: u32 = 0xfffa3238;
const BIOME: u32 = 0xffffa054;
const GREY: u32 = 0xffa3a3a3;
const LIGHT: u32 = 0xffd1d1d1;
const DIVIDER: u32 = 0x33ffffff;
/// `RenderUtils.q(ratio, true)`: red through yellow to the theme green.
fn durability_color(ratio: f32) -> u32 {
    let ratio = ratio.clamp(0.0, 1.0);
    let (from, to, t) = if ratio <= 0.5 {
        ([255.0, 0.0, 0.0], [255.0, 255.0, 0.0], ratio * 2.0)
    } else {
        ([255.0, 255.0, 0.0], [5.0, 134.0, 105.0], ratio * 2.0 - 1.0)
    };
    (0..3).fold(0xff000000, |argb, i| {
        argb | ((from[i] + (to[i] - from[i]) * t).round() as u32) << (16 - i * 8)
    })
}
/// A tinted texture multiplies every channel, alpha included.
fn tint(base: u32, by: u32) -> u32 {
    (0..4).fold(0, |argb, i| {
        let shift = i * 8;
        argb | (((base >> shift) & 0xff) * ((by >> shift) & 0xff) / 255) << shift
    })
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
    fn line(&mut self, from: [f32; 2], to: [f32; 2], width: f32, color: u32) {
        self.commands.push(Command::Line {
            from,
            to,
            width,
            color,
        });
    }
    fn triangle(&mut self, points: [[f32; 2]; 3], color: u32) {
        self.commands.push(Command::Triangle { points, color });
    }
    /// `CoordinatesHudFrame.drawDirectionArrow`.
    fn arrow(&mut self, x: f32, y: f32, positive: bool) {
        self.fill(x - 4.0, y - 4.0, 8.0, 8.0, 0x911a191a);
        if positive {
            self.triangle(
                [[x - 2.2, y + 0.5], [x, y - 1.5], [x + 1.7, y + 0.5]],
                GREEN,
            );
        } else {
            self.triangle([[x - 2.2, y - 1.0], [x + 1.7, y - 1.0], [x, y + 1.0]], RED);
        }
    }
    /// The reference's cracked `armor` shield, drawn as geometry over its
    /// 24x25 texture pixels; the texture itself is not shipped.
    fn shield(&mut self, x: f32, y: f32, scale: f32, color: u32) {
        let at = |px: f32, py: f32| [x + px * scale, y + py * scale];
        for [x0, y0, x1, y1] in [
            [8.0, 3.5, 19.5, 6.5],
            [13.5, 6.5, 19.5, 13.0],
            [5.0, 8.0, 11.5, 13.0],
            [5.0, 5.0, 6.5, 8.0],
        ] {
            let [left, top] = at(x0, y0);
            self.fill(left, top, (x1 - x0) * scale, (y1 - y0) * scale, color);
        }
        let point = [
            at(5.5, 13.0),
            at(19.5, 13.0),
            at(17.5, 16.5),
            at(12.0, 19.5),
            at(6.5, 16.5),
        ];
        for i in 1..4 {
            self.triangle([point[0], point[i], point[i + 1]], color);
        }
    }
    /// The reference's `bar-1` broken-bar dashes over its 49x12 texture pixels.
    fn broken_bar(&mut self, x: f32, y: f32, scale: f32, color: u32) {
        for (x0, x1) in [(5.0, 23.5), (25.5, 44.5)] {
            self.fill(
                x + x0 * scale,
                y + 3.5 * scale,
                (x1 - x0) * scale,
                3.0 * scale,
                color,
            );
        }
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
                // ArmorStatusItemComponent rows sit 2 in; compact rows are
                // 22 tall with 2 between them, detailed rows 20.
                let height = if *compact { 22.0 } else { 20.0 };
                let step = height + if *compact { 2.0 } else { 0.0 };
                for (i, piece) in pieces.iter().enumerate() {
                    let y = 2.0 + i as f32 * step;
                    if piece.broken {
                        output.fill(0.0, y, output.width, height, 0x78fa3239);
                        if *compact {
                            output.shield(6.0, y + 3.0, 0.45, RED);
                            output.broken_bar(3.0, y + 15.0, 0.15, RED);
                        } else {
                            output.shield(4.0, y + 3.0, 0.45, RED);
                            output.broken_bar(25.0, y + 11.0, 0.2, RED);
                            output.line([22.0, y + 4.0], [22.0, y + 16.0], 2.0, RED);
                            output.text(28.0, y + 5.0, 0.85, "0%", color);
                        }
                        continue;
                    }
                    let percent = piece.percent();
                    if *compact && percent.is_none() {
                        continue;
                    }
                    // No durability reads as -1%, which the detailed row
                    // still warns about, as the reference does.
                    let ratio = percent.map_or(-0.01, |p| p as f32 / 100.0);
                    if ratio < 0.1 {
                        let alpha = (30.0 + 30.0 * (0.1 - ratio) / 0.1) as u32;
                        output.fill(0.0, y, output.width, height, alpha << 24 | 0xfa3239);
                    }
                    if !*compact {
                        output.line([22.0, y + 4.0], [22.0, y + 16.0], 2.0, DIVIDER);
                    }
                    output.commands.push(Command::ArmorIcon {
                        slot: piece.slot,
                        x: if *compact { 5.0 } else { 4.0 },
                        y: y + if *compact { 0.0 } else { 2.0 },
                        preview: piece.preview,
                    });
                    let Some(percent) = percent else {
                        continue;
                    };
                    let (x, bar_y) = if *compact {
                        (3.0, y + 17.0)
                    } else {
                        output.text(28.0, y + 5.0, 0.85, format!("{percent}%"), color);
                        (28.0, y + 13.0)
                    };
                    output.fill(x, bar_y, 20.0, 1.5, 0x99000000);
                    output.fill(x, bar_y, 20.0 * ratio, 1.5, durability_color(ratio));
                    if *compact && i + 1 < pieces.len() {
                        output.line([9.0, y + 23.0], [17.0, y + 23.0], 2.0, DIVIDER);
                    }
                }
            }
            Content::Potions { effects } => {
                // PotionEffectsHudFrame: 30-tall rows from the top edge.
                let mut text_width = 0_i32;
                for (i, effect) in effects.iter().enumerate() {
                    let y = i as f32 * 30.0;
                    // Infinite effects (duration -1) came after the
                    // reference's versions; they keep a full ring and read ∞.
                    let percent = if effect.duration < 0 {
                        100.0
                    } else {
                        effect.duration as f32 / effect.maximum.max(1) as f32 * 100.0
                    };
                    let ring = if percent <= 25.0 {
                        RED
                    } else if percent <= 50.0 {
                        ORANGE
                    } else {
                        GREEN
                    };
                    output.commands.push(Command::Arc {
                        center: [16.125, y + 16.125],
                        radius: 10.225,
                        start: 0.0,
                        end: 360.0,
                        width: 0.8,
                        color: 0xc8000000,
                    });
                    output.commands.push(if percent >= 100.0 {
                        Command::Arc {
                            center: [16.0, y + 16.0],
                            radius: 10.25,
                            start: 0.0,
                            end: 360.0,
                            width: 1.5,
                            color: ring,
                        }
                    } else {
                        // Anticlockwise from the top, emptying from the right.
                        Command::Arc {
                            center: [16.0, y + 16.0],
                            radius: 10.0,
                            start: 270.0 - 3.6 * percent,
                            end: 270.0,
                            width: 1.5,
                            color: ring,
                        }
                    });
                    output.line([30.0, y + 10.0], [30.0, y + 21.5], 2.0, DIVIDER);
                    let duration = if effect.duration < 0 {
                        "∞".into()
                    } else {
                        let seconds = effect.duration / 20;
                        format!("{:02}:{:02}", seconds / 60, seconds % 60)
                    };
                    text_width = text_width
                        .max((measure(&effect.name) * 0.85) as i32)
                        .max((measure(&duration) * 0.85) as i32);
                    output.text(35.0, y + 9.0, 0.85, &effect.name, color);
                    let label = if percent > 100.0 { LIGHT } else { ring };
                    output.text(35.0, y + 17.0, 0.85, duration, label);
                    output.commands.push(Command::Sprite {
                        id: format!("mob_effect/{}", effect.id),
                        x: 9.0,
                        y: y + 10.0,
                        width: 14.0,
                        height: 14.0,
                    });
                }
                output.width = output.width.max(40.0 + text_width as f32);
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
                // CoordinatesHudFrame: Math.round positions, text x truncated.
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
                        let y = row * (i as f32 + 0.5);
                        output.text(8.0, y, 0.7, label, color);
                        let x = (8.0 + measure(label) * 0.7 + 2.5).floor();
                        output.text(x, y - 2.5, 1.1, &values[i], color);
                        if i != 1 {
                            let y = y + if i == 0 { 1.5 } else { 0.0 };
                            output.arrow(output.width - 12.0, y, positive[i]);
                        }
                        let y = row * (i as f32 + 1.0) + 1.5;
                        output.line([8.0, y], [output.width - 7.0, y], 1.0, DIVIDER);
                    }
                    let y = row * 3.5 - 2.5;
                    output.text(8.0, y, 0.7, "BIOME:", color);
                    let x = (8.0 + measure("BIOME:") * 0.7 + 4.0).floor();
                    output.text(x, y, 0.7, biome, BIOME);
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
                        }
                        if i != 2 {
                            if i == 0 {
                                x += 10.0;
                            }
                            let (left, top) = (x.floor(), (half / 2.0).floor());
                            output.line([left, top], [left, top + 8.0], 1.2, DIVIDER);
                            x += 10.0;
                        }
                    }
                    // The frame ends 12 past the Z arrow's centre.
                    output.width = output.width.max(x + 12.0);
                    output.text(10.0, half * 1.5 - 2.5, 0.75, "BIOME:", color);
                    let x = (10.0 + measure("BIOME: ") * 0.75).floor();
                    output.text(x, half * 1.5 - 2.5, 0.75, biome, BIOME);
                }
            }
            Content::TextGui(_) | Content::TargetInfo(_) => {}
            Content::Scoreboard { width, height } => {
                // The host paints the sidebar itself over this box.
                output.width = *width;
                output.height = *height;
            }
            Content::Label { text } => {
                output.width = output.width.max(measure(text) + 10.0);
                output.text(5.0, 5.5, 1.0, text.clone(), color);
            }
            Content::Compass { yaw } => {
                // CompassStripComponent: 1400 tenth-degree steps across the
                // strip from 70 degrees left of the heading, the first one
                // truncated to a tenth; every fifth degree gets a tick.
                let heading = heading(*yaw);
                let first = ((heading - 70.0) * 10.0) as i32;
                let spacing = (output.width + 4.0) / 1400.0;
                let middle = output.height / 2.0;
                let mut tenth = first + (50 - first.rem_euclid(50)) % 50;
                while tenth - first < 1400 {
                    let x = (tenth - first + 1) as f32 * spacing;
                    let degree = (tenth / 10).rem_euclid(360);
                    let major = degree % 45 == 0;
                    // compassbigpoint/compasssmallpoint: a one-pixel column
                    // just right of x, with a faint shadow copy unbackgrounded.
                    let (top, bottom, white, shadow) = if major {
                        (middle - 9.125, middle - 2.875, 0x66ffffff, 0x28000000)
                    } else {
                        (middle - 7.0, middle - 5.0, 0x60ffffff, 0x13000000)
                    };
                    if !self.background {
                        output.line([x + 1.0, top], [x + 1.0, bottom], 1.0, shadow);
                    }
                    output.line([x + 1.0, top], [x + 1.0, bottom], 1.0, white);
                    if degree % 15 == 0 {
                        let text = if major {
                            ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][(degree / 45) as usize]
                                .into()
                        } else {
                            degree.to_string()
                        };
                        let label = if !self.background {
                            0xffe6e6e6
                        } else if major {
                            0xffffffff
                        } else {
                            GREY
                        };
                        output.text(x - measure(&text) * 0.35, middle, 0.7, text, label);
                    }
                    tenth += 50;
                }
                // compassarrow at half scale, tinted by the foreground.
                let (x, y) = (output.width / 2.0 - 2.7, -2.5);
                let arrow = tint(RED, color);
                output.fill(x + 1.8, y + 1.0, 5.4, 1.5, arrow);
                output.triangle(
                    [[x + 1.8, y + 2.5], [x + 7.2, y + 2.5], [x + 4.5, y + 6.75]],
                    arrow,
                );
                output.fill(x + 4.0, y + 8.0, 1.0, 6.0, tint(0x8be52e33, color));
                let text = (heading as i32).to_string();
                let x = output.width / 2.0 + 2.3 - measure(&text) * 0.4;
                output.text(x, -8.0, 0.8, text, 0xffe6e6e6);
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
            [0.0, 90.0, 180.0, 270.0, 360.0]
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
    fn texts(layout: &Layout) -> Vec<(f32, f32, &str, u32)> {
        layout
            .commands
            .iter()
            .filter_map(|c| match c {
                Command::Text {
                    x, y, text, color, ..
                } => Some((*x, *y, text.as_str(), *color)),
                _ => None,
            })
            .collect()
    }
    fn piece(slot: usize, max_damage: u32, damage: u32, broken: bool) -> ArmorPiece {
        ArmorPiece {
            slot,
            max_damage,
            damage,
            preview: false,
            broken,
        }
    }
    #[test]
    fn armor_percent_follows_the_reference_edges() {
        assert_eq!(piece(0, 0, 0, false).percent(), None);
        assert_eq!(piece(0, 363, 363, false).percent(), Some(1));
        assert_eq!(piece(0, 363, 400, false).percent(), Some(100));
        assert_eq!(piece(0, 363, 0, false).percent(), Some(100));
        assert_eq!(piece(0, 363, 362, false).percent(), Some(1));
        assert_eq!(piece(0, 363, 181, false).percent(), Some(51));
        assert_eq!(durability_color(0.0), 0xffff0000);
        assert_eq!(durability_color(0.5), 0xffffff00);
        assert_eq!(durability_color(1.0), GREEN);
        assert_eq!(tint(RED, 0xffffffff), RED);
        assert_eq!(tint(RED, 0x80ffffff) >> 24, 0x80);
    }
    #[test]
    fn armor_rows_draw_detailed_and_compact_reference_layouts() {
        let pieces = vec![
            piece(0, 100, 0, false),
            piece(1, 100, 95, false),
            piece(2, 0, 0, false),
            piece(3, 100, 100, true),
        ];
        let detailed = panel(
            Content::Armor {
                pieces: pieces.clone(),
                compact: false,
            },
            54.0,
            84.0,
        )
        .layout(|s| s.len() as f32 * 6.0);
        let text = texts(&detailed);
        // The no-durability helmet has no percentage; the broken boots read 0%.
        assert_eq!(
            text.iter().map(|t| (t.1, t.2)).collect::<Vec<_>>(),
            [(7.0, "100%"), (27.0, "5%"), (67.0, "0%")]
        );
        // Low and no-durability rows warn with a fill as tall as the row.
        let warnings = detailed
            .commands
            .iter()
            .filter_map(|c| match c {
                Command::Fill {
                    y, height, color, ..
                } if *color & 0xffffff == 0xfa3239 => Some((*y, *height, *color >> 24)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            warnings,
            [(22.0, 20.0, 45), (42.0, 20.0, 63), (62.0, 20.0, 120)]
        );
        let icons = |layout: &Layout| {
            layout
                .commands
                .iter()
                .filter_map(|c| match c {
                    Command::ArmorIcon { slot, x, y, .. } => Some((*slot, *x, *y)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            icons(&detailed),
            [(0, 4.0, 4.0), (1, 4.0, 24.0), (2, 4.0, 44.0)]
        );
        // The broken row draws the red shield and bar instead of an icon.
        assert!(detailed.commands.iter().any(|c| matches!(
            c,
            Command::Triangle { color: RED, points } if points[0][1] > 62.0
        )));
        let compact = panel(
            Content::Armor {
                pieces,
                compact: true,
            },
            26.0,
            98.0,
        )
        .layout(|s| s.len() as f32 * 6.0);
        assert!(texts(&compact).is_empty());
        // Compact skips the no-durability piece entirely.
        assert_eq!(icons(&compact), [(0, 5.0, 2.0), (1, 5.0, 26.0)]);
        let dividers = compact
            .commands
            .iter()
            .filter(|c| matches!(c, Command::Line { color: DIVIDER, .. }))
            .count();
        assert_eq!(dividers, 2);
    }
    #[test]
    fn potion_rows_count_down_with_reference_colours() {
        let effects = vec![
            PotionView::preview("speed", 6500, false),
            PotionView::preview("slowness", 5000, true),
            PotionView::preview("fire_resistance", 1000, false),
            PotionView {
                maximum: 100,
                ..PotionView::preview("water_breathing", -1, false)
            },
            PotionView {
                maximum: 72000,
                ..PotionView::preview("night_vision", 72000, false)
            },
        ];
        let output =
            panel(Content::Potions { effects }, 40.0, 152.0).layout(|s| s.len() as f32 * 6.0);
        let durations = texts(&output)
            .into_iter()
            .filter(|t| t.0 == 35.0 && (t.1 - 17.0) % 30.0 == 0.0)
            .map(|t| (t.2, t.3))
            .collect::<Vec<_>>();
        assert_eq!(
            durations,
            [
                ("05:25", GREEN),
                ("04:10", ORANGE),
                ("00:50", RED),
                ("∞", GREEN),
                ("60:00", GREEN)
            ]
        );
        let rings = output
            .commands
            .iter()
            .filter_map(|c| match c {
                Command::Arc {
                    center,
                    start,
                    end,
                    color,
                    ..
                } if *color != 0xc8000000 => Some((center[1], *start, *end, *color)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(rings[0], (16.0, 270.0 - 234.0, 270.0, GREEN));
        assert_eq!(rings[1], (46.0, 90.0, 270.0, ORANGE));
        assert_eq!(rings[3], (106.0, 0.0, 360.0, GREEN));
        // "Fire Resistance" is the widest text: 15 glyphs at 0.85.
        assert_eq!(output.width, 40.0 + (15.0 * 6.0 * 0.85_f32) as i32 as f32);
    }
    #[test]
    fn compass_ticks_follow_the_tenth_degree_strip() {
        let output =
            panel(Content::Compass { yaw: 0.0 }, 308.0, 30.0).layout(|s| s.len() as f32 * 6.0);
        let ticks = output
            .commands
            .iter()
            .filter_map(|c| match c {
                Command::Line { from, color, .. } if *color >> 24 >= 0x60 => {
                    Some((from[0], *color))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        // 140 degrees in steps of 5 from 110 to 250, S major at the centre.
        assert_eq!(ticks.len(), 28);
        let spacing = 312.0 / 1400.0;
        assert!(ticks.contains(&(701.0 * spacing + 1.0, 0x66ffffff)));
        assert!(ticks.contains(&(51.0 * spacing + 1.0, 0x60ffffff)));
        let text = texts(&output);
        assert!(text.contains(&(701.0 * spacing - 3.0 * 0.7, 15.0, "S", 0xffffffff)));
        assert!(text.contains(&(851.0 * spacing - 9.0 * 0.7, 15.0, "195", GREY)));
        assert!(text.contains(&(154.0 + 2.3 - 9.0 * 0.8, -8.0, "180", 0xffe6e6e6)));
        let mut bare = panel(Content::Compass { yaw: 0.0 }, 308.0, 30.0);
        bare.background = false;
        let bare = bare.layout(|s| s.len() as f32 * 6.0);
        assert!(texts(&bare).iter().all(|t| t.3 == 0xffe6e6e6));
        assert_eq!(bare.commands.len(), output.commands.len() + 28);
    }
}
