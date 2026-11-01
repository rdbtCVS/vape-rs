fn motion_blur(renderer: &crate::gfx::Renderer, encoder: &mut wgpu::CommandEncoder, target: &wgpu::Texture, view: &wgpu::TextureView) {
    vape_motion_blur::apply(renderer, encoder, target, view);
}
