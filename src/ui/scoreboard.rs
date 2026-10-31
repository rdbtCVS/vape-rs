// Scoreboard (`ScoreboardHudModule`): the vanilla sidebar moved to a HUD
// panel, with its red score numbers hidden unless Show Score Numbers is on.
// The host measures and paints the sidebar; this module holds the settings
// and the reference's text replacement.

use super::super::core::settings::{Definition, Kind, map_entries};
use serde_json::{Value, json};

pub const SHOW_NUMBERS: &str = "hud.scoreboard.show-score-numbers";
/// `StringMapValue.create(this, "Replace scoreboard text", "Find text",
/// "Replace with")`.
pub const REPLACE_TEXT: &str = "hud.scoreboard.replace-text";

pub fn definitions() -> Vec<Definition> {
    let mut numbers = Definition::boolean(SHOW_NUMBERS, "Show score numbers", false);
    numbers.description = "Show the score next to each sidebar line".into();
    let replace = Definition::new(
        REPLACE_TEXT,
        "Replace scoreboard text",
        Kind::StringMap {
            max_entries: 64,
            max_chars: 256,
            placeholders: ["Find text", "Replace with"],
        },
        json!([]),
    );
    vec![numbers, replace]
}

/// The replacements in the order they were added.
pub fn replacements(value: &Value) -> Vec<(String, String)> {
    map_entries(value)
}

/// `ScoreboardHudModule.replaceScoreText`, bug for bug: when the visible
/// text (no `§` codes, no characters above 1000) contains `search`, its
/// characters are matched in order over the lowercased formatted text and
/// the matched span, if longer than one character, becomes `replacement`.
/// The rest of the result stays lowercased.
pub fn replace_score_text(search: &str, formatted: &str, replacement: &str) -> String {
    let mut visible = String::new();
    let mut chars = formatted.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else if c as u32 <= 1000 {
            visible.push(c);
        }
    }
    let search = search.to_lowercase();
    if !visible.to_lowercase().contains(&search) {
        return formatted.into();
    }
    let search: Vec<char> = search.chars().collect();
    let text: Vec<char> = formatted.to_lowercase().chars().collect();
    let (mut matched, mut start, mut end) = (0, None, None);
    let mut index = 0;
    while index < text.len() {
        if matched < search.len() {
            if text[index] == search[matched] {
                start.get_or_insert(index);
                matched += 1;
                if matched == search.len() {
                    end = Some(index);
                }
            } else if start.is_some() && text[index] == '§' {
                index += 1;
            }
        }
        index += 1;
    }
    match (start, end) {
        (Some(start), Some(end)) if end > start => {
            let mut out = String::new();
            for (index, c) in text.iter().enumerate() {
                if index < start || index > end {
                    out.push(*c);
                }
                if index == start {
                    out.push_str(replacement);
                }
            }
            out
        }
        _ => formatted.into(),
    }
}

/// Every replacement in order, as `renderScoreboard` applies them.
pub fn replace_all(replacements: &[(String, String)], text: &str) -> String {
    replacements
        .iter()
        .fold(text.to_string(), |text, (find, with)| {
            replace_score_text(find, &text, with)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_numbers_are_hidden_by_default() {
        let definitions = definitions();
        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].id, SHOW_NUMBERS);
        assert_eq!(definitions[1].id, REPLACE_TEXT);
        assert_eq!(definitions[1].default, json!([]));
    }

    #[test]
    fn replacement_follows_the_reference_quirks() {
        // A plain match: the span is replaced, the rest lowercased.
        assert_eq!(
            replace_score_text("play.example", "Play.Example.NET", "Vape"),
            "Vape.net"
        );
        // Matching is case-insensitive and keeps the replacement's case.
        assert_eq!(replace_score_text("TITLE", "my title", "Name"), "my Name");
        // Colour codes inside the match are skipped and removed with it.
        assert_eq!(
            replace_score_text("server", "§eser§6ver§r ip", "x"),
            "§ex§r ip"
        );
        // Not in the visible text: unchanged, case and all.
        assert_eq!(replace_score_text("absent", "Some Text", "x"), "Some Text");
        // A single character never replaces (matchEnd must pass matchStart).
        assert_eq!(replace_score_text("a", "Abc", "x"), "Abc");
        // An empty search never matches.
        assert_eq!(replace_score_text("", "Abc", "x"), "Abc");
        // Characters above 1000 are invisible to the containment check...
        assert_eq!(
            replace_score_text("a\u{4e00}", "a\u{4e00}b", "x"),
            "a\u{4e00}b"
        );
        // ...and the walk steps over them.
        assert_eq!(replace_score_text("ab", "a\u{4e00}b", "x"), "x");
        // The walk matches in order, not contiguously: "ab" is visible at
        // the end, but the span runs from the first 'a'.
        assert_eq!(replace_score_text("ab", "axab", "Q"), "Q");
        assert_eq!(
            replace_all(
                &[("one".into(), "two".into()), ("two".into(), "Three".into())],
                "One"
            ),
            "Three"
        );
    }
}
