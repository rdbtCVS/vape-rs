// StorageESP (`StorageESP`): outlines of the enabled container types, drawn
// through walls. The reference's fill and "Outline open" lid contrast are
// pending host support for filled world quads and lid state.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::world::{Line, Point, outline_box};
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

pub fn definitions() -> Vec<Definition> {
    let mut out: Vec<Definition> = Storage::ALL
        .iter()
        .map(|s| {
            let (key, label, _, _, on, _) = s.spec();
            Definition::boolean(&format!("{ID}.{key}"), label, on)
        })
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

/// Box outlines (1.5 px, as the reference's line width) for the enabled
/// storage blocks.
pub fn lines(settings: &Settings, blocks: &[(Storage, [i32; 3])], out: &mut Vec<Line>) {
    for &(storage, [x, y, z]) in blocks {
        if let Some(color) = color(settings, storage) {
            let min = [f64::from(x), f64::from(y), f64::from(z)];
            outline_box(
                out,
                Point(min),
                Point([min[0] + 1.0, min[1] + 1.0, min[2] + 1.0]),
                color,
                1.5,
            );
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
