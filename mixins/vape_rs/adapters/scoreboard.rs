fn scoreboard(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    use crate::game::vape_rs::core::{ModuleId, with_client};
    // Scoreboard moves the sidebar into its HUD panel: the vanilla one is
    // skipped while the module is on.
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if with_client(&client, |runtime| runtime.enabled(ModuleId::Scoreboard)) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
