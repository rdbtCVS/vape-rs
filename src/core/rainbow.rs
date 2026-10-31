// Per-colour rainbow (`ColorValue.getMutableColor`): while a colour's
// "<name> rainbow" is on, its hue advances by ClientSettings "Rainbow speed"
// at most every 50 ms and wraps to 0 past 255. The cycled colour is written
// back without marking the profile dirty (`setPersistenceSuppressed`), so a
// later save stores wherever the hue is, as the reference's does.
use super::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const STEP: Duration = Duration::from_millis(50);

#[derive(Debug, Default)]
pub(super) struct Rainbows(HashMap<String, Cycle>);

#[derive(Debug)]
struct Cycle {
    /// Hue, saturation and brightness on the reference's 0..255 scales.
    hsb: [f64; 3],
    alpha: u32,
    /// The ARGB last written, to notice edits and profile loads.
    written: u32,
    /// `lastRainbowAdvance`: per colour, kept across edits.
    last: Option<Instant>,
}

impl Cycle {
    /// `ColorValue.setColor`: the HSB components of an edited colour.
    fn seed(&mut self, argb: u32) {
        let [h, s, b] = rgb_to_hsb(argb);
        self.hsb = [h as f64 * 255.0, s as f64 * 255.0, b as f64 * 255.0];
        self.alpha = argb >> 24;
        self.written = argb;
    }
    fn argb(&self) -> u32 {
        let [h, s, b] = self.hsb.map(|c| (c / 255.0) as f32);
        self.alpha << 24 | (hsb_to_rgb(h, s, b) & 0x00ff_ffff)
    }
}

impl Runtime {
    pub(super) fn advance_rainbows(&mut self, now: Instant) {
        let speed = self
            .settings
            .get("client.rainbow-speed")
            .and_then(Value::as_f64)
            .unwrap_or(1.0);
        let ids: Vec<String> = self.settings.rainbow_colors().map(str::to_owned).collect();
        self.rainbows.0.retain(|id, _| ids.contains(id));
        for id in ids {
            let Some(argb) = self.settings.get(&id).and_then(Value::as_u64) else {
                continue;
            };
            let argb = argb as u32;
            let cycle = self.rainbows.0.entry(id.clone()).or_insert_with(|| {
                let mut cycle = Cycle {
                    hsb: [0.0; 3],
                    alpha: 0,
                    written: !argb,
                    last: None,
                };
                cycle.seed(argb);
                cycle
            });
            if cycle.written != argb {
                cycle.seed(argb);
            }
            if cycle
                .last
                .is_some_and(|last| now.duration_since(last) < STEP)
            {
                continue;
            }
            cycle.last = Some(now);
            // `advanceRainbowHue`.
            cycle.hsb[0] += speed;
            if !(0.0..=255.0).contains(&cycle.hsb[0]) {
                cycle.hsb[0] = 0.0;
            }
            cycle.written = cycle.argb();
            let _ = self.settings.set(&id, json!(cycle.written));
        }
    }
}

/// `java.awt.Color.HSBtoRGB`, opaque.
pub(crate) fn hsb_to_rgb(hue: f32, saturation: f32, brightness: f32) -> u32 {
    let channel = |v: f32| (v * 255.0 + 0.5) as u32;
    let (r, g, b) = if saturation == 0.0 {
        let v = channel(brightness);
        (v, v, v)
    } else {
        let h = (hue - hue.floor()) * 6.0;
        let f = h - h.floor();
        let p = brightness * (1.0 - saturation);
        let q = brightness * (1.0 - saturation * f);
        let t = brightness * (1.0 - saturation * (1.0 - f));
        let (r, g, b) = match h as u32 {
            0 => (brightness, t, p),
            1 => (q, brightness, p),
            2 => (p, brightness, t),
            3 => (p, q, brightness),
            4 => (t, p, brightness),
            _ => (brightness, p, q),
        };
        (channel(r), channel(g), channel(b))
    };
    0xff00_0000 | r << 16 | g << 8 | b
}

