#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingMode {
    Toggle,
    Hold,
    Release,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyAction {
    Press,
    Repeat,
    Release,
    FocusLost,
}

#[derive(Debug, Default)]
pub struct Binding {
    down: bool,
}

impl Binding {
    pub fn reset(&mut self) {
        self.down = false;
    }

    // None means leave the current module state alone. Input while typing
    // never toggles a module, but release always clears a held key.
    pub fn apply(
        &mut self,
        mode: BindingMode,
        action: KeyAction,
        enabled: bool,
        accepts_input: bool,
    ) -> Option<bool> {
        if action == KeyAction::FocusLost {
            let was_down = std::mem::take(&mut self.down);
            return (was_down && mode == BindingMode::Hold).then_some(false);
        }
        if action == KeyAction::Release {
            let was_down = std::mem::take(&mut self.down);
            return match mode {
                BindingMode::Hold if was_down => Some(false),
                BindingMode::Release if was_down && accepts_input => Some(!enabled),
                _ => None,
            };
        }
        if !accepts_input || action == KeyAction::Repeat || self.down {
            return None;
        }
        self.down = true;
        match mode {
            BindingMode::Toggle => Some(!enabled),
            BindingMode::Hold => Some(true),
            BindingMode::Release => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_and_duplicate_presses_do_not_toggle() {
        let mut binding = Binding::default();
        assert_eq!(
            binding.apply(BindingMode::Toggle, KeyAction::Press, false, true),
            Some(true)
        );
        assert_eq!(
            binding.apply(BindingMode::Toggle, KeyAction::Repeat, true, true),
            None
        );
        assert_eq!(
            binding.apply(BindingMode::Toggle, KeyAction::Press, true, true),
            None
        );
        binding.apply(BindingMode::Toggle, KeyAction::Release, true, true);
        assert_eq!(
            binding.apply(BindingMode::Toggle, KeyAction::Press, true, true),
            Some(false)
        );
    }

    #[test]
    fn typing_and_unmatched_releases_have_no_effect() {
        let mut binding = Binding::default();
        for action in [KeyAction::Press, KeyAction::Repeat, KeyAction::Release] {
            assert_eq!(
                binding.apply(BindingMode::Release, action, false, false),
                None
            );
        }
        assert_eq!(
            binding.apply(BindingMode::Release, KeyAction::Release, false, true),
            None
        );
    }

    #[test]
    fn hold_cleans_up_when_focus_or_context_changes() {
        let mut binding = Binding::default();
        assert_eq!(
            binding.apply(BindingMode::Hold, KeyAction::Press, false, true),
            Some(true)
        );
        assert_eq!(
            binding.apply(BindingMode::Hold, KeyAction::Release, true, false),
            Some(false)
        );
        binding.apply(BindingMode::Hold, KeyAction::Press, false, true);
        assert_eq!(
            binding.apply(BindingMode::Hold, KeyAction::FocusLost, true, false),
            Some(false)
        );
    }

    #[test]
    fn release_bind_requires_matching_accepted_press() {
        let mut binding = Binding::default();
        assert_eq!(
            binding.apply(BindingMode::Release, KeyAction::Press, false, true),
            None
        );
        assert_eq!(
            binding.apply(BindingMode::Release, KeyAction::Release, false, true),
            Some(true)
        );
        assert_eq!(
            binding.apply(BindingMode::Release, KeyAction::Release, true, true),
            None
        );
    }
}

#[derive(Debug, Default)]
pub struct MultiBinding {
    pressed: std::collections::BTreeSet<String>,
    armed: bool,
}
impl MultiBinding {
    pub fn complete(&self, required: usize) -> bool {
        required > 0 && self.pressed.len() == required
    }
    pub fn apply(
        &mut self,
        key: &str,
        action: KeyAction,
        mode: BindingMode,
        enabled: bool,
        accepts_input: bool,
        required: usize,
    ) -> Option<bool> {
        if action == KeyAction::FocusLost {
            let held = !self.pressed.is_empty();
            self.pressed.clear();
            self.armed = false;
            return (held && enabled && mode == BindingMode::Hold).then_some(false);
        }
        if action == KeyAction::Release {
            let was_down = self.pressed.remove(key);
            let armed = std::mem::take(&mut self.armed);
            return match mode {
                BindingMode::Hold if was_down && enabled => Some(false),
                BindingMode::Release if was_down && armed && accepts_input => Some(!enabled),
                _ => None,
            };
        }
        if !accepts_input || action == KeyAction::Repeat || !self.pressed.insert(key.into()) {
            return None;
        }
        if required == 0 || self.pressed.len() != required {
            return None;
        }
        self.armed = true;
        match mode {
            BindingMode::Toggle => Some(!enabled),
            BindingMode::Hold => Some(true),
            BindingMode::Release => None,
        }
    }
}
#[cfg(test)]
mod multi_tests {
    use super::*;
    #[test]
    fn reference_chords_activate_only_when_complete_and_release_any_member() {
        let mut bind = MultiBinding::default();
        assert_eq!(
            bind.apply("A", KeyAction::Press, BindingMode::Hold, false, true, 2),
            None
        );
        assert_eq!(
            bind.apply("B", KeyAction::Press, BindingMode::Hold, false, true, 2),
            Some(true)
        );
        assert_eq!(
            bind.apply("A", KeyAction::Release, BindingMode::Hold, true, true, 2),
            Some(false)
        );
        assert_eq!(
            bind.apply("B", KeyAction::Release, BindingMode::Hold, false, true, 2),
            None
        );
    }
    #[test]
    fn focus_loss_releases_all_keys_and_repeat_never_generates_an_edge() {
        let mut bind = MultiBinding::default();
        assert_eq!(
            bind.apply("A", KeyAction::Repeat, BindingMode::Toggle, false, true, 1),
            None
        );
        bind.apply("A", KeyAction::Press, BindingMode::Hold, false, true, 1);
        assert_eq!(
            bind.apply("", KeyAction::FocusLost, BindingMode::Hold, true, false, 1),
            Some(false)
        );
        assert_eq!(
            bind.apply("A", KeyAction::Press, BindingMode::Hold, false, true, 1),
            Some(true)
        );
    }
}
