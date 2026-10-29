fn world_lines(game: &crate::game::Game<'_>, alpha: f64, size: [u32; 2]) -> core::ops::ControlFlow<Vec<Line>> {
    crate::game::fishing::vape_world_lines::fishing_line(game, alpha, size)
}
