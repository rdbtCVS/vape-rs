// Scoreboard (`ScoreboardHudModule`): the vanilla sidebar moved to a HUD
// panel, with its red score numbers hidden unless Show Score Numbers is on.
// The host measures and paints the sidebar; this module only holds the
// settings.

use super::super::core::settings::Definition;

pub const SHOW_NUMBERS: &str = "hud.scoreboard.show-score-numbers";

pub fn definitions() -> Vec<Definition> {
    let mut numbers = Definition::boolean(SHOW_NUMBERS, "Show score numbers", false);
    numbers.description = "Show the score next to each sidebar line".into();
    vec![numbers]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_numbers_are_hidden_by_default() {
        let definitions = definitions();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].id, SHOW_NUMBERS);
    }
}
