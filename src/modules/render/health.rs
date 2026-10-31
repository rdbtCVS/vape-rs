// Health (`Health`): your health in hearts under the crosshair, in the
// active-module stack (`ActiveModuleStackFrame`: centred, 10 below the
// screen centre).
use super::overlay::{Align, Draw, Frame};

/// The label and its colour: hearts rounded to halves with a trailing `.0`
/// dropped, then a heart, gold with absorption and red otherwise.
pub fn label(health: f32, absorption: f32) -> (String, u32) {
    let mut hearts = f64::from(health) / 2.0;
    let heart = if absorption > 0.0 {
        hearts += f64::from(absorption) / 2.0;
        "\u{a7}6\u{2764}"
    } else {
        "\u{a7}c\u{2764}"
    };
    let rounded = ((hearts + 0.25) / 0.5).floor() * 0.5;
    let number = format!("{rounded:.1}").replace(".0", "");
    let color = if hearts >= 7.0 {
        0xff02_be3a
    } else if hearts > 4.0 {
        0xffff_f912
    } else {
        0xffff_1414
    };
    (format!("{number} {heart}"), color)
}

pub fn draws(frame: &Frame) -> Vec<Draw> {
    let (text, color) = label(frame.own.health, frame.own.absorption);
    let p = &frame.projection;
    vec![Draw::Text {
        x: p.width / 2.0,
        y: p.height / 2.0 + 10.0,
        scale: 1.0,
        text,
        color,
        shadow: true,
        align: Align::Center,
        backdrop: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::label;

    #[test]
    fn hearts_round_to_halves() {
        assert_eq!(
            label(20.0, 0.0),
            ("10 \u{a7}c\u{2764}".to_owned(), 0xff02_be3a)
        );
        assert_eq!(label(13.0, 0.0).0, "6.5 \u{a7}c\u{2764}");
        assert_eq!(label(9.0, 0.0).1, 0xffff_f912);
        assert_eq!(label(8.0, 0.0).1, 0xffff_1414);
        assert_eq!(
            label(6.0, 4.0),
            ("5 \u{a7}6\u{2764}".to_owned(), 0xffff_f912)
        );
        assert_eq!(label(7.4, 0.0).0, "3.5 \u{a7}c\u{2764}");
    }
}
