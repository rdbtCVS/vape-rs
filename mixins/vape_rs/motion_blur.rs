// MotionBlur's blend (vape-rs), after the HUD's last draw of the frame:
// the presented frame, with every screen and the click GUI, is mixed with
// the previous blended frame by the reference's shader, and the result is
// both the frame and the next history. The client publishes one request a
// frame; none (a frame drawn without the client's) or Skip leaves the frame
// and keeps the history, Off frees the textures so the frame is the host's.
// Like the host's menu blur, it needs a frame it can copy (COPY_SRC).
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pass {
    pub blurriness: f32,
    pub velocity_factor: f32,
    pub smooth: bool,
    pub rgb: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    Off,
    Skip,
    Draw(Pass),
}

static REQUEST: Mutex<Option<Request>> = Mutex::new(None);
static BLUR: Mutex<Option<Blur>> = Mutex::new(None);

/// The next frame's blend.
pub fn request(request: Request) {
    *REQUEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(request);
}

const SHADER: &str = r"
struct Blur {
    blurriness: f32,
    velocity_factor: f32,
    smooth_blur: u32,
    render_rgb: u32,
};
@group(0) @binding(0) var current_texture: texture_2d<f32>;
@group(0) @binding(1) var history_texture: texture_2d<f32>;
@group(0) @binding(2) var linear: sampler;
@group(0) @binding(3) var<uniform> blur: Blur;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VertexOut;
    out.position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

fn blur_history(uv: vec2<f32>) -> vec4<f32> {
    let offset = 0.0006 * blur.velocity_factor;
    var sum = vec4<f32>(0.0);
    sum += textureSample(history_texture, linear, uv + vec2<f32>(-offset, 0.0)) * 0.25;
    sum += textureSample(history_texture, linear, uv + vec2<f32>(offset, 0.0)) * 0.25;
    sum += textureSample(history_texture, linear, uv + vec2<f32>(0.0, -offset)) * 0.25;
    sum += textureSample(history_texture, linear, uv + vec2<f32>(0.0, offset)) * 0.25;
    return sum;
}

struct FragmentOut {
    @location(0) frame: vec4<f32>,
    @location(1) history: vec4<f32>,
};

@fragment
fn fs_main(in: VertexOut) -> FragmentOut {
    let current = textureSample(current_texture, linear, in.uv);
    let history = textureSample(history_texture, linear, in.uv);
    var cur_blurriness = blur.blurriness;
    var blurred_history = history;
    if blur.velocity_factor > 0.0 {
        let base = blur.blurriness * 0.5;
        cur_blurriness = base + blur.velocity_factor * base;
        if blur.smooth_blur != 0u {
            blurred_history = blur_history(in.uv);
        }
    }
    var color = mix(current, blurred_history, cur_blurriness);
    if blur.render_rgb == 0u {
        color = mix(vec4<f32>(current.r), blurred_history, cur_blurriness);
    }
    var out: FragmentOut;
    out.frame = color;
    out.history = color;
    return out;
}
";

struct Blur {
    device: wgpu::Device,
    format: wgpu::TextureFormat,
    size: wgpu::Extent3d,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    current: wgpu::Texture,
    /// The ping-pong history; `read` is last frame's.
    history: [(wgpu::Texture, wgpu::TextureView); 2],
    /// Sampling `current` and `history[i]`.
    groups: [wgpu::BindGroup; 2],
    read: usize,
}

impl Blur {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, size: wgpu::Extent3d) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vape-rs motion blur"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vape-rs motion blur"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vape-rs motion blur"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        // Depth and blending off: the blend overwrites the frame.
        let target = Some(wgpu::ColorTargetState {
            format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("vape-rs motion blur"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[target.clone(), target],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        // GL_LINEAR, GL_CLAMP_TO_EDGE.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("vape-rs motion blur"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vape-rs motion blur"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = |label, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: usage | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let current = texture("vape-rs motion blur frame", wgpu::TextureUsages::empty());
        let current_view = current.create_view(&Default::default());
        let history = [0, 1].map(|_| {
            let t = texture(
                "vape-rs motion blur history",
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
            let view = t.create_view(&Default::default());
            (t, view)
        });
        let groups = [0, 1].map(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("vape-rs motion blur"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&current_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&history[i].1),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            })
        });
        Self {
            device: device.clone(),
            format,
            size,
            pipeline,
            uniform,
            current,
            history,
            groups,
            read: 0,
        }
    }
}

/// Blend the frame `target` (drawn through `view`, its non-sRGB view) by
/// this frame's request.
pub(super) fn apply(
    renderer: &super::Renderer,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::Texture,
    view: &wgpu::TextureView,
) {
    let request = REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take();
    let mut blur = BLUR.lock().unwrap_or_else(|e| e.into_inner());
    let pass = match request {
        Some(Request::Draw(pass)) => pass,
        Some(Request::Off) => {
            *blur = None;
            return;
        }
        Some(Request::Skip) | None => return,
    };
    let format = renderer.config.format.remove_srgb_suffix();
    let copy = wgpu::TextureUsages::COPY_SRC;
    if !renderer.config.usage.contains(copy)
        || !target.usage().contains(copy)
        || target.format().remove_srgb_suffix() != format
    {
        return;
    }
    let size = wgpu::Extent3d {
        depth_or_array_layers: 1,
        ..target.size()
    };
    if size.width == 0 || size.height == 0 {
        return;
    }
    let fresh = !blur
        .as_ref()
        .is_some_and(|b| b.device == renderer.device && b.format == format && b.size == size);
    if fresh {
        *blur = Some(Blur::new(&renderer.device, format, size));
    }
    let Some(blur) = blur.as_mut() else {
        return;
    };
    encoder.copy_texture_to_texture(target.as_image_copy(), blur.current.as_image_copy(), size);
    if fresh {
        // `first`: the history starts as this frame.
        encoder.copy_texture_to_texture(
            target.as_image_copy(),
            blur.history[blur.read].0.as_image_copy(),
            size,
        );
    }
    let words = [
        pass.blurriness.to_bits(),
        pass.velocity_factor.to_bits(),
        u32::from(pass.smooth),
        u32::from(pass.rgb),
    ];
    renderer
        .queue
        .write_buffer(&blur.uniform, 0, bytemuck::cast_slice(&words));
    let write = 1 - blur.read;
    let ops = wgpu::Operations {
        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        store: wgpu::StoreOp::Store,
    };
    let attachments = [
        Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            depth_slice: None,
            ops,
        }),
        Some(wgpu::RenderPassColorAttachment {
            view: &blur.history[write].1,
            resolve_target: None,
            depth_slice: None,
            ops,
        }),
    ];
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("vape-rs motion blur"),
        color_attachments: &attachments,
        ..Default::default()
    });
    pass.set_pipeline(&blur.pipeline);
    pass.set_bind_group(0, &blur.groups[blur.read], &[]);
    pass.draw(0..3, 0..1);
    drop(pass);
    blur.read = write;
}
