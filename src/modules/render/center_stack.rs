// The active-module stack (`ActiveModuleStackFrame`): centre-screen
// readouts of the modules that publish display info (Health, AutoTotem's
// totem count). Entries stack downward from 10 below the screen centre; with
// more than one entry each label gets its module's name as a grey suffix,
// and the label alone stays centred (the reference measures the label, then
// draws label plus suffix from that left edge).
use super::overlay::{Align, Draw, Frame};

/// One module's display info.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub module: &'static str,
    pub label: String,
    /// ARGB.
    pub color: u32,
}

/// The reference font's line height at the stack's scale, plus its 4 gap.
const LINE: f32 = 9.0 + 4.0;

pub fn draws(frame: &Frame, entries: &[Entry]) -> Vec<Draw> {
    let p = &frame.projection;
    let named = entries.len() > 1;
    let mut y = p.height / 2.0 + 10.0;
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut text = entry.label.clone();
        if named {
            text.push_str(" \u{a7}7(");
            text.push_str(entry.module);
            text.push(')');
        }
        out.push(Draw::Text {
            x: p.width / 2.0,
            y,
            scale: 1.0,
            text,
            color: entry.color,
            shadow: true,
            align: Align::CenterOn(entry.label.len()),
            backdrop: None,
        });
        y += LINE;
    }
    out
}

/// AutoTotem's display info: the count, red below 2, yellow below 4, else
/// green.
pub fn totem_entry(count: u32) -> Entry {
    let [r, g, b] = super::super::utility::auto_totem::count_color(count);
    Entry {
        module: "AutoTotem",
        label: count.to_string(),
        color: 0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::render::overlay::{Own, Projection};

    fn frame() -> Frame {
        Frame {
            projection: Projection {
                camera: [0.0; 3],
                matrix: [0.0; 16],
                width: 400.0,
                height: 300.0,
            },
            entities: Vec::new(),
            own: Own::default(),
            first_person: true,
            now_ms: 0,
            spawners: Vec::new(),
        }
    }

    #[test]
    fn one_entry_is_bare_and_several_get_module_names() {
        let health = Entry {
            module: "Health",
            label: "10 \u{a7}c\u{2764}".to_owned(),
            color: 0xff02_be3a,
        };
        let alone = draws(&frame(), std::slice::from_ref(&health));
        let Draw::Text {
            x, y, text, align, ..
        } = &alone[0]
        else {
            panic!("text")
        };
        assert_eq!(
            (*x, *y, text.as_str()),
            (200.0, 160.0, "10 \u{a7}c\u{2764}")
        );
        assert_eq!(*align, Align::CenterOn(health.label.len()));
        let both = draws(&frame(), &[health, totem_entry(3)]);
        let texts: Vec<_> = both
            .iter()
            .map(|d| match d {
                Draw::Text { y, text, color, .. } => (*y, text.clone(), *color),
                _ => panic!("text"),
            })
            .collect();
        assert_eq!(texts[0].1, "10 \u{a7}c\u{2764} \u{a7}7(Health)");
        assert_eq!(
            texts[1],
            (173.0, "3 \u{a7}7(AutoTotem)".to_owned(), 0xffff_f912)
        );
    }

    #[test]
    fn totem_colours_follow_the_count() {
        assert_eq!(totem_entry(1).color, 0xffff_1414);
        assert_eq!(totem_entry(2).color, 0xffff_f912);
        assert_eq!(totem_entry(4).color, 0xff02_be3a);
    }
}
