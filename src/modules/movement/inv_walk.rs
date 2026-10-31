// InvWalk (reference `InvWalk`, Category.OTHER): while a screen is open the
// movement keys (W/A/S/D, jump and, with "Sneak", sneak) follow the keys
// physically held, so the player keeps walking behind inventories and menus.
// Chat (and a focused Vape text field) is excluded; "Inventory Only" limits
// it to the player inventory and the pause menu. "Rotate" turns with the
// arrow keys: up/down change pitch by 3 degrees, left/right yaw by 5.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::Value;

pub const ID: &str = "blatant.inv-walk";

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::boolean(&key("inventory-only"), "Inventory Only", false),
        Definition::boolean(&key("sneak"), "Sneak", false),
        Definition::boolean(&key("rotate"), "Rotate", false),
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub inventory_only: bool,
    pub sneak: bool,
    pub rotate: bool,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let flag = |k: &str| {
            settings
                .get(&key(k))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        Self {
            inventory_only: flag("inventory-only"),
            sneak: flag("sneak"),
            rotate: flag("rotate"),
        }
    }
}

/// The screen in front of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// No screen: the game has the keys itself.
    None,
    Chat,
    /// The player's own inventory (no container open).
    Inventory,
    Pause,
    /// A Vape text field has the keyboard.
    TextField,
    /// Any other screen, including containers and the click GUI.
    Other,
}

/// Whether InvWalk hands the physical movement keys to the player.
pub fn active(o: &Options, screen: Screen) -> bool {
    match screen {
        Screen::None | Screen::Chat | Screen::TextField => false,
        Screen::Inventory | Screen::Pause => true,
        Screen::Other => !o.inventory_only,
    }
}

/// Physically held keys this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub up: bool,
    pub down: bool,
    pub turn_left: bool,
    pub turn_right: bool,
}

/// The arrow-key turn: (yaw, pitch), the pitch clamped to +-89.5.
pub fn rotate(o: &Options, held: &Held, yaw: f32, pitch: f32) -> (f32, f32) {
    if !o.rotate {
        return (yaw, pitch);
    }
    let mut pitch = pitch;
    let mut yaw = yaw;
    if held.up {
        pitch -= 3.0;
    }
    if held.down {
        pitch += 3.0;
    }
    if held.turn_left {
        yaw -= 5.0;
    }
    if held.turn_right {
        yaw += 5.0;
    }
    (yaw, pitch.clamp(-89.5, 89.5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screens_and_inventory_only() {
        let mut o = Options::default();
        assert!(active(&o, Screen::Other));
        assert!(active(&o, Screen::Inventory));
        assert!(!active(&o, Screen::Chat));
        assert!(!active(&o, Screen::None));
        assert!(!active(&o, Screen::TextField));
        o.inventory_only = true;
        assert!(!active(&o, Screen::Other));
        assert!(active(&o, Screen::Pause));
    }

    #[test]
    fn arrows_turn_and_clamp() {
        let o = Options {
            rotate: true,
            ..Default::default()
        };
        let held = Held {
            down: true,
            turn_right: true,
            ..Default::default()
        };
        assert_eq!(rotate(&o, &held, 10.0, 88.0), (15.0, 89.5));
        assert_eq!(rotate(&Options::default(), &held, 10.0, 0.0), (10.0, 0.0));
    }
}
