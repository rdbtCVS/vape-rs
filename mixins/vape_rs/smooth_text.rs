// The click GUI's text in Horizon's bundled interface font, rasterized at the
// final on-screen size instead of scaling the bitmap game font. Sizes and
// positions are in GUI pixels; the line box is centred like the theme's.
impl super::Ui<'_> {
    fn vape_rs_dp(&self) -> f32 {
        self.scale / self.density
    }
    pub fn vape_rs_measure(&self, text: &str, size: f32) -> f32 {
        let k = self.vape_rs_dp();
        self.measure(super::Face::Regular, size * k, text) / k
    }
    pub fn vape_rs_text(&mut self, text: &str, x: f32, y: f32, size: f32, line: f32, color: u32) {
        let k = self.vape_rs_dp();
        self.text_line(super::Face::Regular, size * k, line * k, x * k, y * k, color, text);
    }
}
