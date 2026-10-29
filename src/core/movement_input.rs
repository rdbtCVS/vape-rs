// Deterministic per-tick movement input arbitration.
// Requests only live for the current tick; equal priorities are resolved by owner.

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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Owner {
    Sprint,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitration_preserves_unclaimed_inputs_and_prefers_priority() {
        let mut overrides = Overrides::default();
        overrides.request(Request {
            axis: Axis::Sprint,
            owner: Owner::Sprint,
            priority: 1,
            pressed: true,
        });
        overrides.request(Request {
            axis: Axis::Sprint,
            owner: Owner::Sprint,
            priority: 2,
            pressed: false,
        });
        assert!(!overrides.resolve(Axis::Sprint, true));
        assert!(overrides.resolve(Axis::Jump, true));
        assert!(!overrides.resolve(Axis::Forward, false));
    }
}
