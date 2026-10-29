// Observes the authoritative fishing-hook metadata before Horizon consumes it.
// The mod retains only DATA_HOOKED_ENTITY because Horizon's remote Hook model
// currently retains DATA_BITING but deliberately does not mirror the target.
fn observe_hook_data(
    game: &crate::game::Game<'_>,
    id: u32,
    values: &[(u8, i32, horizon_protocol::entity_data::Value)],
) {
    // `remote_hook_data` receives every tracked bobber. Keep only this
    // client's current hook; the host applies the same ownership filter.
    if !game.hook.as_ref().is_some_and(|hook| hook.id == id) {
        return;
    }
    let target = values
        .iter()
        .rev()
        .find(|(index, _, _)| *index == 8)
        .map(|(_, _, value)| match value {
            // FishingHook.DATA_HOOKED_ENTITY stores the real entity id plus
            // one; zero is the no-target sentinel.
            horizon_protocol::entity_data::Value::Int(value) if *value > 0 => Some(*value - 1),
            _ => None,
        });
    let Some(target) = target else {
        return;
    };
    let client = game.__horizon_mixins_vape_rs_client().clone();
    crate::game::vape_rs::core::with_client(&client, |runtime| {
        runtime.observe_fish_hook(id, target);
    });
}
