fn player_close(result: ()) {
    let _ = result;
    if let Err(error) = horizon_client::lan::playerdata::vape_player_saves::flush() {
        log::warn!("player save barrier: {error}");
    }
}
