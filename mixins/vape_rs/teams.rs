// TargetFilter's team snapshot: the player list (`getPlayerInfoMap`) and the
// scoreboard teams of the local player and the other players.
use super::Game;
use crate::game::vape_rs::modules::utility::target_filter::{Entry, Snapshot, Team};
use horizon_client::scoreboard::{comp_text, team_color_rgb};

fn team(game: &Game<'_>, name: &str) -> Option<Team> {
    let t = game.score.board.players_team(name)?;
    Some(Team {
        name: t.name.clone(),
        prefix: comp_text(&t.prefix).plain(),
        color: t.color.as_deref().and_then(team_color_rgb),
    })
}

pub(crate) fn snapshot(game: &Game<'_>) -> Snapshot {
    let Some(r) = game.remote.as_ref() else {
        return Snapshot::default();
    };
    // `Entity.getTeam` holds a player by its profile name.
    let local = r.profile_name.clone();
    let entries: Vec<Entry> = r
        .tab_players
        .values()
        .map(|tab| Entry {
            name: tab.name.clone(),
            display: tab.display_name.clone(),
            display_text: tab.display_name.as_ref().map(|c| comp_text(c).plain()),
            team: team(game, &tab.name),
        })
        .collect();
    let teams = entries
        .iter()
        .map(|e| e.name.as_str())
        .chain(r.level_player_names.values().map(String::as_str))
        .filter(|n| *n != local)
        .filter_map(|n| Some((n.to_owned(), team(game, n)?)))
        .collect();
    Snapshot {
        local_team: team(game, &local),
        local,
        entries,
        teams,
    }
}

/// Pushes this tick's snapshot before targeting runs (`EventPreTick`).
pub(crate) fn tick(game: &Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let active = crate::game::vape_rs::core::with_client(&client, |rt| rt.target_filter_active());
    let snapshot = if active { snapshot(game) } else { Snapshot::default() };
    crate::game::vape_rs::core::with_client(&client, |rt| rt.target_filter_tick(snapshot));
}
