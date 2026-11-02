fn player_save_barrier(result: Result<()>) -> Result<()> {
    let saved = horizon_client::lan::playerdata::vape_player_saves::flush();
    result?;
    saved.map_err(Into::into)
}
