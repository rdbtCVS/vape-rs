// Search (`Search`): an outline around every listed block within range,
// drawn through walls, optionally only blocks touching air ("Only caves")
// and with a tracer to each. The reference's Search frame holds each block
// with its own colour, enabled toggle and tracer toggle (`SearchBlock`,
// tracers off by default); here each "Blocks" entry is a block id with an
// optional `=AARRGGBB` colour (white without one), a leading `!` turning it
// off and a trailing `+tracer` turning its tracer on. A block's tracer is
// drawn only with "Use tracers" on too (`Search.onRender` with
// `SearchResultData.n`). The host scans loaded chunks a few per frame
// (`SearchBlockChunkScanner.scanLoadedChunks`) and hands back the matches.
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

/// One searched block: its id (no namespace), colour and whether it has a
/// tracer.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub id: String,
    pub color: u32,
    pub tracer: bool,
}

/// What the host scans for: the blocks, the horizontal range in blocks, and
/// whether a block must touch air.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub blocks: Vec<Block>,
    pub range: i32,
    pub only_caves: bool,
}

impl Query {
    fn block(&self, block: &str) -> Option<&Block> {
        let block = block.trim_start_matches("minecraft:");
        self.blocks.iter().find(|b| b.id == block)
    }
    /// The colour of a block id, when it is searched for.
    pub fn color(&self, block: &str) -> Option<u32> {
        self.block(block).map(|b| b.color)
    }
}

/// One "Blocks" entry: `id` or `id=AARRGGBB`, then optionally `+tracer`; a
/// leading `!` disables it.
fn entry(text: &str) -> Option<Block> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('!') {
        return None;
    }
    let (text, tracer) = match text.len().checked_sub(7) {
        Some(at) if text.is_char_boundary(at) && text[at..].eq_ignore_ascii_case("+tracer") => {
            (text[..at].trim_end(), true)
        }
        _ => (text, false),
    };
    let (id, color) = match text.split_once('=') {
        Some((id, hex)) => (
            id,
            u32::from_str_radix(hex.trim().trim_start_matches('#'), 16).ok()?,
        ),
        None => (text, 0xffff_ffff),
    };
    let id = id.trim().trim_start_matches("minecraft:").to_lowercase();
    (!id.is_empty()).then_some(Block { id, color, tracer })
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
        let Some(&Block { color, tracer, .. }) = q.block(block) else {
            continue;
        };
        let min = pos.map(f64::from);
        outline_box(out, Point(min), Point(min.map(|v| v + 1.0)), color, 1.5);
        if tracers && tracer {
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
        let block = |id: &str, color, tracer| Block {
            id: id.into(),
            color,
            tracer,
        };
        assert_eq!(
            entry("minecraft:Diamond_Ore"),
            Some(block("diamond_ore", 0xffff_ffff, false))
        );
        assert_eq!(
            entry("gold_ore=#ffffd700"),
            Some(block("gold_ore", 0xffff_d700, false))
        );
        assert_eq!(
            entry("gold_ore=ffffd700 +Tracer"),
            Some(block("gold_ore", 0xffff_d700, true))
        );
        assert_eq!(
            entry("emerald_ore+tracer"),
            Some(block("emerald_ore", 0xffff_ffff, true))
        );
        assert_eq!(entry("!iron_ore"), None);
        assert_eq!(entry("iron_ore=zz"), None);
        let q = Query {
            blocks: vec![block("gold_ore", 1, false)],
            range: 50,
            only_caves: false,
        };
        assert_eq!(q.color("minecraft:gold_ore"), Some(1));
        assert_eq!(q.color("stone"), None);
    }

    #[test]
    fn a_tracer_needs_its_block_toggle_and_use_tracers() {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s.set(
            &key("blocks"),
            json!(["gold_ore=ffffd700 +tracer", "diamond_ore"]),
        )
        .unwrap();
        let camera = Camera {
            eye: [0.0; 3],
            look: [0.0, 0.0, 1.0],
            yaw: 0.0,
            pitch: 0.0,
        };
        let found = [
            ([0, 0, 5], "gold_ore".to_owned()),
            ([1, 0, 5], "diamond_ore".to_owned()),
        ];
        let tracers = |s: &Settings| {
            let mut out = Vec::new();
            lines(s, &camera, &found, &mut out);
            out.iter()
                .filter(|l| l.width == 2.0)
                .map(|l| (l.end.0, l.color))
                .collect::<Vec<_>>()
        };
        assert!(tracers(&s).is_empty());
        s.set(&key("use-tracers"), json!(true)).unwrap();
        assert_eq!(tracers(&s), [([0.5, 0.5, 5.5], 0xffff_d700)]);
    }
}
