fn player_read() {
    if let Err(error) = crate::lan::playerdata::vape_player_saves::flush() {
        log::warn!("player save barrier: {error}");
    }
}
