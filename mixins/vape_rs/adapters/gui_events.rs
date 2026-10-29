fn gui_events(
    game: &mut crate::game::Game<'_>,
    id: winit::window::WindowId,
    event: &winit::event::WindowEvent,
) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    use crate::game::vape_rs::{core::with_client, input::KeyAction};
    use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
    if game.serving.is_some()
        || game.options.dedicated.is_some()
        || game.window.as_ref().is_none_or(|w| w.id() != id)
    {
        return;
    }
    with_client(&client, |runtime| {
        if let WindowEvent::Focused(false) = event {
            runtime.release_input();
            runtime.gui.key("", KeyAction::FocusLost);
        }
        if !runtime.gui.open || game.dialog_open() || game.server_packs.prompt.is_some() {
            return;
        }
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(renderer) = &game.renderer {
                    let batch = horizon_client::hud::HudBatch::new([
                        renderer.config.width,
                        renderer.config.height,
                    ]);
                    runtime.gui_pointer((
                        position.x as f32 / batch.scale,
                        position.y as f32 / batch.scale,
                    ));
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let name = match button {
                    MouseButton::Left => "MouseLeft".into(),
                    MouseButton::Right => "MouseRight".into(),
                    MouseButton::Middle => "MouseMiddle".into(),
                    MouseButton::Back => "MouseBack".into(),
                    MouseButton::Forward => "MouseForward".into(),
                    MouseButton::Other(n) => format!("Mouse{n}"),
                };
                runtime.gui_mouse(&name, *state == ElementState::Pressed, game.cursor);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 32.0,
                };
                runtime.gui_wheel(y, game.cursor);
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && !game.modifiers.control_key()
                    && !game.modifiers.super_key() =>
            {
                if let Some(text) = &event.text {
                    runtime.gui.commit_text(text, true);
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => runtime.gui.commit_text(text, false),
            _ => {}
        }
    });
}
