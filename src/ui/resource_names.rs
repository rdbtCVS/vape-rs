/// Translation keys use the resource namespace, including for modded names.
pub fn resource_translation_key(kind: &str, id: &str) -> String {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    format!("{kind}.{namespace}.{path}")
}

#[cfg(test)]
mod translation_key_tests {
    use super::resource_translation_key;

    #[test]
    fn resource_names_keep_their_namespace() {
        assert_eq!(
            resource_translation_key("effect", "minecraft:speed"),
            "effect.minecraft.speed"
        );
        assert_eq!(
            resource_translation_key("biome", "example:custom_biome"),
            "biome.example.custom_biome"
        );
        assert_eq!(
            resource_translation_key("biome", "plains"),
            "biome.minecraft.plains"
        );
    }
}
