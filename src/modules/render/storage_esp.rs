// Initial storage outlines. Other container types and fill are pending host support.
use super::super::super::core::settings::{Definition, Kind};
use serde_json::json;

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::boolean("render.storage-esp.chests", "Render Chests", true),
        Definition::new(
            "render.storage-esp.chest-color",
            "Chest Color",
            Kind::Color,
            json!(0x6401_ff92_u32),
        ),
        Definition::boolean("render.storage-esp.furnace", "Render Furnace", false),
        Definition::new(
            "render.storage-esp.furnace-color",
            "Furnace Color",
            Kind::Color,
            json!(0xff5a_5a5a_u32),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_match_reference_for_supported_storage() {
        let definitions = definitions();
        let default = |id: &str| &definitions.iter().find(|d| d.id == id).unwrap().default;
        assert_eq!(default("render.storage-esp.chests"), &json!(true));
        assert_eq!(default("render.storage-esp.furnace"), &json!(false));
        assert_eq!(
            default("render.storage-esp.chest-color"),
            &json!(0x6401_ff92_u32)
        );
        assert_eq!(
            default("render.storage-esp.furnace-color"),
            &json!(0xff5a_5a5a_u32)
        );
    }
}
