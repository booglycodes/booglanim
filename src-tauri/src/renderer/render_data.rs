use std::collections::{HashMap, VecDeque};

use crate::interface::{FrameDescription, Object, Point};

use super::shader_structs::{ColorVertex, TextureVertex};

use bytemuck::Pod;
use image::DynamicImage;
use lyon::geom::euclid::{Point2D, UnknownUnit};
use lyon::lyon_tessellation::{
    BuffersBuilder, LineCap, StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
};
use lyon::math::point;
use lyon::path::Path;
use wgpu::{BindGroup, Device, RenderPipeline};

use wgpu::{util::DeviceExt, Buffer};
use winit::dpi::PhysicalSize;

#[derive(Debug, Clone, Copy)]
pub struct RenderSettings<'a> {
    pub pipeline: &'a RenderPipeline,
    pub bind_group: Option<&'a BindGroup>,
    pub vertices_buffer_id: usize,
    pub indices_buffer_id: usize,
    pub vertices_range: (u64, u64),
    pub indices_range: (u32, u32),
}

pub struct RenderData<'a> {
    pub buffers: Vec<Buffer>,
    pub render_order: Vec<RenderSettings<'a>>,
}

pub const VERTEX: (&str, wgpu::BufferUsages) = ("Vertex Buffer", wgpu::BufferUsages::VERTEX);
pub const INDEX: (&str, wgpu::BufferUsages) = ("Index Buffer", wgpu::BufferUsages::INDEX);
fn slice_to_buffer<T: Pod>(
    device: &Device,
    slice: &[T],
    kind: (&str, wgpu::BufferUsages),
) -> Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(kind.0),
        contents: bytemuck::cast_slice(slice),
        usage: kind.1,
    })
}

fn mid(a: &Point, b: &Point) -> Point2D<f32, UnknownUnit> {
    ((a.x + b.x) * 0.5, (a.y + b.y) * 0.5).into()
}

const RECT: &[u16; 6] = &[0, 1, 2, 0, 2, 3];
impl<'a> RenderData<'a> {
    pub fn new(
        device: &Device,
        resolution: PhysicalSize<u32>,
        frame: &FrameDescription,
        images: &HashMap<u32, DynamicImage>,
        triangle_pipeline: &'a RenderPipeline,
        texture_pipeline: &'a RenderPipeline,
        texture_pipeline_bind_groups: &'a HashMap<u32, BindGroup>,
    ) -> Self {
        (|| {
            let mut texture_vertices_index = 0;
            let mut texture_vertices = vec![];

            let mut triangle_vertices = vec![];
            let mut triangle_indices = vec![];

            let mut render_order = vec![];
            let mut queue = VecDeque::from_iter(frame.things.iter().map(|t| (t, (Point { x : t.pos.x, y : t.pos.y}, t.scale))));
            while let Some((transform, (global_pos, global_scale))) = queue.pop_front() {
                for child in &transform.children {
                    match child {
                        Object::Transform(t) => {
                            queue.push_front((t, (Point { x: global_pos.x + t.pos.x * global_scale, y: global_pos.y + t.pos.y * global_scale }, global_scale * t.scale)))
                        },
                        Object::Bezier(bezier) => {
                            fn pt(p : &Point) -> Point2D<f32, UnknownUnit> {
                                point(p.x, p.y)
                            }
                            let limb = &bezier.points;
        
                            let mut path_builder = Path::builder();
                            path_builder.begin(pt(&limb[0]));
                            path_builder.cubic_bezier_to(
                                mid(&limb[0], &limb[1]),
                                mid(&limb[1], &limb[2]),
                                pt(&limb[2]),
                            );
                            path_builder.end(false);
                            let path = path_builder.build();
        
                            let mut tesselator = StrokeTessellator::new();
                            let mut geometry: VertexBuffers<_, u16> = VertexBuffers::new();
        
                            let stroke_options = StrokeOptions::default()
                                .with_line_width(bezier.thickness)
                                .with_line_cap(LineCap::Round)
                                .with_tolerance(0.001);
        
                            tesselator
                                .tessellate_path(
                                    &path,
                                    &stroke_options,
                                    &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                                        ColorVertex {
                                            position: vertex.position().to_array(),
                                            color: bezier.color.to_linear_rgb(),
                                        }
                                    }),
                                )
                                .expect("Error during tessellation");
        
                            render_order.push(RenderSettings {
                                pipeline: triangle_pipeline,
                                bind_group: None,
                                vertices_buffer_id: 2,
                                indices_buffer_id: 3,
                                vertices_range: (
                                    triangle_indices.len() as u64,
                                    triangle_indices.len() as u64 + geometry.indices.len() as u64,
                                ),
                                indices_range: (
                                    triangle_vertices.len() as u32,
                                    triangle_vertices.len() as u32 + geometry.vertices.len() as u32,
                                ),
                            });
                            let triangle_indices_len = triangle_indices.len();
                            triangle_indices.extend(geometry.indices.iter().map(|x| x + triangle_indices_len as u16));
                            triangle_vertices.append(&mut geometry.vertices);
                        },
                        Object::Img(img) => {
                            let (x, y) = (global_pos.x, global_pos.y);
                            let scale = transform.scale * global_scale;
                            let image = &images[&img.id];
                            let (w, h) = (image.width(), image.height());
                            let aspect_ratio = (h as f32) / (w as f32);
                            let resolution_aspect_ratio =
                                (resolution.width as f32) / (resolution.height as f32);
                            let (w, h) = (scale, scale * aspect_ratio * resolution_aspect_ratio);
                            texture_vertices.append(&mut vec![
                                TextureVertex {
                                    position: [x - w / 2.0, y - h / 2.0],
                                    tex_coords: [0.0, 1.0],
                                },
                                TextureVertex {
                                    position: [x + w / 2.0, y - h / 2.0],
                                    tex_coords: [1.0, 1.0],
                                },
                                TextureVertex {
                                    position: [x + w / 2.0, y + h / 2.0],
                                    tex_coords: [1.0, 0.0],
                                },
                                TextureVertex {
                                    position: [x - w / 2.0, y + h / 2.0],
                                    tex_coords: [0.0, 0.0],
                                },
                            ]);
            
                            let texture_settings = RenderSettings {
                                pipeline: texture_pipeline,
                                bind_group: Some(texture_pipeline_bind_groups.get(&img.id).unwrap()),
                                vertices_buffer_id: 0,
                                indices_buffer_id: 1,
                                vertices_range: (texture_vertices_index, texture_vertices_index + 4),
                                indices_range: (0, 6),
                            };
                            texture_vertices_index += 4;
                            render_order.push(texture_settings);
                        },
                        Object::Text(_) => {},
                    }
                }
            }
            let texture_vertices = slice_to_buffer(device, &texture_vertices, VERTEX);
            let texture_indicies = slice_to_buffer(device, RECT, INDEX);
            let triangle_vertices = slice_to_buffer(device, &triangle_vertices, VERTEX);
            let triangle_indices = slice_to_buffer(device, &triangle_indices, INDEX);

            Some(Self {
                buffers: vec![
                    texture_vertices,
                    texture_indicies,
                    triangle_vertices,
                    triangle_indices,
                ],
                render_order,
            })
        })()
        .unwrap()
    }
}
