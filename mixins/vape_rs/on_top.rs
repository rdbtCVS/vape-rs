// Lines drawn through walls. The client's frame passes its depth-tested lines
// and then its always-on-top lines to `camera_relative`; the fishing-line hook
// marks through-wall lines with a negative width. The first call drops them
// from the depth-tested set, and the next call appends them to the on-top set.
use super::Line;
use core::cell::RefCell;
use core::ops::ControlFlow;

thread_local! {
    static PENDING: RefCell<Vec<Line>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn camera_relative(lines: &[Line], camera: [f64; 3], view: glam::Mat4) -> ControlFlow<Vec<[f32; 8]>> {
    if lines.iter().any(|l| l.width < 0.0) {
        let (marked, plain): (Vec<Line>, Vec<Line>) = lines.iter().copied().partition(|l| l.width < 0.0);
        let out = super::camera_relative(&plain, camera, view);
        PENDING.with(|p| *p.borrow_mut() = marked.into_iter().map(|l| Line { width: -l.width, ..l }).collect());
        return ControlFlow::Break(out);
    }
    let pending = PENDING.with(|p| core::mem::take(&mut *p.borrow_mut()));
    if pending.is_empty() {
        return ControlFlow::Continue(());
    }
    let mut all = lines.to_vec();
    all.extend(pending);
    ControlFlow::Break(super::camera_relative(&all, camera, view))
}
