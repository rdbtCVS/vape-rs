// ESP's Outline mode: players it selects take the host's glowing outline
// (drawn through walls) in the ESP colour. The world-lines hook, which runs
// before entities are drawn, sets this frame's colours.
use super::Game;
use core::cell::RefCell;
use core::ops::ControlFlow;
use horizon_client::mobs::Mob;
use std::collections::HashMap;

thread_local! {
    static COLORS: RefCell<HashMap<u32, u32>> = RefCell::new(HashMap::new());
}

/// This frame's outline colours (RGB) by entity id.
pub(crate) fn set(colors: HashMap<u32, u32>) {
    COLORS.with(|c| *c.borrow_mut() = colors);
}

pub(crate) fn mob_outline(_game: &Game<'_>, m: &Mob) -> ControlFlow<Option<u32>> {
    match COLORS.with(|c| c.borrow().get(&m.id).copied()) {
        Some(rgb) => ControlFlow::Break(Some(rgb)),
        None => ControlFlow::Continue(()),
    }
}
