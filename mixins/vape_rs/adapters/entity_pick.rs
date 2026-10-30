fn entity_pick(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<Option<(usize, f64)>> {
    crate::game::vape_combat::pick(game)
}
