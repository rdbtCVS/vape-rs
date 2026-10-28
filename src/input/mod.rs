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