/// `java.awt.Color.RGBtoHSB` of an ARGB colour's RGB.
pub(crate) fn rgb_to_hsb(argb: u32) -> [f32; 3] {
    let [r, g, b] = [16, 8, 0].map(|shift| ((argb >> shift) & 0xff) as f32);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let brightness = max / 255.0;
    let saturation = if max != 0.0 { (max - min) / max } else { 0.0 };
    let hue = if saturation == 0.0 {
        0.0
    } else {
        let [rc, gc, bc] = [r, g, b].map(|c| (max - c) / (max - min));
        let hue = if r == max {
            bc - gc
        } else if g == max {
            2.0 + rc - bc
        } else {
            4.0 + gc - rc
        } / 6.0;
        if hue < 0.0 { hue + 1.0 } else { hue }
    };
    [hue, saturation, brightness]
}

/// `ColorUtil.createReadableHsbColor(h, s, b)`: the HSB colour, lightened
/// by bisection (15 steps) until its WCAG contrast against the GUI's
/// (45, 45, 45) exceeds 4 (`adjustHslForContrast`).
pub(crate) fn readable_hsb(hue: f32, saturation: f32, brightness: f32) -> u32 {
    const BACKGROUND: u32 = 0xff2d_2d2d;
    const MINIMUM: f64 = 4.0;
    let color = hsb_to_rgb(hue, saturation, brightness);
    if contrast(color, BACKGROUND) >= MINIMUM {
        return color;
    }
    let mut hsl = rgb_to_hsl(color);
    let (mut lower, mut upper) = (hsl[2], 1.0f32);
    let mut current = color;
    for _ in 0..15 {
        if upper - lower <= 1.0e-5 {
            break;
        }
        hsl[2] = (lower + upper) / 2.0;
        current = hsl_to_rgb(hsl);
        if contrast(current, BACKGROUND) > MINIMUM {
            upper = hsl[2];
        } else {
            lower = hsl[2];
        }
    }
    current
}

/// `ColorContrastUtil.calculateContrast` of two opaque colours.
fn contrast(a: u32, b: u32) -> f64 {
    let luminance = |argb: u32| {
        let [r, g, b] = [16, 8, 0].map(|shift| {
            let c = ((argb >> shift) & 0xff) as f64 / 255.0;
            if c < 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        });
        // The Y of `rgbToXyz`, over its 100 white.
        r * 0.2126 + g * 0.7152 + b * 0.0722 + 0.05
    };
    let (a, b) = (luminance(a), luminance(b));
    a.max(b) / a.min(b)
}

