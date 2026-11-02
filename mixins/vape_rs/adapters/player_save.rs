fn player_save(
    guest: &crate::lan::guest::Guest,
    world: &std::path::Path,
    vehicle: Option<[i32; 4]>,
) -> core::ops::ControlFlow<()> {
    let mut level = crate::save::Level {
        player_uuid: Some(crate::lan::playerdata::uuid_ints(guest.uuid)),
        ..Default::default()
    };
    crate::lan::playerdata::store(guest, &mut level, vehicle);
    crate::lan::playerdata::vape_player_saves::submit(
        crate::lan::playerdata::path(world, guest.uuid),
        level.player_tag(),
    );
    core::ops::ControlFlow::Break(())
}
