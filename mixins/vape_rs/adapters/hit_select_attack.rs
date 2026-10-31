fn hit_select_attack(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    crate::game::vape_hit_select::attack(game)
}
