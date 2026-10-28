fn control_key_names(name: &str) -> core::ops::ControlFlow<Option<KeyCode>> {
    use winit::keyboard::KeyCode;
    let key = if name.eq_ignore_ascii_case("rshift") {
        KeyCode::ShiftRight
    } else if name.eq_ignore_ascii_case("f8") {
        KeyCode::F8
    } else if name.eq_ignore_ascii_case("f9") {
        KeyCode::F9
    } else if name.eq_ignore_ascii_case("f10") {
        KeyCode::F10
    } else if name.eq_ignore_ascii_case("home") {
        KeyCode::Home
    } else if name.eq_ignore_ascii_case("end") {
        KeyCode::End
    } else if name.eq_ignore_ascii_case("delete") {
        KeyCode::Delete
    } else {
        return core::ops::ControlFlow::Continue(());
    };
    core::ops::ControlFlow::Break(Some(key))
}
