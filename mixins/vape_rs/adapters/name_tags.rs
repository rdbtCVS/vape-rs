fn name_tags(game: &mut crate::game::Game<'_>, camera: glam::DVec3, alpha: f64, out: &mut Vec<Vertex>, see_through: &mut Vec<Vertex>) -> core::ops::ControlFlow<()> {
    crate::game::names::vape_name_tags::geometry(game, camera, alpha, out, see_through)
}
