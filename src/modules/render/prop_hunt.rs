// PropHunt (`PropHunt`): a white outline with a faint white fill around
// every falling block entity (the props), and a red outline with a faint
// red fill on each block a single block update changed, for 200 ticks
// (at most 1024 tracked), all through walls. The reference's 0..256 height
// bound is the old world's; here any height the packet carries counts.
use super::world::{Line, Point, Quad, fill_box, outline_box};

pub const MAX_TRACKED: usize = 1024;
pub const LIFETIME: u32 = 200;

#[derive(Debug, Default)]
pub struct PropHunt {
    blocks: Vec<([i32; 3], u32)>,
}

impl PropHunt {
    /// `onPacketReceive` for a `ClientboundBlockUpdatePacket`.
    pub fn block_update(&mut self, pos: [i32; 3]) {
        let inside = (-30_000_000..30_000_000).contains(&pos[0])
            && (-30_000_000..30_000_000).contains(&pos[2]);
        if inside && self.blocks.len() < MAX_TRACKED && !self.blocks.iter().any(|b| b.0 == pos) {
            self.blocks.push((pos, 0));
        }
    }
    /// `onTick`: age each block, dropping it at 200.
    pub fn tick(&mut self) {
        self.blocks.retain_mut(|(_, age)| {
            if *age >= LIFETIME {
                return false;
            }
            *age += 1;
            true
        });
    }
    pub fn clear(&mut self) {
        self.blocks.clear();
    }
    pub fn tracked(&self) -> usize {
        self.blocks.len()
    }
    /// A falling block's box: 0.98 wide and tall over its position.
    fn prop_box(at: [f64; 3]) -> ([f64; 3], [f64; 3]) {
        (
            [at[0] - 0.49, at[1], at[2] - 0.49],
            [at[0] + 0.49, at[1] + 0.98, at[2] + 0.49],
        )
    }
    pub fn lines(&self, props: &[[f64; 3]], out: &mut Vec<Line>) {
        for &at in props {
            let (min, max) = Self::prop_box(at);
            outline_box(out, Point(min), Point(max), 0xffff_ffff, 1.5);
        }
        for (pos, _) in &self.blocks {
            let min = pos.map(f64::from);
            outline_box(
                out,
                Point(min),
                Point(min.map(|v| v + 1.0)),
                0xffff_0000,
                1.5,
            );
        }
    }
    pub fn quads(&self, props: &[[f64; 3]], out: &mut Vec<Quad>) {
        for &at in props {
            let (min, max) = Self::prop_box(at);
            fill_box(out, Point(min), Point(max), 0x23ff_ffff);
        }
        for (pos, _) in &self.blocks {
            let min = pos.map(f64::from);
            fill_box(out, Point(min), Point(min.map(|v| v + 1.0)), 0x23ff_0000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_age_out_after_200_ticks() {
        let mut p = PropHunt::default();
        p.block_update([1, 64, 1]);
        p.block_update([1, 64, 1]);
        p.block_update([40_000_000, 0, 0]);
        assert_eq!(p.tracked(), 1);
        for _ in 0..200 {
            p.tick();
        }
        assert_eq!(p.tracked(), 1);
        p.tick();
        assert_eq!(p.tracked(), 0);
    }
}
