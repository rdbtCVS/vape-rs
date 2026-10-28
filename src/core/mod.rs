use super::input::{Binding, BindingMode, KeyAction};
use super::ui::{CoordinatesLayout, HudLine, Snapshot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleId {
    Coordinates,
    Fps,
}

#[derive(Debug, Default)]
pub struct Runtime {
    coordinates: bool,
    fps: bool,
    coordinates_bind: Binding,
    fps_bind: Binding,
    ticks: u64,
    layout: CoordinatesLayout,
}

impl Runtime {
    pub fn enabled(&self, module: ModuleId) -> bool {
        match module {
            ModuleId::Coordinates => self.coordinates,
            ModuleId::Fps => self.fps,
        }
    }

    pub fn set_enabled(&mut self, module: ModuleId, enabled: bool) {
        match module {
            ModuleId::Coordinates => self.coordinates = enabled,
            ModuleId::Fps => self.fps = enabled,
        }
    }

    pub fn set_layout(&mut self, layout: CoordinatesLayout) {
        self.layout = layout;
    }

    pub fn key(&mut self, module: ModuleId, action: KeyAction, accepts_input: bool) {
        let current = self.enabled(module);
        let bind = match module {
            ModuleId::Coordinates => &mut self.coordinates_bind,
            ModuleId::Fps => &mut self.fps_bind,
        };
        if let Some(enabled) = bind.apply(BindingMode::Toggle, action, current, accepts_input) {
            self.set_enabled(module, enabled);
        }
    }

    pub fn release_input(&mut self) {
        self.coordinates_bind.reset();
        self.fps_bind.reset();
    }

    pub fn panic(&mut self) {
        self.coordinates = false;
        self.fps = false;
        self.release_input();
    }

    // A diagnostic tick counter, never the host's simulation/world clock.
    pub fn tick(&mut self) -> bool {
        let first = self.ticks == 0;
        self.ticks = self.ticks.saturating_add(1);
        first
    }

    pub fn hud(&self, snapshot: Snapshot) -> Vec<HudLine> {
        if !snapshot.in_world || !snapshot.show_hud {
            return Vec::new();
        }
        let mut lines = Vec::with_capacity(4);
        if self.coordinates {
            lines.extend(self.layout.lines(snapshot.position));
        }
        if self.fps && snapshot.fps.is_finite() {
            lines.push(HudLine {
                label: "FPS".into(),
                value: format!("{:.0}", snapshot.fps.max(0.0)),
            });
        }
        lines
    }
}

std::thread_local! {
    // Only called for the window's Game by adapters. The integrated server has
    // a separate thread and is rejected before accessing this state.
    static CLIENT: std::cell::RefCell<Runtime> = std::cell::RefCell::new(Runtime::default());
}

pub fn with_client<T>(action: impl FnOnce(&mut Runtime) -> T) -> T {
    CLIENT.with_borrow_mut(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            position: [-1.5, 64.2, 10.9],
            fps: 143.8,
            in_world: true,
            show_hud: true,
        }
    }

    #[test]
    fn disabled_by_default_and_panic_clears_all_state() {
        let mut runtime = Runtime::default();
        assert!(runtime.hud(snapshot()).is_empty());
        runtime.key(ModuleId::Coordinates, KeyAction::Press, true);
        runtime.set_enabled(ModuleId::Fps, true);
        assert_eq!(runtime.hud(snapshot()).len(), 2);
        runtime.panic();
        assert!(runtime.hud(snapshot()).is_empty());
        runtime.key(ModuleId::Coordinates, KeyAction::Press, true);
        assert!(runtime.enabled(ModuleId::Coordinates));
    }

    #[test]
    fn screens_and_f1_hide_draws_without_changing_preferences() {
        let mut runtime = Runtime::default();
        runtime.set_enabled(ModuleId::Coordinates, true);
        let mut data = snapshot();
        data.in_world = false;
        assert!(runtime.hud(data).is_empty());
        data.in_world = true;
        data.show_hud = false;
        assert!(runtime.hud(data).is_empty());
        assert!(runtime.enabled(ModuleId::Coordinates));
    }

    #[test]
    fn tick_diagnostic_only_fires_once() {
        let mut runtime = Runtime::default();
        assert!(runtime.tick());
        assert!(!runtime.tick());
        runtime.panic();
        assert!(!runtime.tick());
    }

    #[test]
    fn independent_runtime_instances_do_not_share_module_state() {
        let mut first = Runtime::default();
        let second = Runtime::default();
        first.set_enabled(ModuleId::Fps, true);
        assert!(!second.enabled(ModuleId::Fps));
    }
}
