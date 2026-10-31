// StorageESP (`StorageESP`, `RenderUtil.L`/`q`): a filled box in each
// enabled container type's colour, drawn through walls, and with "Outline
// open" a 3 px outline in the inverse colour while a chest's lid is more
// than 0.1 open. Containers come from the host's rendered block entities, so
// ender chests and shulker boxes are included.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::world::{Line, Point, Quad, fill_box, outline_box};
use serde_json::{Value, json};

const ID: &str = "render.storage-esp";

/// Container kinds in the reference's order, with each one's setting keys,
/// labels and default (enabled, ARGB colour).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Chest,
    TrappedChest,
    EnderChest,
    Hopper,
    Furnace,
    Dispenser,
    Dropper,
    Shulker,
}

impl Storage {
    pub const ALL: [Self; 8] = [
        Self::Chest,
        Self::TrappedChest,
        Self::EnderChest,
        Self::Hopper,
        Self::Furnace,
        Self::Dispenser,
        Self::Dropper,
        Self::Shulker,
    ];
    /// (render key, render label, colour key, colour label, enabled, colour).
    fn spec(
        self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        bool,
        u32,
    ) {
        match self {
            Self::Chest => (
                "chests",
                "Render Chests",
                "chest-color",
                "Chest Color",
                true,
                0x6401_ff92,
            ),
            Self::TrappedChest => (
                "trapped-chests",
                "Render Trapped Chests",
                "trapped-chest-color",
                "Chest Color",
                true,
                0x64ff_0000,
            ),
            Self::EnderChest => (
                "enderchests",
                "Render Enderchests",
                "ender-chest-color",
                "Ender Chest Color",
                false,
                0x647e_159c,
            ),
            Self::Hopper => (
                "hopper",
                "Render Hopper",
                "hopper-color",
                "Hopper Color",
                false,
                0xff8a_8a8a,
            ),
            Self::Furnace => (
                "furnace",
                "Render Furnace",
                "furnace-color",
                "Furnace Color",
                false,
                0xff5a_5a5a,
            ),
            Self::Dispenser => (
                "dispenser",
                "Render Dispenser",
                "dispenser-color",
                "Dispenser Color",
                false,
                0x6401_14c8,
            ),
            Self::Dropper => (
                "dropper",
                "Render Dropper",
                "dropper-color",
                "Dropper Color",
                false,
                0x6446_c8c8,
            ),
            Self::Shulker => (
                "shulker",
                "Render Shulker",
                "shulker-color",
                "Shulker Color",
                false,
                0x64ff_ffff,
            ),
        }
    }
    /// The kind of a block-entity block from its registry name. Furnaces
    /// include blast furnaces and smokers, as `TileEntityFurnace` subclasses.
    pub fn from_block(name: &str) -> Option<Self> {
        let name = name.trim_start_matches("minecraft:");
        Some(match name {
            "chest" => Self::Chest,
            "trapped_chest" => Self::TrappedChest,
            "ender_chest" => Self::EnderChest,
            "hopper" => Self::Hopper,
            "furnace" | "blast_furnace" | "smoker" => Self::Furnace,
            "dispenser" => Self::Dispenser,
            "dropper" => Self::Dropper,
            _ if name.ends_with("shulker_box") => Self::Shulker,
            _ => return None,
        })
    }
}

/// One container this frame: its kind, block and lid openness (0 shut).
pub type Block = (Storage, [i32; 3], f32);

pub fn definitions() -> Vec<Definition> {
    let mut out: Vec<Definition> = std::iter::once(Definition::boolean(
        &format!("{ID}.outline-open"),
        "Outline open",
        true,
    ))
    .chain(Storage::ALL.iter().map(|s| {
        let (key, label, _, _, on, _) = s.spec();
        Definition::boolean(&format!("{ID}.{key}"), label, on)
    }))
    .collect();
    for s in Storage::ALL {
        let (key, _, color_key, color_label, _, color) = s.spec();
        out.push(
            Definition::new(
                &format!("{ID}.{color_key}"),
                color_label,
                Kind::Color,
                json!(color),
            )
            .when(&format!("{ID}.{key}"), json!(true)),
        );
    }
    out
}

/// The colour for `storage`, or None when its type is off.
pub fn color(settings: &Settings, storage: Storage) -> Option<u32> {
    let (key, _, color_key, _, on, color) = storage.spec();
    settings
        .get(&format!("{ID}.{key}"))
        .and_then(Value::as_bool)
        .unwrap_or(on)
        .then(|| {
            settings
                .get(&format!("{ID}.{color_key}"))
                .and_then(Value::as_u64)
                .map_or(color, |c| c as u32)
        })
}

fn unit(x: i32, y: i32, z: i32) -> (Point, Point) {
    let min = [f64::from(x), f64::from(y), f64::from(z)];
    (Point(min), Point(min.map(|v| v + 1.0)))
}

/// The fills of the enabled storage blocks.
pub fn quads(settings: &Settings, blocks: &[Block], out: &mut Vec<Quad>) {
    for &(storage, [x, y, z], _) in blocks {
        if let Some(color) = color(settings, storage) {
            let (min, max) = unit(x, y, z);
            fill_box(out, min, max, color);
        }
    }
}

/// "Outline open": the inverse-colour outline of open chests.
pub fn lines(settings: &Settings, blocks: &[Block], out: &mut Vec<Line>) {
    let outline_open = settings
        .get(&format!("{ID}.outline-open"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if !outline_open {
        return;
    }
    for &(storage, [x, y, z], open) in blocks {
        if open <= 0.1 {
            continue;
        }
        if let Some(color) = color(settings, storage) {
            let inverse = (color & 0xff00_0000) | (!color & 0x00ff_ffff);
            let (min, max) = unit(x, y, z);
            outline_box(out, min, max, inverse, 3.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_match_reference_for_supported_storage() {
        let definitions = definitions();
        let default = |id: &str| &definitions.iter().find(|d| d.id == id).unwrap().default;
        assert_eq!(default("render.storage-esp.chests"), &json!(true));
        assert_eq!(default("render.storage-esp.trapped-chests"), &json!(true));
        assert_eq!(default("render.storage-esp.furnace"), &json!(false));
        assert_eq!(
            default("render.storage-esp.chest-color"),
            &json!(0x6401_ff92_u32)
        );
        assert_eq!(
            default("render.storage-esp.furnace-color"),
            &json!(0xff5a_5a5a_u32)
        );
        assert_eq!(
            Storage::from_block("minecraft:red_shulker_box"),
            Some(Storage::Shulker)
        );
        assert_eq!(Storage::from_block("minecraft:barrel"), None);
    }
}
