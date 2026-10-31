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
fn hsb_to_rgb(hue: f32, saturation: f32, brightness: f32) -> u32 {
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
fn rgb_to_hsb(argb: u32) -> [f32; 3] {
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
}
