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
fn projectile(id: &str) -> bool {
    matches!(id, "minecraft:snowball" | "minecraft:egg")
}
