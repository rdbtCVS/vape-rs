// The module registry: one line per module, in GUI/profile order.
//
// Each line is: variant => stable id, display name, GUI category, HUD panel
// default position (None for non-panel modules), default key bindings.
// Adding a module means adding one line here; everything keyed by ModuleId
// (settings, profiles, binds, GUI listing) derives from this table.

macro_rules! registry {
    ($($variant:ident => $id:literal, $name:literal, $category:literal, $panel:expr, [$($bind:literal),*];)*) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
        pub enum ModuleId {
            $($variant,)*
        }
        impl ModuleId {
            pub const ALL: [Self; [$($id),*].len()] = [$(Self::$variant),*];
            pub fn id(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)*
                }
            }
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }
            pub fn category(self) -> &'static str {
                match self {
                    $(Self::$variant => $category,)*
                }
            }
            fn panel(self) -> Option<(f64, f64)> {
                match self {
                    $(Self::$variant => $panel,)*
                }
            }
            pub fn default_binds(self) -> &'static [&'static str] {
                match self {
                    $(Self::$variant => &[$($bind),*],)*
                }
            }
        }
    };
}

registry! {
    Coordinates => "hud.coordinates", "Coords", "Legit", Some((8.0, 8.0)), ["F8"];
    Fps => "hud.fps", "FPS", "Legit", Some((8.0, 36.0)), ["F9"];
    Compass => "hud.compass", "Compass", "Legit", Some((46.0, 200.0)), [];
    NoHurtCamera => "render.no-hurt-camera", "NoHurtCam", "Legit", None, [];
    AutoTool => "world.auto-tool", "AutoTool", "World", None, [];
    Clock => "hud.clock", "Clock", "Legit", Some((250.0, 8.0)), [];
    Keystrokes => "hud.keystrokes", "Keystrokes", "Legit", Some((8.0, 120.0)), [];
    ArmorStatus => "hud.armor", "Armor Status", "Legit", Some((330.0, 100.0)), [];
    PotionEffects => "hud.potions", "Potion Effects", "Legit", Some((240.0, 180.0)), [];
    AutoArmor => "inventory.auto-armor", "AutoArmor", "Utility", None, [];
    AutoTotem => "inventory.auto-totem", "AutoTotem", "Utility", None, [];
    Refill => "inventory.refill", "Refill", "Utility", None, [];
    ChestSteal => "world.chest-steal", "ChestSteal", "World", None, [];
    FastPlace => "world.fast-place", "FastPlace", "World", None, [];
    Fullbright => "render.fullbright", "Fullbright", "Render", None, [];
    ClearWater => "render.clear-water", "Clear Water", "Legit", None, [];
    AutoFish => "utility.auto-fish", "AutoFish", "Utility", None, [];
    NoJumpDelay => "legit.no-jump-delay", "NoJumpDelay", "Legit", None, [];
    NoClickDelay => "legit.no-click-delay", "NoClickDelay", "Legit", None, [];
    Sprint => "combat.sprint", "Sprint", "Legit", None, [];
    Panic => "utility.panic", "Panic", "Utility", None, ["F10"];
    StorageEsp => "render.storage-esp", "StorageESP", "Render", None, [];
    Parkour => "world.parkour", "Parkour", "World", None, [];
    AimAssist => "combat.aim-assist", "AimAssist", "Combat", None, [];
    SafeWalk => "world.safe-walk", "SafeWalk", "World", None, [];
    TriggerBot => "combat.trigger-bot", "TriggerBot", "Combat", None, [];
    Velocity => "combat.velocity", "Velocity", "Utility", None, [];
}

impl ModuleId {
    pub fn is_panel(self) -> bool {
        self.panel().is_some()
    }
    pub(super) fn position(self) -> (f64, f64) {
        self.panel().unwrap_or((8.0, 8.0))
    }
}
