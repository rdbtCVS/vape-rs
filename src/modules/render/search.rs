// Search (`Search`): an outline around every listed block within range,
// drawn through walls, optionally only blocks touching air ("Only caves")
// and with a tracer to each. The reference's Search frame holds each block
// with its own colour; here each "Blocks" entry is a block id with an
// optional `=AARRGGBB` colour (white without one). The host scans loaded
// chunks a few per frame (`SearchBlockChunkScanner.scanLoadedChunks`) and
// hands back the matches; the reference's per-block tracer toggle is not
// ported, "Use tracers" applies to every block.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::esp::Camera;
use super::world::{Line, Point, outline_box};
use serde_json::{Value, json};

pub const ID: &str = "render.search";

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::new(
            &key("blocks"),
            "Blocks",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!([]),
        ),
        Definition::number(&key("range"), "Range", 1.0, 50.0, 100.0, 1.0),
        Definition::boolean(&key("only-caves"), "Only caves", false),
        Definition::boolean(&key("use-tracers"), "Use tracers", false),
    ]
}

/// What the host scans for: block ids (no namespace) with their colours,
/// the horizontal range in blocks, and whether a block must touch air.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub blocks: Vec<(String, u32)>,
    pub range: i32,
    pub only_caves: bool,
}

impl Query {
    /// The colour of a block id, when it is searched for.
    pub fn color(&self, block: &str) -> Option<u32> {
        let block = block.trim_start_matches("minecraft:");
        self.blocks.iter().find(|(b, _)| b == block).map(|b| b.1)
    }
}

/// One "Blocks" entry: `id` or `id=AARRGGBB`; a leading `!` disables it.
fn entry(text: &str) -> Option<(String, u32)> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('!') {
        return None;
    }
    let (id, color) = match text.split_once('=') {
        Some((id, hex)) => (
            id,
            u32::from_str_radix(hex.trim().trim_start_matches('#'), 16).ok()?,
        ),
        None => (text, 0xffff_ffff),
    };
    let id = id.trim().trim_start_matches("minecraft:").to_lowercase();
    (!id.is_empty()).then_some((id, color))
}

pub fn query(settings: &Settings) -> Query {
    Query {
        blocks: settings
            .get(&key("blocks"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(entry)
            .collect(),
        range: settings
            .get(&key("range"))
            .and_then(Value::as_f64)
            .unwrap_or(50.0) as i32,
        only_caves: settings
            .get(&key("only-caves"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// A unit box per found block in its colour (`RenderUtil.u` with no
/// fill) and, with "Use tracers", a 2-wide line from just ahead of the
/// eye to the block's centre.
pub fn lines(
    settings: &Settings,
    camera: &Camera,
    found: &[([i32; 3], String)],
    out: &mut Vec<Line>,
) {
    let q = query(settings);
    let tracers = settings
        .get(&key("use-tracers"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    for (pos, block) in found {
        let Some(color) = q.color(block) else {
            continue;
        };
        let min = pos.map(f64::from);
        outline_box(out, Point(min), Point(min.map(|v| v + 1.0)), color, 1.5);
        if tracers {
            out.push(Line {
                start: Point(std::array::from_fn(|i| camera.eye[i] + camera.look[i])),
                end: Point(min.map(|v| v + 0.5)),
                color,
                width: 2.0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_parse_ids_and_colours() {
        assert_eq!(
            entry("minecraft:Diamond_Ore"),
            Some(("diamond_ore".into(), 0xffff_ffff))
        );
        assert_eq!(
            entry("gold_ore=#ffffd700"),
            Some(("gold_ore".into(), 0xffff_d700))
        );
        assert_eq!(entry("!iron_ore"), None);
        assert_eq!(entry("iron_ore=zz"), None);
        let q = Query {
            blocks: vec![("gold_ore".into(), 1)],
            range: 50,
            only_caves: false,
        };
        assert_eq!(q.color("minecraft:gold_ore"), Some(1));
        assert_eq!(q.color("stone"), None);
    }
}
