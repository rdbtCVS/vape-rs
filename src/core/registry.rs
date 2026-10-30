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
    Coordinates => "hud.coordinates", "Coords", "HUD", Some((8.0, 8.0)), ["F8"];
    Fps => "hud.fps", "FPS", "HUD", Some((8.0, 36.0)), ["F9"];
    Compass => "hud.compass", "Compass", "HUD", Some((46.0, 200.0)), [];
    NoHurtCamera => "render.no-hurt-camera", "NoHurtCam", "Render", None, [];
    AutoTool => "world.auto-tool", "AutoTool", "World", None, [];
    Clock => "hud.clock", "Clock", "HUD", Some((250.0, 8.0)), [];
    Keystrokes => "hud.keystrokes", "Keystrokes", "HUD", Some((8.0, 120.0)), [];
    ArmorStatus => "hud.armor", "Armor Status", "HUD", Some((330.0, 100.0)), [];
    PotionEffects => "hud.potions", "Potion Effects", "HUD", Some((240.0, 180.0)), [];
    AutoArmor => "inventory.auto-armor", "AutoArmor", "Inventory", None, [];
    AutoTotem => "inventory.auto-totem", "AutoTotem", "Inventory", None, [];
    Refill => "inventory.refill", "Refill", "Inventory", None, [];
    ChestSteal => "world.chest-steal", "ChestSteal", "World", None, [];
    FastPlace => "world.fast-place", "FastPlace", "World", None, [];
    Fullbright => "render.fullbright", "Fullbright", "Render", None, [];
    ClearWater => "render.clear-water", "Clear Water", "Render", None, [];
    AutoFish => "utility.auto-fish", "AutoFish", "World", None, [];
    NoJumpDelay => "legit.no-jump-delay", "NoJumpDelay", "Render", None, [];
    NoClickDelay => "legit.no-click-delay", "NoClickDelay", "Render", None, [];
    Sprint => "combat.sprint", "Sprint", "Combat", None, [];
    Panic => "utility.panic", "Panic", "Utility", None, ["F10"];
    StorageEsp => "render.storage-esp", "StorageESP", "Render", None, [];
    Parkour => "world.parkour", "Parkour", "World", None, [];
    AimAssist => "combat.aim-assist", "AimAssist", "Combat", None, [];
    SafeWalk => "world.safe-walk", "SafeWalk", "World", None, [];
    TriggerBot => "combat.trigger-bot", "TriggerBot", "Combat", None, [];
    Velocity => "combat.velocity", "Velocity", "Utility", None, [];
    AutoClicker => "combat.auto-clicker", "AutoClicker", "Combat", None, [];
    RightClicker => "combat.right-clicker", "RightClicker", "Combat", None, [];
    Reach => "combat.reach", "Reach", "Combat", None, [];
    HitBoxes => "blatant.hitboxes", "HitBoxes", "Other", None, [];
    Timer => "blatant.timer", "Timer", "Other", None, [];
    Fly => "blatant.fly", "Fly", "Other", None, [];
    NoFall => "blatant.no-fall", "NoFall", "Other", None, [];
    NoSlowdown => "blatant.no-slowdown", "NoSlowdown", "Hidden", None, [];
    TimeChanger => "legit.time-changer", "Time Changer", "Render", None, [];
    Weather => "legit.weather", "Weather", "Render", None, [];
    AntiAfk => "world.anti-afk", "Anti-AFK", "World", None, [];
    Step => "blatant.step", "Step", "Other", None, [];
    WTap => "combat.wtap", "WTap", "Combat", None, [];
    Speed => "blatant.speed", "Speed", "Hidden", None, [];
    LongJump => "blatant.long-jump", "LongJump", "Other", None, [];
    AntiFall => "blatant.anti-fall", "AntiFall", "Other", None, [];
}

impl ModuleId {
    pub fn is_panel(self) -> bool {
        self.panel().is_some()
    }
    pub(super) fn position(self) -> (f64, f64) {
        self.panel().unwrap_or((8.0, 8.0))
    }
}
