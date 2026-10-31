// The player list's game mode and display name of the players in the level
// (NameTags' creative marker, ESP's "Use Displayname").
use super::Game;

/// `(entity id, game mode, plain display name)` per player in the level.
pub(crate) fn players(game: &Game<'_>) -> Vec<(u32, i32, Option<String>)> {
    let Some(r) = game.remote.as_ref() else {
        return Vec::new();
    };
    r.level_players
        .iter()
        .filter_map(|(id, uuid)| {
            let tab = r.tab_players.get(uuid)?;
            let display = tab.display_name.as_ref().map(|c| horizon_client::scoreboard::comp_text(c).plain());
            Some((*id as u32, tab.game_mode, display))
        })
        .collect()
}
