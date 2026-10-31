// The Text GUI module list (`TextGuiSettings` + `TextGuiOverlayComponent`):
// enabled modules with a GUI colour, sorted by rendered length or name, each
// with a Basic/Extended suffix, a row background and a coloured edge bar.
// This file is pure: the core builds a `View`, the host painter measures and
// draws the `Draw` list `layout` returns.

use super::super::core::rainbow::{readable_hsb, rgb_to_hsb};
use super::super::core::settings::{Definition, Kind};
use serde_json::json;

pub const ID: &str = "hud.text-gui";
pub const CLICK_DISABLE: &str = "hud.text-gui.click-disable";
const SUFFIX_GRAY: u32 = 0xff969696;

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    let color =
        |id: &str, label: &str| Definition::new(&key(id), label, Kind::Color, json!(0xffce0707u32));
    let describe = |mut d: Definition, text: &str| {
        d.description = text.into();
        d
    };
    vec![
        Definition::choice(&key("sort"), "Sort", &["Length", "Alphabetical"], "Length"),
        Definition::choice(
            &key("suffix"),
            "Suffix mode",
            &["Basic", "Extended", "None"],
            "Basic",
        ),
        Definition::choice(
            &key("color-mode"),
            "Color Mode",
            &["Module color", "Match GUI color", "Custom color"],
            "Module color",
        ),
        color("color", "Text GUI color").when(&key("color-mode"), json!("Custom color")),
        describe(
            Definition::boolean(CLICK_DISABLE, "Click disable", false),
            "Click modules in text gui to toggle them",
        ),
        describe(
            Definition::boolean(&key("shadow"), "Shadow", true),
            "Renders shadowed text",
        ),
        describe(
            Definition::boolean(&key("animations"), "Animations", true),
            "Use animations on text gui",
        ),
        Definition::boolean(&key("smooth-font"), "Smooth font", true),
        describe(
            Definition::boolean(&key("watermark"), "Watermark", false),
            "Renders a vape watermark",
        ),
        describe(
            Definition::boolean(&key("hide-modules"), "Hide modules", false),
            "Allows you to blacklist certain modules from being shown",
        ),
        // Stored like the reference's, which never reads it.
        describe(
            Definition::boolean(&key("rescale"), "Rescale", true),
            "Rescales text GUI",
        ),
        describe(
            Definition::new(
                &key("hidden"),
                "Hidden Modules",
                Kind::StringList {
                    max_entries: 64,
                    max_chars: 64,
                },
                json!(["ESP", "NameTags", "StorageESP"]),
            ),
            "Name of module to hide",
        )
        .when(&key("hide-modules"), json!(true)),
        Definition::boolean(&key("custom-text"), "Add custom text", false),
        Definition::new(
            &key("text"),
            "Custom text",
            Kind::Text { max_chars: 128 },
            json!(""),
        )
        .when(&key("custom-text"), json!(true)),
        Definition::boolean(&key("custom-text-color"), "Set custom text color", false)
            .when(&key("custom-text"), json!(true)),
        color("text-color", "Color of custom text").when(&key("custom-text-color"), json!(true)),
    ]
}

/// A module's `guiColor` as opaque ARGB; `None` is colour 0 or a module the
/// reference never lists (HUD-group legit modules and `UtilityMod`s).
pub fn module_color(id: &str) -> Option<u32> {
    let rgb = match id {
        "world.auto-tool" => 0x62f442,
        "inventory.auto-armor" => 0x42cbf4,
        "inventory.auto-totem" => 0xff5555,
        "world.chest-steal" => 0xffff30,
        "world.fast-place" => 0x05ff20,
        "render.fullbright" => 0xffff00,
        "utility.auto-fish" => 0xbe00b5,
        "combat.sprint" => 0x00ff8c,
        "render.storage-esp" => 0x34df32,
        "render.esp" | "render.name-tags" | "combat.reach" => 0x00ff00,
        "render.tracers" => 0x44b2ea,
        "render.trajectories" => 0x03af93,
        "world.parkour" => 0x8a36bf,
        "combat.aim-assist" => 0xfb0006,
        "world.safe-walk" => 0x73fa3d,
        "combat.trigger-bot" | "combat.auto-clicker" | "combat.right-clicker" => 0xff0b11,
        "combat.velocity" => 0xff0084,
        "blatant.hitboxes" => 0x00ffe5,
        "blatant.timer" => 0xff1010,
        "blatant.fly" | "blatant.speed" => 0x00c1de,
        "blatant.no-fall" | "blatant.blink" | "world.fake-lag" | "blatant.regen" => 0x88193f,
        "blatant.no-slowdown" => 0xe242f4,
        "world.anti-afk" => 0x9542f4,
        "blatant.step" => 0x2aafe0,
        "combat.wtap" => 0xf5ce42,
        "blatant.long-jump" | "blatant.anti-fall" => 0xf49241,
        "render.item-esp" => 0x00be8b,
        "combat.no-item-release" => 0xdb1136,
        "blatant.potion-saver" | "render.anti-debuff" => 0xffff00,
        "combat.jump-reset" => 0x328fa8,
        "combat.knockback-delay" => 0x2070a0,
        "blatant.backtrack" => 0xff2020,
        "render.health" => 0xffadad,
        "render.motion-blur" => 0x3478f6,
        _ => return None,
    };
    Some(0xff000000 | rgb)
}

