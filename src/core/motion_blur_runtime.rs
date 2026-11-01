// Runtime side of MotionBlur: the frame's blend, read once per presented
// frame before it is drawn.
use super::super::modules::render::motion_blur::{Frame, Request};
use super::{ModuleId, Runtime};

impl Runtime {
    pub fn motion_blur(&mut self, frame: Frame) -> Request {
        let enabled = self.enabled(ModuleId::MotionBlur);
        self.motion_blur.frame(enabled, &self.settings, frame)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::modules::render::motion_blur::Frame;
    use super::*;

    #[test]
    fn follows_the_module() {
        let mut r = Runtime::default();
        assert_eq!(r.motion_blur(Frame::default()), Request::Off);
        r.set_enabled(ModuleId::MotionBlur, true);
        assert!(matches!(r.motion_blur(Frame::default()), Request::Draw(_)));
        r.set_enabled(ModuleId::MotionBlur, false);
        assert_eq!(r.motion_blur(Frame::default()), Request::Off);
    }
}
