// Lines drawn through walls. The client's frame passes its depth-tested lines
// and then its always-on-top lines to `camera_relative`; the fishing-line hook
// marks through-wall lines with a negative width. The first call drops them
// from the depth-tested set, and the next call appends them to the on-top set.
//
// Every call also records the frame's real camera (third person and freecam
// included) and view rotation, which the HUD-time overlay projects with, and
// a line starting at NaN (Tracers) starts one block in front of that camera.
use super::Line;
use core::cell::{Cell, RefCell};
use core::ops::ControlFlow;

thread_local! {
    static PENDING: RefCell<Vec<Line>> = const { RefCell::new(Vec::new()) };
    static CAMERA: Cell<Option<([f64; 3], glam::Mat4)>> = const { Cell::new(None) };
}

/// The camera position and view rotation of the frame being drawn.
pub fn frame_camera() -> Option<([f64; 3], glam::Mat4)> {
    CAMERA.with(Cell::get)
}

pub(crate) fn camera_relative(lines: &[Line], camera: [f64; 3], view: glam::Mat4) -> ControlFlow<Vec<[f32; 8]>> {
    CAMERA.with(|c| c.set(Some((camera, view))));
    if lines.iter().any(|l| l.width < 0.0) {
        let (marked, plain): (Vec<Line>, Vec<Line>) = lines.iter().copied().partition(|l| l.width < 0.0);
        let out = super::camera_relative(&plain, camera, view);
        let forward = view.inverse().transform_vector3(glam::Vec3::NEG_Z).as_dvec3();
        let centre = [camera[0] + forward.x, camera[1] + forward.y, camera[2] + forward.z];
        PENDING.with(|p| {
            *p.borrow_mut() = marked
                .into_iter()
                .map(|l| Line { width: -l.width, start: if l.start[0].is_nan() { centre } else { l.start }, ..l })
                .collect();
        });
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
