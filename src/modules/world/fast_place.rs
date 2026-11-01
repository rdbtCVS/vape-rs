#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeldItem {
    All,
    Blocks,
    Projectiles,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Options {
    pub held_item: HeldItem,
    pub delay: u8,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            held_item: HeldItem::All,
            delay: 1,
        }
    }
}

/// Mirrors Vape's pre-tick assignment: it can shorten a pending repeat, but
/// never lengthens one that another ordinary use path already made sooner.
pub fn clamp_delay(current: u32, options: Options) -> u32 {
    current.min(u32::from(options.delay))
}
pub fn applies(
    options: Options,
    main: Option<&str>,
    off: Option<&str>,
    is_block: impl Fn(&str) -> bool,
) -> bool {
    match options.held_item {
        HeldItem::All => true,
        HeldItem::Blocks => main.is_some_and(&is_block) || off.is_some_and(is_block),
        HeldItem::Projectiles => main.is_some_and(projectile) || off.is_some_and(projectile),
    }
}
/// `ItemStackScoreUtil.Z`: the snowball, egg, ender pearl, eye of ender and
/// fire charge item classes (the 1.21.5 blue and brown eggs are eggs).
fn projectile(id: &str) -> bool {
    matches!(
        id,
        "minecraft:snowball"
            | "minecraft:egg"
            | "minecraft:blue_egg"
            | "minecraft:brown_egg"
            | "minecraft:ender_pearl"
            | "minecraft:ender_eye"
            | "minecraft:fire_charge"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(mode: HeldItem, main: Option<&str>, off: Option<&str>) -> bool {
        let options = Options {
            held_item: mode,
            delay: 1,
        };
        applies(options, main, off, |id| id == "minecraft:stone")
    }

    #[test]
    fn held_item_modes_follow_the_reference_item_classes() {
        assert!(held(HeldItem::All, None, None));
        assert!(held(HeldItem::Blocks, Some("minecraft:stone"), None));
        assert!(held(HeldItem::Blocks, None, Some("minecraft:stone")));
        assert!(!held(HeldItem::Blocks, Some("minecraft:snowball"), None));
        for item in [
            "minecraft:snowball",
            "minecraft:egg",
            "minecraft:blue_egg",
            "minecraft:ender_pearl",
            "minecraft:ender_eye",
            "minecraft:fire_charge",
        ] {
            assert!(held(HeldItem::Projectiles, Some(item), None), "{item}");
        }
        assert!(held(
            HeldItem::Projectiles,
            Some("minecraft:stone"),
            Some("minecraft:egg")
        ));
        assert!(!held(
            HeldItem::Projectiles,
            Some("minecraft:wind_charge"),
            None
        ));
        assert!(!held(HeldItem::Projectiles, Some("minecraft:stone"), None));
        assert_eq!(clamp_delay(4, Options::default()), 1);
        assert_eq!(clamp_delay(0, Options::default()), 0);
    }
}
