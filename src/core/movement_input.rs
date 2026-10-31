// Deterministic per-tick movement input arbitration with one final writer.
// Requests only live for the current tick; equal priorities are resolved by owner.
//
// The host keeps canonical gameplay keys (after rebinding, controllers and
// the control socket) in `Game::keys`, with sneak/sprint derived from its
// toggle mappings. `Writer` reconstructs what the user is holding from that
// host state, applies module requests on top, and is the only code that
// writes synthetic movement keys. When no module requests an axis it hands
// the axis back with the user's state, never a raw physical key reading.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Axis {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Sneak,
    Sprint,
}
impl Axis {
    pub const ALL: [Axis; 7] = [
        Axis::Forward,
        Axis::Back,
        Axis::Left,
        Axis::Right,
        Axis::Jump,
        Axis::Sneak,
        Axis::Sprint,
    ];
    fn index(self) -> usize {
        self as usize
    }
    /// Sneak and sprint are host toggle mappings whose state the host keeps
    /// separately from the key set.
    fn toggle(self) -> Option<usize> {
        match self {
            Axis::Sneak => Some(0),
            Axis::Sprint => Some(1),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Owner {
    Sprint,
    Parkour,
    AntiAfk,
    WTap,
    JumpReset,
    Scaffold,
    WindCharge,
    NoFall,
    AntiFireball,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    pub axis: Axis,
    pub owner: Owner,
    pub priority: u8,
    pub pressed: bool,
}

#[derive(Default)]
pub struct Overrides {
    requests: Vec<Request>,
}

impl Overrides {
    pub fn request(&mut self, request: Request) {
        self.requests.push(request);
    }
    pub fn resolve(&self, axis: Axis, original: bool) -> bool {
        self.requests
            .iter()
            .filter(|r| r.axis == axis)
            .max_by_key(|r| (r.priority, r.owner))
            .map_or(original, |r| r.pressed)
    }
}

/// Host input state read at the writer's point in the tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostView {
    /// Whether each axis' canonical key is in the host key set.
    pub keys: [bool; 7],
    /// The host's sneak and sprint toggle-mapping state (`ToggleKeys::is_down`).
    pub toggles: [bool; 2],
}
impl HostView {
    pub fn key(&self, axis: Axis) -> bool {
        self.keys[axis.index()]
    }
}

#[derive(Clone, Copy, Debug)]
struct Written {
    /// The user's state when the synthetic value was written.
    user: bool,
    /// The value written into the host key set.
    wrote: bool,
    /// The last host-processed key event for this axis since the write.
    touched: Option<bool>,
    /// A key press inserted directly into the key set (the control socket),
    /// bypassing the host's toggle mappings, since the write.
    direct: bool,
}

/// Per-client final writer for movement keys.
#[derive(Debug, Default)]
pub struct Writer {
    written: [Option<Written>; 7],
}
impl Writer {
    /// Records a key event the host is about to process for this axis (a
    /// rebound key, controller button or socket key, all already canonical).
    pub fn host_event(&mut self, axis: Axis, pressed: bool) {
        if let Some(written) = &mut self.written[axis.index()] {
            written.touched = Some(pressed);
        }
    }
    /// Records a key the host inserts directly into its key set, outside the
    /// toggle mappings (control-socket key presses).
    pub fn direct_key(&mut self, axis: Axis) {
        if let Some(written) = &mut self.written[axis.index()] {
            written.touched = Some(true);
            written.direct = true;
        }
    }
    /// Whether this writer currently overrides the axis.
    pub fn owns(&self, axis: Axis) -> bool {
        self.written[axis.index()].is_some()
    }
    /// What the user is holding, independent of synthetic writes.
    pub fn user(&self, axis: Axis, view: &HostView) -> bool {
        let written = self.written[axis.index()];
        // Someone else in the host changed the key we wrote (for example a
        // screen clearing all keys): that change is authoritative.
        let changed = written.is_some_and(|w| view.key(axis) != w.wrote);
        if let Some(slot) = axis.toggle() {
            // Key events flip toggle mappings, so their own state is the
            // user's; a key held outside them (the control socket inserts
            // keys directly) still counts unless this writer put it there.
            let outside = written.is_none_or(|w| w.direct) || changed;
            return view.toggles[slot] || (view.key(axis) && outside);
        }
        match written {
            None => view.key(axis),
            Some(_) if changed => view.key(axis),
            Some(w) => w.touched.unwrap_or(w.user),
        }
    }
    /// Resolves every axis and returns the writes the host must apply. Axes
    /// that are neither requested nor previously overridden are untouched.
    pub fn resolve(&mut self, overrides: &Overrides, view: &HostView) -> Vec<(Axis, bool)> {
        let mut writes = Vec::new();
        for axis in Axis::ALL {
            let user = self.user(axis, view);
            let wanted = overrides.resolve(axis, user);
            let slot = &mut self.written[axis.index()];
            if wanted != user {
                *slot = Some(Written {
                    user,
                    wrote: wanted,
                    touched: None,
                    direct: false,
                });
                writes.push((axis, wanted));
            } else if slot.take().is_some() {
                writes.push((axis, user));
            }
        }
        writes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprint(pressed: bool) -> Request {
        Request {
            axis: Axis::Sprint,
            owner: Owner::Sprint,
            priority: 1,
            pressed,
        }
    }
    fn jump() -> Request {
        Request {
            axis: Axis::Jump,
            owner: Owner::Parkour,
            priority: 1,
            pressed: true,
        }
    }
    /// Resolves and applies the writes to a host view as the adapter does.
    fn step(writer: &mut Writer, requests: &Overrides, view: &mut HostView) {
        let writes = writer.resolve(requests, view);
        apply(view, &writes);
    }
    /// Applies writes to a host view as the adapter does.
    fn apply(view: &mut HostView, writes: &[(Axis, bool)]) {
        for (axis, pressed) in writes {
            view.keys[axis.index()] = *pressed;
        }
    }

    #[test]
    fn arbitration_preserves_unclaimed_inputs_and_prefers_priority() {
        let mut overrides = Overrides::default();
        overrides.request(sprint(true));
        overrides.request(Request {
            priority: 2,
            ..sprint(false)
        });
        assert!(!overrides.resolve(Axis::Sprint, true));
        assert!(overrides.resolve(Axis::Jump, true));
        assert!(!overrides.resolve(Axis::Forward, false));
    }

    #[test]
    fn untouched_axes_are_never_written() {
        let mut writer = Writer::default();
        let view = HostView {
            keys: [true, false, false, false, true, false, false],
            toggles: [false, false],
        };
        assert!(writer.resolve(&Overrides::default(), &view).is_empty());
    }

    #[test]
    fn releasing_synthetic_sprint_keeps_a_rebound_or_toggled_sprint() {
        let mut writer = Writer::default();
        let mut view = HostView::default();
        let mut requests = Overrides::default();
        requests.request(sprint(true));
        let writes = writer.resolve(&requests, &view);
        assert_eq!(writes, [(Axis::Sprint, true)]);
        apply(&mut view, &writes);
        // The user now holds a rebound sprint key (or toggled sprint on):
        // the host reports it through its toggle mapping, not Left Ctrl.
        view.toggles[1] = true;
        assert!(
            writer
                .resolve(&requests, &view)
                .contains(&(Axis::Sprint, true))
        );
        // Module disabled: the user's sprint stays.
        let writes = writer.resolve(&Overrides::default(), &view);
        assert!(writes.iter().all(|w| *w != (Axis::Sprint, false)));
        assert!(!writer.owns(Axis::Sprint));
        // And with nobody holding it, disabling does release the synthetic key.
        let mut writer = Writer::default();
        let mut view = HostView::default();
        step(&mut writer, &requests, &mut view);
        assert_eq!(
            writer.resolve(&Overrides::default(), &view),
            [(Axis::Sprint, false)]
        );
    }

    #[test]
    fn sprint_held_outside_toggle_mappings_is_not_taken_over() {
        let mut writer = Writer::default();
        let mut view = HostView::default();
        view.keys[Axis::Sprint.index()] = true; // socket-held, toggles untouched
        let mut requests = Overrides::default();
        requests.request(sprint(true));
        assert!(writer.resolve(&requests, &view).is_empty());
        assert!(writer.resolve(&Overrides::default(), &view).is_empty());
        assert!(view.key(Axis::Sprint));
        // A socket press while the module owns sprint survives its release.
        let mut view = HostView::default();
        step(&mut writer, &requests, &mut view);
        writer.direct_key(Axis::Sprint);
        step(&mut writer, &Overrides::default(), &mut view);
        assert!(view.key(Axis::Sprint));
        let mut writer = Writer::default();
        // Toggle mode: a second press turns sprint off; the toggle state wins.
        let mut view = HostView::default();
        view.toggles[1] = true;
        view.keys[Axis::Sprint.index()] = true;
        step(&mut writer, &requests, &mut view);
        writer.host_event(Axis::Sprint, true);
        view.toggles[1] = false;
        view.keys[Axis::Sprint.index()] = false; // the host's sync_toggle
        step(&mut writer, &requests, &mut view);
        assert!(view.key(Axis::Sprint), "module keeps sprinting");
        step(&mut writer, &Overrides::default(), &mut view);
        assert!(
            !view.key(Axis::Sprint),
            "toggled off by the user, released by the module"
        );
    }

    #[test]
    fn parkour_tap_preserves_a_jump_pressed_while_it_was_owned() {
        let mut writer = Writer::default();
        let mut view = HostView::default();
        let mut requests = Overrides::default();
        requests.request(jump());
        step(&mut writer, &requests, &mut view);
        assert!(view.key(Axis::Jump));
        // The user presses (a possibly rebound) jump while the tap is held.
        writer.host_event(Axis::Jump, true);
        step(&mut writer, &Overrides::default(), &mut view);
        assert!(
            view.key(Axis::Jump),
            "user's jump must survive the tap ending"
        );
        assert!(!writer.owns(Axis::Jump));
    }

    #[test]
    fn release_during_ownership_and_host_clears_are_respected() {
        let mut requests = Overrides::default();
        requests.request(jump());
        // User held jump, Parkour does not need to write; nothing owned.
        let mut writer = Writer::default();
        let mut view = HostView::default();
        view.keys[Axis::Jump.index()] = true;
        assert!(writer.resolve(&requests, &view).is_empty());
        // Synthetic tap, then the user presses and releases before it ends.
        let mut view = HostView::default();
        step(&mut writer, &requests, &mut view);
        writer.host_event(Axis::Jump, true);
        writer.host_event(Axis::Jump, false);
        view.keys[Axis::Jump.index()] = false; // the host processed the release
        step(&mut writer, &Overrides::default(), &mut view);
        assert!(!view.key(Axis::Jump));
        assert!(!writer.owns(Axis::Jump));
        // A screen clears every key while a tap is owned: nothing is re-added.
        let mut view = HostView::default();
        step(&mut writer, &requests, &mut view);
        view.keys = [false; 7];
        step(&mut writer, &Overrides::default(), &mut view);
        assert!(!view.key(Axis::Jump));
    }

    #[test]
    fn suppression_restores_a_held_key_when_it_ends() {
        let mut writer = Writer::default();
        let mut view = HostView::default();
        view.keys[Axis::Forward.index()] = true;
        let mut requests = Overrides::default();
        requests.request(Request {
            axis: Axis::Forward,
            owner: Owner::Parkour,
            priority: 2,
            pressed: false,
        });
        let writes = writer.resolve(&requests, &view);
        assert_eq!(writes, [(Axis::Forward, false)]);
        apply(&mut view, &writes);
        assert_eq!(
            writer.resolve(&Overrides::default(), &view),
            [(Axis::Forward, true)]
        );
    }
}
