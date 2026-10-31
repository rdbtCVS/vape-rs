fn auto_heal_post_motion(value: (), game: &mut crate::game::Game<'_>) {
    let () = value;
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    crate::game::remote::vape_transactions::auto_heal_post_motion(game);
}
