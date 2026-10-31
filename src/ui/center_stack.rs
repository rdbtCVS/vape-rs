// The centre-screen readout stack (`ActiveModuleStackFrame`): modules that
// report a `ModuleDisplayInfo` (Health on this port) are drawn centred just
// below the crosshair with the 1.5 smooth font. Pure: the host measures and
// paints the `Draw` list.

use super::text_gui::{Draw, spans};

/// `ModuleDisplayInfo(label, color, description, suffix)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub module: &'static str,
    pub label: String,
    pub color: u32,
    /// Text the entry is centred by, when it differs from the label.
    pub description: Option<String>,
    /// Shown after the label when several entries stack; defaults to the
    /// grey module name.
    pub suffix: Option<String>,
}

/// The smooth font size the frame uses (`getFontManager().p(1.5)`).
pub const SIZE: f32 = 1.5;
/// Smooth font line height at size 1, in HUD units.
const LINE: f32 = 9.0;

/// `Health.getModuleDisplayInfo`: hearts plus absorption, rounded to the
/// nearest half heart, a gold heart with absorption and a red one without.
pub fn health(health: f32, absorption: f32) -> Entry {
    let mut hearts = f64::from(health) / 2.0;
    let heart = if absorption > 0.0 {
        hearts += f64::from(absorption) / 2.0;
        "\u{a7}6\u{2764}"
    } else {
        "\u{a7}c\u{2764}"
    };
    let rounded = ((hearts + 0.25) / 0.5).floor() * 0.5;
    // NumberFormat("#.#") then `.replace(".0", "")`.
    let text = if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    };
    let rgb = if hearts >= 7.0 {
        (2, 190, 58)
    } else if hearts > 4.0 {
        (255, 249, 18)
    } else {
        (255, 20, 20)
    };
    Entry {
        module: "Health",
        label: format!("{text} {heart}"),
        color: 0xff000000 | (rgb.0 << 16) | (rgb.1 << 8) | rgb.2,
        description: Some(text),
        suffix: None,
    }
}

/// `renderStack`: entries from the screen centre, 10 units down, each
/// centred by its description and spaced by the line height plus 4.
pub fn layout(
    entries: &[Entry],
    screen: (f32, f32),
    measure: impl Fn(&str, f32) -> f32,
) -> Vec<Draw> {
    let plain = |text: &str| {
        spans(text, 0)
            .into_iter()
            .map(|s| s.text)
            .collect::<String>()
    };
    let center_x = screen.0 / 2.0;
    let mut y = screen.1 / 2.0 + 10.0;
    let named = entries.len() > 1;
    let mut draws = Vec::new();
    for entry in entries {
        let width_text = entry.description.as_deref().unwrap_or(&entry.label);
        let width = measure(&plain(width_text), SIZE);
        let mut text = entry.label.clone();
        if named {
            match &entry.suffix {
                Some(suffix) => text += suffix,
                None => text += &format!(" \u{a7}7({})", entry.module),
            }
        }
        let mut x = center_x - (width / 2.0).ceil() + 1.0;
        for span in spans(&text, entry.color) {
            let w = measure(&span.text, SIZE);
            draws.push(Draw::Text {
                x,
                y,
                size: SIZE,
                text: span.text,
                color: span.color,
                shadow: None,
            });
            x += w;
        }
        y += LINE * SIZE + 4.0;
    }
    draws
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_rounds_to_half_hearts_and_colours_by_threshold() {
        let full = health(20.0, 0.0);
        assert_eq!(full.label, "10 \u{a7}c\u{2764}");
        assert_eq!(full.color, 0xff02be3a);
        assert_eq!(full.description.as_deref(), Some("10"));
        // 13 hp = 6.5 hearts: yellow; 13.4 rounds to 6.5, 13.6 to 7.
        assert_eq!(health(13.0, 0.0).label, "6.5 \u{a7}c\u{2764}");
        assert_eq!(health(13.0, 0.0).color, 0xfffff912);
        assert_eq!(health(13.6, 0.0).description.as_deref(), Some("7"));
        assert_eq!(health(8.0, 0.0).color, 0xffff1414);
        // Absorption adds to the hearts and turns the heart gold.
        let gold = health(10.0, 4.0);
        assert_eq!(gold.label, "7 \u{a7}6\u{2764}");
        assert_eq!(gold.color, 0xff02be3a);
    }

    #[test]
    fn stack_centres_by_description_and_names_only_when_stacked() {
        let measure = |s: &str, size: f32| s.chars().count() as f32 * 6.0 * size;
        let one = layout(&[health(20.0, 0.0)], (400.0, 300.0), measure);
        // "10" is 18 wide: x = 200 - 9 + 1.
        assert!(matches!(&one[0], Draw::Text { x, y, text, .. }
            if *x == 192.0 && *y == 160.0 && text == "10 "));
        assert!(matches!(
            &one[1],
            Draw::Text {
                color: 0xffff5555,
                ..
            }
        ));
        let two = layout(
            &[health(20.0, 0.0), health(2.0, 0.0)],
            (400.0, 300.0),
            measure,
        );
        assert!(two.iter().any(|d| matches!(d, Draw::Text { text, y, .. }
            if text == "(Health)" && *y == 160.0 + 17.5)));
    }
}
