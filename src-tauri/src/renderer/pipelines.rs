use super::{
    shader_structs::{ColorVertex, TextureVertex},
    texture::Texture,
};
use image::DynamicImage;
use std::collections::HashMap;
use wgpu::{
    BindGroup, PipelineLayout, RenderPipeline, RenderPipelineDescriptor, ShaderModule,
    VertexBufferLayout, ColorTargetState, TextureFormat, BindGroupLayout,
};

pub fn screen_pipeline(
    device: &wgpu::Device,
    format : TextureFormat,
) -> (RenderPipeline, BindGroupLayout) {
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
        label: Some("texture_bind_group_layout"),
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/screen.wgsl").into()),
    });

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let vertex_buffers = [TextureVertex::desc()];
    let targets = [Some(color_target_state(format))];
    let pipeline_descriptor = render_pipeline_descriptor(
        &render_pipeline_layout,
        &shader,
        &vertex_buffers,
        &targets
    );
    (device.create_render_pipeline(&pipeline_descriptor), bind_group_layout)
}


pub fn texture_pipeline(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format : TextureFormat,
    images: &HashMap<u64, DynamicImage>,
) -> (RenderPipeline, HashMap<u64, BindGroup>) {
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
        label: Some("texture_bind_group_layout"),
    });

    let textures: HashMap<_, _> = images
        .iter()
        .map(|(&id, img)| (id, Texture::from_image(&device, &queue, img, None)))
        .collect();

    let bind_groups: HashMap<_, _> = textures
        .iter()
        .map(|(&id, texture)| {
            (
                id,
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&texture.sampler),
                        },
                    ],
                    label: Some("diffuse_bind_group"),
                }),
            )
        })
        .collect();

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/img.wgsl").into()),
    });

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let vertex_buffers = [TextureVertex::desc()];
    let targets = [Some(color_target_state(format))];
    let pipeline_descriptor = render_pipeline_descriptor(
        &render_pipeline_layout,
        &shader,
        &vertex_buffers,
        &targets
    );
    let render_pipeline = device.create_render_pipeline(&pipeline_descriptor);
    (render_pipeline, bind_groups)
}

pub fn triangle_pipeline(
    device: &wgpu::Device,
    format : TextureFormat
) -> RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/color.wgsl").into()),
    });

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let vertex_buffers = [ColorVertex::desc()];
    let targets = [Some(color_target_state(format))];
    let pipeline_descriptor = render_pipeline_descriptor(
        &render_pipeline_layout,
        &shader,
        &vertex_buffers,
        &targets
    );
    device.create_render_pipeline(&pipeline_descriptor)
}

fn color_target_state(format : TextureFormat) -> ColorTargetState {
    ColorTargetState {
        format,
        blend: Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        }),
        write_mask: wgpu::ColorWrites::ALL,
    }
}

fn render_pipeline_descriptor<'a>(
    render_pipeline_layout: &'a PipelineLayout,
    shader: &'a ShaderModule,
    vertex_buffers: &'a [VertexBufferLayout<'a>],
    targets: &'a [Option<ColorTargetState>]
) -> RenderPipelineDescriptor<'a> {
    RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            buffers: vertex_buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "fs_main",
            targets,
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Cw,
            cull_mode: None,
            // Setting this to anything other than Fill requires Features::POLYGON_MODE_LINE
            // or Features::POLYGON_MODE_POINT
            polygon_mode: wgpu::PolygonMode::Fill,
            // Requires Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            // Requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        // If the pipeline will be used with a multiview render pass, this
        // indicates how many array layers the attachments will have.
        multiview: None,
    }
}
