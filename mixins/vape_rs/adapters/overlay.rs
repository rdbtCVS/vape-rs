fn overlay(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::fov::vape_overlay::draw(game, batch);
}
