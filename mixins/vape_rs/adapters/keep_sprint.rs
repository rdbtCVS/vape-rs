fn keep_sprint(game: &mut crate::game::Game<'_>, mob: usize) -> core::ops::ControlFlow<()> {
    crate::game::remote::vape_combat_remote::keep_sprint(game, mob)
}
