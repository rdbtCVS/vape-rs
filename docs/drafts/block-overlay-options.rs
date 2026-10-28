// DESIGN DRAFT ONLY — excluded from modules, native pack and compilation.
// Retained BlockOverlay visual parameters and geometry constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Options {
    /// Source `Overlay Color`: translucent fill, ARGB.
    pub overlay_color: u32,
    /// Source `Outline Color`: depth-tested edge stroke, ARGB.
    pub outline_color: u32,
    /// The source has no width value: both render paths use a fixed 2 px.
    pub line_width: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            overlay_color: 0x5fff_0000,
            outline_color: 0xc8ff_0000,
            line_width: 2,
        }
    }
}

/// The source expands selected bounds by this amount before drawing, avoiding
/// z-fighting with terrain while preserving collision/outline geometry.
pub const OUTLINE_EPSILON: f64 = 0.002;

pub fn expand(bounds: [f64; 6]) -> ([f64; 3], [f64; 3]) {
    (
        [
            bounds[0] - OUTLINE_EPSILON,
            bounds[1] - OUTLINE_EPSILON,
            bounds[2] - OUTLINE_EPSILON,
        ],
        [
            bounds[3] + OUTLINE_EPSILON,
            bounds[4] + OUTLINE_EPSILON,
            bounds[5] + OUTLINE_EPSILON,
        ],
    )
}