/// `ColorContrastUtil.rgbToHsl`: hue in degrees, saturation and lightness 0..1.
fn rgb_to_hsl(argb: u32) -> [f32; 3] {
    let [r, g, b] = [16, 8, 0].map(|shift| ((argb >> shift) & 0xff) as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let chroma = max - min;
    let lightness = (max + min) / 2.0;
    let (hue, saturation) = if max == min {
        (0.0, 0.0)
    } else {
        let hue = if max == r {
            ((g - b) / chroma) % 6.0
        } else if max == g {
            (b - r) / chroma + 2.0
        } else {
            (r - g) / chroma + 4.0
        };
        (hue, chroma / (1.0 - (2.0 * lightness - 1.0).abs()))
    };
    let mut hue = (hue * 60.0) % 360.0;
    if hue < 0.0 {
        hue += 360.0;
    }
    [
        hue.clamp(0.0, 360.0),
        saturation.clamp(0.0, 1.0),
        lightness.clamp(0.0, 1.0),
    ]
}

/// `ColorContrastUtil.hslToColor`, opaque.
fn hsl_to_rgb([hue, saturation, lightness]: [f32; 3]) -> u32 {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let m = lightness - 0.5 * chroma;
    let x = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match hue as i32 / 60 {
        0 => (chroma + m, x + m, m),
        1 => (x + m, chroma + m, m),
        2 => (m, chroma + m, x + m),
        3 => (m, x + m, chroma + m),
        4 => (x + m, m, chroma + m),
        5 | 6 => (chroma + m, m, x + m),
        _ => (0.0, 0.0, 0.0),
    };
    let channel = |v: f32| ((255.0 * v).round() as i32).clamp(0, 255) as u32;
    0xff00_0000 | channel(r) << 16 | channel(g) << 8 | channel(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLOR: &str = "render.arrows.color";

    fn color(r: &Runtime) -> u32 {
        r.settings.get(COLOR).and_then(Value::as_u64).unwrap() as u32
    }

    #[test]
    fn java_hsb_conversion_round_trips_primaries() {
        assert_eq!(hsb_to_rgb(0.0, 1.0, 1.0), 0xffff_0000);
        assert_eq!(hsb_to_rgb(1.0 / 3.0, 1.0, 1.0), 0xff00_ff00);
        assert_eq!(hsb_to_rgb(0.5, 0.0, 0.5), 0xff80_8080);
        assert_eq!(rgb_to_hsb(0xff00_00ff), [2.0 / 3.0, 1.0, 1.0]);
        // The Text GUI default, 206/7/7.
        let [h, s, b] = rgb_to_hsb(0xffce_0707);
        assert_eq!(hsb_to_rgb(h, s, b), 0xffce_0707);
    }

    #[test]
    fn rainbow_advances_by_speed_every_50_ms_without_dirtying_the_profile() {
        let mut r = Runtime::default();
        let start = Instant::now();
        r.advance_rainbows(start);
        assert_eq!(color(&r), 0xffff_0000, "off: untouched");
        r.settings
            .set(&format!("{COLOR}.rainbow"), json!(true))
            .unwrap();
        r.settings.set("client.rainbow-speed", json!(10.0)).unwrap();
        r.dirty = false;
        r.advance_rainbows(start);
        let first = color(&r);
        assert_eq!(first >> 24, 0xff);
        assert_ne!(first, 0xffff_0000);
        r.advance_rainbows(start + Duration::from_millis(30));
        assert_eq!(color(&r), first, "throttled to 50 ms");
        // 26 steps of 10 pass 255 and wrap the hue back to red.
        for step in 1..26 {
            r.advance_rainbows(start + STEP * step);
        }
        assert_eq!(r.rainbows.0[COLOR].hsb[0], 0.0);
        assert_eq!(color(&r), 0xffff_0000);
        assert!(!r.dirty);
    }

    #[test]
    fn an_edited_colour_reseeds_the_cycle_and_keeps_its_alpha() {
        let mut r = Runtime::default();
        let start = Instant::now();
        r.settings
            .set(&format!("{COLOR}.rainbow"), json!(true))
            .unwrap();
        r.advance_rainbows(start);
        r.settings.set(COLOR, json!(0x8000_00ffu32)).unwrap();
        r.advance_rainbows(start + STEP);
        let cycle = &r.rainbows.0[COLOR];
        assert_eq!(cycle.alpha, 0x80);
        assert!((cycle.hsb[0] - 171.0).abs() < 1e-3, "{}", cycle.hsb[0]);
        assert_eq!(color(&r) >> 24, 0x80);
        // Turning it off drops the cycle and leaves the colour where it was.
        r.settings
            .set(&format!("{COLOR}.rainbow"), json!(false))
            .unwrap();
        let kept = color(&r);
        r.advance_rainbows(start + STEP * 2);
        assert!(r.rainbows.0.is_empty());
        assert_eq!(color(&r), kept);
    }

    #[test]
    fn readable_hsb_lightens_dark_hues_against_the_gui_background() {
        // Bright yellow already contrasts with (45, 45, 45).
        assert_eq!(
            super::readable_hsb(1.0 / 6.0, 0.9, 1.0),
            hsb_to_rgb(1.0 / 6.0, 0.9, 1.0)
        );
        // Pure blue is too dark and comes back lighter, still blue.
        let blue = super::readable_hsb(2.0 / 3.0, 0.9, 1.0);
        assert_ne!(blue, hsb_to_rgb(2.0 / 3.0, 0.9, 1.0));
        assert!(super::contrast(blue, 0xff2d_2d2d) > 4.0);
        let [h, ..] = rgb_to_hsb(blue);
        assert!((h - 2.0 / 3.0).abs() < 0.02);
    }
}