/// Java `DecimalFormat` with `places` optional fraction digits (HALF_EVEN).
pub fn decimal(value: f64, places: usize) -> String {
    let factor = 10f64.powi(places as i32);
    let scaled = value * factor;
    let rounded = {
        let floor = scaled.floor();
        let diff = scaled - floor;
        if (diff - 0.5).abs() < 1e-9 {
            if floor % 2.0 == 0.0 {
                floor
            } else {
                floor + 1.0
            }
        } else {
            scaled.round()
        }
    };
    let text = format!("{:.*}", places, rounded / factor);
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    };
    if text == "-0" { "0".into() } else { text }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub color: u32,
}

/// Legacy `§` formatting: colour codes switch colour, `r` resets, other
/// style codes are dropped.
pub fn spans(text: &str, base: u32) -> Vec<Span> {
    const PALETTE: [u32; 16] = [
        0x000000, 0x0000aa, 0x00aa00, 0x00aaaa, 0xaa0000, 0xaa00aa, 0xffaa00, 0xaaaaaa, 0x555555,
        0x5555ff, 0x55ff55, 0x55ffff, 0xff5555, 0xff55ff, 0xffff55, 0xffffff,
    ];
    let mut out: Vec<Span> = Vec::new();
    let mut color = base;
    let mut chars = text.chars();
    let mut current = String::new();
    while let Some(c) = chars.next() {
        if c == '\u{a7}' {
            let Some(code) = chars.next() else { break };
            let next = match code.to_ascii_lowercase() {
                d @ ('0'..='9' | 'a'..='f') => {
                    Some((base & 0xff000000) | PALETTE[d.to_digit(16).unwrap() as usize])
                }
                'r' => Some(base),
                _ => None,
            };
            if let Some(next) = next
                && next != color
            {
                if !current.is_empty() {
                    out.push(Span {
                        text: std::mem::take(&mut current),
                        color,
                    });
                }
                color = next;
            }
            continue;
        }
        current.push(c);
    }
    if !current.is_empty() {
        out.push(Span {
            text: current,
            color,
        });
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The module, or empty for the editor's example rows.
    pub id: &'static str,
    pub name: String,
    pub suffix: String,
    pub color: u32,
    /// Show/hide animation progress, 0..=1.
    pub progress: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub rows: Vec<Row>,
    pub alphabetical: bool,
    pub shadow: bool,
    pub background: bool,
    pub smooth: bool,
    pub watermark: bool,
    /// Custom text (with `&` codes already turned to `§`) and its colour.
    pub custom: Option<(String, u32)>,
    /// The colour of the watermark and editor examples.
    pub accent: u32,
    /// "Gui Color" rainbow: module rows step down in hue from the accent.
    pub rainbow: bool,
    /// The module row under the cursor while "Click disable" applies.
    pub hovered: Option<&'static str>,
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
    Text {
        x: f32,
        y: f32,
        /// Font size relative to the 8px game font line.
        size: f32,
        text: String,
        color: u32,
        shadow: Option<u32>,
    },
}

pub struct Layout {
    pub width: f32,
    pub height: f32,
    pub draws: Vec<Draw>,
    /// Each drawn module row's box (x, y, w, h), for "Click disable".
    pub rows: Vec<(&'static str, [f32; 4])>,
}

/// The hovered row's colour (`ColorUtil.calculatePerceivedBrightness`, then
/// `MutableColor.blend`): bright colours dim toward translucent gray, the
/// rest lighten toward white. Alpha blends like the other channels.
pub fn hover_blend(argb: u32) -> u32 {
    let channel = |c: u32, shift: u32| ((c >> shift) & 0xff) as f32;
    let [r, g, b] = [16, 8, 0].map(|shift| channel(argb, shift));
    let brightness = (r * r * 0.241 + g * g * 0.691 + b * b * 0.068).sqrt() as i32;
    let (with, f) = if brightness > 200 {
        (0x6464_6464u32, 0.1)
    } else {
        (0xffff_ffff, 0.2)
    };
    [24, 16, 8, 0].into_iter().fold(0, |out, shift| {
        let mixed = (channel(argb, shift) * (1.0 - f) + channel(with, shift) * f) as u32;
        out | mixed.min(255) << shift
    })
}

/// Name and suffix size factors: 16 and 14 (16 without smooth font) of the
/// reference's 16-unit module name font.
fn suffix_size(smooth: bool) -> f32 {
    if smooth { 14.0 / 16.0 } else { 1.0 }
}

/// Lay out the list at scale `s`, right-aligned to the box edge when
/// `right`. `measure(text, size)` is the width at `s * size`.
pub fn layout(view: &View, s: f32, right: bool, measure: impl Fn(&str, f32) -> f32) -> Layout {
    let suffix_scale = suffix_size(view.smooth);
    let plain = |text: &str| -> String {
        spans(text, 0)
            .into_iter()
            .map(|span| span.text)
            .collect::<String>()
    };
    let width_of = |row: &Row| {
        let name = measure(&row.name, s);
        if row.suffix.is_empty() {
            name
        } else {
            name + measure(&format!(" {}", plain(&row.suffix)), s * suffix_scale)
        }
    };
    let mut rows: Vec<(&Row, f32)> = view.rows.iter().map(|r| (r, width_of(r))).collect();
    if view.alphabetical {
        rows.sort_by_key(|a| a.0.name.to_lowercase());
    } else {
        // TextGuiModuleComparator: longest first; the sort is stable.
        rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    }
    let mut widest: f32 = 10.0;
    for (_, w) in &rows {
        widest = widest.max(*w);
    }
    let watermark_text = "vape v4";
    if view.watermark {
        widest = widest.max(60.0 * s).max(measure(watermark_text, s * 1.4));
    }
    let custom_size = s * 22.0 / 16.0;
    if let Some((text, _)) = &view.custom {
        widest = widest.max(measure(&plain(text), custom_size));
    }
    let width = widest + 7.0 * s + 2.0;
    let mut draws = Vec::new();
    let shadow = view.shadow.then_some(if view.background {
        0x50000000
    } else {
        0xaa000000
    });
    let mut y = 0.0;
    let text = |draws: &mut Vec<Draw>, x: f32, y: f32, size: f32, raw: &str, color: u32| {
        let mut x = x;
        for span in spans(raw, color) {
            let w = measure(&span.text, size);
            draws.push(Draw::Text {
                x,
                y,
                size,
                text: span.text,
                color: span.color,
                shadow,
            });
            x += w;
        }
    };
    let align = |w: f32| {
        if right { width - 4.0 * s - w } else { 4.0 * s }
    };
    if view.watermark {
        let w = measure(watermark_text, s * 1.4);
        text(&mut draws, align(w), y, s * 1.4, "vape", view.accent);
        let vape = measure("vape ", s * 1.4);
        text(&mut draws, align(w) + vape, y, s * 1.4, "v4", 0xffffffff);
        y += 15.0 * s;
    }
    if let Some((custom, color)) = &view.custom {
        let w = measure(&plain(custom), custom_size);
        text(&mut draws, align(w), y, custom_size, custom, *color);
        y += 8.0 * custom_size + 4.0 * s;
    }
    let row_height = 8.0 * s + 3.0 * s;
    y += 1.0 * s;
    let mut boxes = Vec::new();
    let [mut hue, saturation, brightness] = rgb_to_hsb(view.accent);
    for (row, w) in rows {
        let p = row.progress.clamp(0.0, 1.0);
        if p <= 0.0 {
            continue;
        }
        let mut color = row.color;
        if view.rainbow && !row.id.is_empty() {
            hue -= 0.025;
            if hue <= 0.0 {
                hue = 1.0;
            }
            color = (row.color & 0xff00_0000)
                | (readable_hsb(hue, saturation, brightness) & 0x00ff_ffff);
        }
        if !row.id.is_empty() && view.hovered == Some(row.id) {
            color = hover_blend(color);
        }
        let fade = |argb: u32| {
            let a = ((argb >> 24) as f32 * p).round() as u32;
            (a << 24) | (argb & 0x00ffffff)
        };
        let box_w = w + 7.0 * s;
        // Rows slide in from the anchored edge.
        let slide = (1.0 - p) * box_w * if right { 1.0 } else { -1.0 };
        let bx = if right { width - box_w } else { 0.0 } + slide;
        if view.background {
            draws.push(Draw::Fill {
                x: bx,
                y,
                w: box_w,
                h: row_height,
                color: fade(0xa0141414),
            });
            draws.push(Draw::Fill {
                x: bx,
                y: y + row_height - 1.0 * s,
                w: box_w,
                h: 1.0 * s,
                color: fade(0x281e1e1e),
            });
        }
        if !row.id.is_empty() {
            boxes.push((row.id, [bx, y, box_w, row_height]));
        }
        let bar_x = if right { bx + box_w - 1.5 * s } else { bx };
        draws.push(Draw::Fill {
            x: bar_x,
            y,
            w: 1.5 * s,
            h: row_height,
            color: fade(color),
        });
        let tx = if right {
            bx + box_w - 4.0 * s - w
        } else {
            bx + 4.0 * s
        };
        let ty = y + 1.5 * s;
        let name_w = measure(&row.name, s);
        text(&mut draws, tx, ty, s, &row.name, fade(color));
        if !row.suffix.is_empty() {
            let sy = ty + (8.0 * s - 8.0 * s * suffix_scale) * 0.75;
            text(
                &mut draws,
                tx + name_w,
                sy,
                s * suffix_scale,
                &format!(" {}", row.suffix),
                fade(SUFFIX_GRAY),
            );
        }
        y += row_height;
    }
    Layout {
        width,
        height: y.max(10.0 * s),
        draws,
        rows: boxes,
    }
}

/// Per-module show/hide progress: ±0.08 per 10 ms (`TextGuiModuleRenderState`).
#[derive(Debug, Default)]
pub struct Animator {
    progress: std::collections::BTreeMap<&'static str, f32>,
}

impl Animator {
    /// Advance by `elapsed_ms` toward each module's shown state; modules not
    /// in `shown` fade out and are then forgotten.
    pub fn update(&mut self, shown: &[&'static str], elapsed_ms: f32, animate: bool) {
        let step = if animate {
            (elapsed_ms / 10.0 * 0.08).max(0.0)
        } else {
            1.0
        };
        for id in shown {
            self.progress.entry(id).or_insert(0.0);
        }
        self.progress.retain(|id, p| {
            if shown.contains(id) {
                *p = (*p + step).min(1.0);
                true
            } else {
                *p = (*p - step).max(0.0);
                *p > 0.0
            }
        });
    }
    pub fn progress(&self, id: &str) -> f32 {
        self.progress.get(id).copied().unwrap_or(0.0)
    }
    /// Show a row at once at full progress (a click-enabled module).
    pub fn show(&mut self, id: &'static str) {
        self.progress.insert(id, 1.0);
    }
    /// Drop a row without fading it (click-disabled rows on GUI close).
    pub fn forget(&mut self, id: &str) {
        self.progress.remove(id);
    }
    pub fn fading(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.progress.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(text: &str, size: f32) -> f32 {
        text.chars().count() as f32 * 6.0 * size
    }

    fn row(name: &str, suffix: &str) -> Row {
        Row {
            id: "",
            name: name.into(),
            suffix: suffix.into(),
            color: 0xff00ff00,
            progress: 1.0,
        }
    }

    fn names(layout: &Layout) -> Vec<String> {
        layout
            .draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text { text, color, .. } if *color != SUFFIX_GRAY => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn length_sort_puts_the_widest_row_with_its_suffix_first() {
        let view = View {
            rows: vec![row("Fly", ""), row("Speed", "Bhop"), row("AutoClicker", "")],
            smooth: false,
            ..View::default()
        };
        assert_eq!(
            names(&layout(&view, 1.0, true, measure)),
            ["AutoClicker", "Speed", "Fly"]
        );
        let view = View {
            alphabetical: true,
            ..view
        };
        assert_eq!(
            names(&layout(&view, 1.0, true, measure)),
            ["AutoClicker", "Fly", "Speed"]
        );
    }

    #[test]
    fn right_anchored_rows_end_at_the_box_edge_with_a_bar() {
        let view = View {
            rows: vec![row("Fly", "")],
            background: true,
            ..View::default()
        };
        let out = layout(&view, 1.0, true, measure);
        // max(10, 18) + 7 + 2
        assert_eq!(out.width, 27.0);
        let bar = out
            .draws
            .iter()
            .find(|d| {
                matches!(
                    d,
                    Draw::Fill {
                        color: 0xff00ff00,
                        ..
                    }
                )
            })
            .unwrap();
        assert_eq!(
            *bar,
            Draw::Fill {
                x: 27.0 - 1.5,
                y: 1.0,
                w: 1.5,
                h: 11.0,
                color: 0xff00ff00
            }
        );
        let left = layout(&view, 1.0, false, measure);
        assert!(left.draws.iter().any(|d| matches!(
            d,
            Draw::Fill { x, color: 0xff00ff00, .. } if *x == 0.0
        )));
    }

    #[test]
    fn colour_codes_split_spans_and_decimal_matches_java_format() {
        assert_eq!(
            spans("\u{a7}cHolding", 0xff969696),
            [Span {
                text: "Holding".into(),
                color: 0xffff5555
            }]
        );
        assert_eq!(spans("a\u{a7}lb", 1).len(), 1);
        assert_eq!(decimal(3.0, 2), "3");
        assert_eq!(decimal(3.1, 2), "3.1");
        assert_eq!(decimal(6.25, 1), "6.2");
        assert_eq!(decimal(13.0, 1), "13");
    }

    #[test]
    fn animation_fades_rows_in_and_out_at_the_reference_rate() {
        let mut a = Animator::default();
        a.update(&["combat.reach"], 50.0, true);
        assert!((a.progress("combat.reach") - 0.4).abs() < 1e-6);
        a.update(&["combat.reach"], 100.0, true);
        assert_eq!(a.progress("combat.reach"), 1.0);
        a.update(&[], 60.0, true);
        assert!((a.progress("combat.reach") - 0.52).abs() < 1e-6);
        a.update(&[], 100.0, true);
        assert_eq!(a.fading().count(), 0);
        a.update(&["blatant.fly"], 1.0, false);
        assert_eq!(a.progress("blatant.fly"), 1.0);
    }

    #[test]
    fn colour_table_excludes_utility_and_hud_modules() {
        assert_eq!(module_color("combat.auto-clicker"), Some(0xffff0b11));
        assert_eq!(module_color("utility.panic"), None);
        assert_eq!(module_color("inventory.refill"), None);
        assert_eq!(module_color("hud.fps"), None);
        assert_eq!(module_color("render.no-hurt-camera"), None);
    }

    #[test]
    fn hover_blend_darkens_bright_rows_and_lightens_dark_ones() {
        // Perceived brightness 255 > 200: 10% toward (100, 100, 100, 100).
        assert_eq!(hover_blend(0xffff_ffff), 0xefef_efef);
        // Black: 20% toward white, alpha included.
        assert_eq!(hover_blend(0xff00_0000), 0xff33_3333);
    }

    #[test]
    fn module_rows_report_boxes_and_rainbow_steps_their_hue() {
        let bars = |view: &View| -> Vec<u32> {
            layout(view, 1.0, true, measure)
                .draws
                .iter()
                .filter_map(|d| match d {
                    Draw::Fill { w, color, .. } if *w == 1.5 => Some(*color),
                    _ => None,
                })
                .collect()
        };
        let module = |id: &'static str, name: &str| Row {
            id,
            ..row(name, "")
        };
        let mut view = View {
            rows: vec![module("a", "Long row"), module("b", "Row"), row("Ex", "")],
            accent: 0xff05_8669,
            ..View::default()
        };
        let out = layout(&view, 1.0, true, measure);
        let ids: Vec<_> = out.rows.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, ["a", "b"], "examples get no click box");
        let [bx, y, w, h] = out.rows[1].1;
        assert_eq!((y, h), (12.0, 11.0));
        assert_eq!(bx + w, out.width, "right-anchored");
        assert_eq!(bars(&view), [0xff00ff00; 3]);
        view.rainbow = true;
        let rainbow = bars(&view);
        let [accent, ..] = rgb_to_hsb(0xff05_8669);
        let hues: Vec<f32> = rainbow.iter().map(|c| rgb_to_hsb(*c)[0]).collect();
        assert!((hues[0] - (accent - 0.025)).abs() < 0.02, "{hues:?}");
        assert!((hues[1] - (accent - 0.05)).abs() < 0.02, "{hues:?}");
        assert_eq!(rainbow[2], 0xff00ff00, "examples keep their colour");
        view.hovered = Some("b");
        let hovered = bars(&view);
        assert_eq!(hovered[0], rainbow[0]);
        assert_eq!(hovered[1], hover_blend(rainbow[1]));
    }
}
