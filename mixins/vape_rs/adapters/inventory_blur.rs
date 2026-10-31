fn inventory_blur(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<Option<f32>> {
    use crate::game::vape_rs::core::{ModuleId, with_client};
    if game.title.is_some() || game.player.is_none() {
        return core::ops::ControlFlow::Continue(());
    }
    if !matches!(game.screen, crate::game::Screen::Inventory | crate::game::Screen::Creative) {
        return core::ops::ControlFlow::Continue(());
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if !with_client(&client, |runtime| runtime.enabled(ModuleId::InventoryBlur)) {
        return core::ops::ControlFlow::Continue(());
    }
    // The menu blur radius, or the vanilla default when it is turned off.
    let radius = game.settings.menu_blur as f32;
    core::ops::ControlFlow::Break(Some(if radius >= 1.0 { radius } else { 5.0 }))
}
