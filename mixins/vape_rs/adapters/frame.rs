fn frame(game: &mut crate::game::Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    crate::game::vape_rs::core::with_client(&client, |runtime| {
        runtime.observe_context(crate::game::vape_rs::core::lifecycle::Context {
            in_world: game.player.is_some() && game.title.is_none(),
            alive: game.player.as_ref().is_some_and(|p| p.health.health() > 0.0),
            interactive: game.focused && game.screen == crate::game::Screen::Playing && game.title.is_none() && !game.dialog_open() && game.server_packs.prompt.is_none(),
        });
        if !runtime.initialized() {
            let home = horizon_client::logging::game_home();
            runtime.attach_storage(
                home.canonicalize()
                    .unwrap_or(home)
                    .join("native-mods/vape-rs"),
            );
        }
        if runtime.ui_screen_owned {
            if runtime.gui.open
                && (game.player.is_none()
                    || game.title.is_some()
                    || game.screen != crate::game::Screen::Paused
                    || game.pause_menu
                    || game.dialog_open()
                    || game.server_packs.prompt.is_some())
            {
                runtime.gui.close();
            }
            if !runtime.gui.open {
                runtime.ui_screen_owned = false;
                if game.screen == crate::game::Screen::Paused && !game.pause_menu {
                    game.screen = crate::game::Screen::Playing;
                    game.pause_menu = true;
                    game.capture(true);
                    if let Some(window) = &game.window {
                        window.set_ime_allowed(false);
                    }
                }
            }
        }
        runtime.frame();
        for note in runtime.drain_notes() {
            eprintln!("[vape-rs] {note}");
        }
    });
    let gamma = crate::game::vape_rs::core::with_client(&client, |runtime| runtime.fullbright_gamma(game.settings.gamma, game.player.is_some() && game.title.is_none()));
    if let Some(gamma) = gamma && let Some(renderer) = game.renderer.as_mut() { renderer.gamma = gamma; }
    crate::game::vape_legit::frame(game);
    crate::game::remote::vape_macros::frame(game, false);
    crate::game::vape_combat::clickers(game);
    // Timer: the clock's next advance runs at this speed (1.0 when off).
    let speed = crate::game::vape_rs::core::with_client(&client, |runtime| runtime.timer_speed());
    game.clock.vape_rs_set_timer_speed(speed);
    crate::game::remote::vape_transactions::dispatch(game);
    // MotionBlur: this frame's blend, drawn after its HUD.
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let now_nanos = START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64 + 1;
    let view = crate::gizmos::vape_on_top::frame_camera().filter(|_| game.player.is_some() && game.title.is_none()).map(|(_, view)| {
        let f = view.inverse().transform_vector3(glam::Vec3::NEG_Z);
        [(-f.x).atan2(f.z).to_degrees(), (-f.y).clamp(-1.0, 1.0).asin().to_degrees()]
    });
    let blur = crate::game::vape_rs::core::with_client(&client, |runtime| {
        let gui_screen = runtime.ui_screen_owned && game.screen == crate::game::Screen::Paused && !game.pause_menu;
        runtime.motion_blur(crate::game::vape_rs::modules::render::motion_blur::Frame {
            gui_open: runtime.gui.open,
            screen_open: game.title.is_some() || game.dialog_open() || (game.screen != crate::game::Screen::Playing && !gui_screen),
            view,
            now_nanos,
        })
    });
    {
        use crate::game::vape_rs::modules::render::motion_blur::Request;
        use horizon_client::gfx::vape_motion_blur as render;
        render::request(match blur {
            Request::Off => render::Request::Off,
            Request::Skip => render::Request::Skip,
            Request::Draw(p) => render::Request::Draw(render::Pass { blurriness: p.blurriness, velocity_factor: p.velocity_factor, smooth: p.smooth, rgb: p.rgb }),
        });
    }
    crate::game::fishing::vape_auto_fish::auto_fish(game);
    // Paused worlds and screens stop ticks; hand synthetic movement keys back
    // without waiting for the next tick.
    if !game.focused || game.screen != crate::game::Screen::Playing || game.title.is_some() {
        crate::game::vape_input_delays::movement(game, false);
    }
}
