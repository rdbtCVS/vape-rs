// `ClientSettings` (the "GUI" module) values beyond multi-binding and the
// GUI bind, plus the `PublicProfileSettings` notification toggles.

use super::super::core::settings::{Definition, Kind};
use serde_json::json;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SearchStyle {
    /// A separate search frame at the top centre of the screen.
    #[default]
    Floating,
    /// The navigation frame's header magnifier.
    Integrated,
    None,
}

impl SearchStyle {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("Integrated") => Self::Integrated,
            Some("None") => Self::None,
            _ => Self::Floating,
        }
    }
}

/// `new Color(5, 134, 105)`, the reference's default Gui Color.
pub const DEFAULT_ACCENT: u32 = 0xff05_8669;

pub fn definitions() -> Vec<Definition> {
    let describe = |mut d: Definition, text: &str| {
        d.description = text.into();
        d
    };
    vec![
        // `config.ClientSettings.guiColor`, the GUI Theme section's colour
        // editor: the accent of the click GUI and the Text GUI.
        Definition::new(
            "client.gui-color",
            "Gui Color",
            Kind::Color,
            json!(DEFAULT_ACCENT),
        ),
        describe(
            Definition::boolean("client.blur-background", "Blur background", true),
            "Blur the background of the GUI",
        ),
        describe(
            Definition::boolean("client.gui-bind-indicator", "GUI bind indicator", true),
            "Displays a message indicating your GUI keybind upon injecting.\nI.E. \"Press RSHIFT to open GUI\"",
        ),
        describe(
            Definition::boolean("client.show-tooltips", "Show tooltips", true),
            "Toggles visibility of these",
        ),
        describe(
            Definition::number("client.rainbow-speed", "Rainbow speed", 0.1, 1.0, 10.0, 0.1),
            "Adjusts the speed of rainbow values",
        ),
        describe(
            Definition::choice(
                "client.search-bar",
                "Search bar style",
                &["Floating", "Integrated", "None"],
                "Floating",
            ),
            "Where the module search bar is shown",
        ),
        describe(
            Definition::boolean("client.notifications", "Notifications", true),
            "Shows notifications",
        ),
        describe(
            Definition::boolean("client.toggle-alerts", "Toggle alert", false),
            "Notifies you if a module is enabled/disabled.",
        )
        .when("client.notifications", serde_json::json!(true)),
        describe(
            Definition::boolean("client.profile-alerts", "Profile switch", false),
            "Notifies you when you switch profiles",
        )
        .when("client.notifications", serde_json::json!(true)),
        describe(
            Definition::boolean("client.profile-auto-load", "Auto-load module states", true),
            "Automatically enable saved module states upon loading, and when selecting profiles",
        ),
        describe(
            Definition::boolean(
                "client.profile-frames",
                "Frame positions per profile",
                false,
            ),
            "Saves the positions of your GUI frames per profile",
        ),
    ]
}

/// LWJGL-style key names (`Keyboard.getKeyName`) for physical key codes.
pub fn key_text(key: &str) -> String {
    let named = match key {
        "ShiftRight" => "RSHIFT",
        "ShiftLeft" => "LSHIFT",
        "ControlRight" => "RCONTROL",
        "ControlLeft" => "LCONTROL",
        "AltRight" => "RMENU",
        "AltLeft" => "LMENU",
        "SuperLeft" => "LMETA",
        "SuperRight" => "RMETA",
        "Space" => "SPACE",
        "Enter" => "RETURN",
        "Backspace" => "BACK",
        "Insert" => "INSERT",
        "Delete" => "DELETE",
        "MouseLeft" => "BUTTON0",
        "MouseRight" => "BUTTON1",
        "MouseMiddle" => "BUTTON2",
        _ => "",
    };
    if !named.is_empty() {
        return named.into();
    }
    key.strip_prefix("Key")
        .or_else(|| key.strip_prefix("Digit"))
        .unwrap_or(key)
        .to_uppercase()
}

/// `Bind.getBindText`: chord keys joined with `+`.
pub fn bind_text(keys: &[&str]) -> String {
    keys.iter()
        .map(|k| key_text(k))
        .collect::<Vec<_>>()
        .join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_and_key_names_follow_the_reference() {
        assert_eq!(SearchStyle::parse(None), SearchStyle::Floating);
        assert_eq!(
            SearchStyle::parse(Some("Integrated")),
            SearchStyle::Integrated
        );
        assert_eq!(bind_text(&["ShiftRight"]), "RSHIFT");
        assert_eq!(bind_text(&["ControlLeft", "KeyG"]), "LCONTROL + G");
        assert_eq!(key_text("F10"), "F10");
        assert_eq!(key_text("Digit4"), "4");
    }
}
