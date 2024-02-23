use std::collections::{HashMap, VecDeque};

use crate::interface::{
    Container, FrameDescription, Object, Point, Rect, Transform, Transformation2D,
};
use crate::renderer::shader_structs::ColorVertex;

use bytemuck::Pod;
use image::DynamicImage;
use lyon::lyon_tessellation::{
    BuffersBuilder, LineCap, StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
};
use lyon::path::Path;
use wgpu::{BindGroup, Device, RenderPipeline};

use wgpu::{util::DeviceExt, Buffer};
use winit::dpi::PhysicalSize;

use super::shader_structs::TextureVertex;

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

const RECT: &[u16; 6] = &[0, 1, 2, 0, 2, 3];

/// Convert a frame description into a list of Objects to render, along with the transformations to apply
/// to those objects. Objects are sorted from least to greatest z depth. Nodes that have visible set to false,
/// and their children, are filtered out.
fn frame_description_to_objects(frame: &FrameDescription) -> Vec<(&Object, Transformation2D)> {
    use Container::{Leaf, Node};
    let mut queue = VecDeque::from_iter(
        frame
            .things
            .iter()
            .map(|node| (node, Transform::identity().to_transformation(), 0.0)),
    );
    let mut objects = vec![];
    while let Some((node, global_transform, z)) = queue.pop_front() {
        if !node.visible {
            continue;
        }
        let z = node.z + z;
        for child in &node.children {
            let transformation = global_transform.multiply(&node.transform.to_transformation());
            match child {
                Node(node) => queue.push_front((node, transformation, z)),
                Leaf(object) => objects.push((object, transformation, z)),
            }
        }
    }
    objects.sort_by(|(_, _, a), (_, _, b)| a.total_cmp(b));
    objects
        .into_iter()
        .map(|(object, transformation, _)| (object, transformation))
        .collect()
}

impl<'a> RenderData<'a> {
    pub fn new(
        device: &Device,
        resolution: PhysicalSize<u32>,
        frame_description: &FrameDescription,
        images: &HashMap<u32, DynamicImage>,
        triangle_pipeline: &'a RenderPipeline,
        texture_pipeline: &'a RenderPipeline,
        texture_pipeline_bind_groups: &'a HashMap<u32, BindGroup>,
    ) -> Self {
        let mut texture_vertices_index = 0;
        let mut texture_vertices = vec![];

        let mut triangle_vertices = vec![];
        let mut triangle_indices = vec![];

        let mut render_order = vec![];
        for (object, transformation) in frame_description_to_objects(frame_description) {
            match object {
                Object::Bezier(bez) => {
                    fn pt(pt: &Point) -> lyon::geom::Point<f32> {
                        [pt.x, pt.y].into()
                    }

                    fn mid(a: &Point, b: &Point) -> Point {
                        Point {
                            x: (a.x + b.x) * 0.5,
                            y: (a.y + b.y) * 0.5,
                        }
                    }

                    fn dist(a : [f32; 2], b : [f32; 2]) -> f32 {
                        (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)
                    }

                    let origin = transformation.apply_to([0.0, 0.0]);
                    let unit_x = transformation.apply_to([1.0, 0.0]);
                    let unit_y = transformation.apply_to([0.0, 1.0]);
                    let x_scaling = dist(unit_x, origin);
                    let y_scaling = dist(unit_y, origin);
                    let scale = x_scaling.min(y_scaling);

                    let points: Vec<_> = bez
                        .points
                        .iter()
                        .map(|p| transformation.apply_to([p.x, p.y]))
                        .map(|p| Point { x: p[0], y: p[1] })
                        .collect();

                    let mut path_builder = Path::builder();
                    path_builder.begin(pt(&points[0]));
                    path_builder.cubic_bezier_to(
                        pt(&mid(&points[0], &points[1])),
                        pt(&mid(&points[1], &points[2])),
                        pt(&points[2]),
                    );
                    path_builder.end(false);
                    let path = path_builder.build();

                    let mut tesselator = StrokeTessellator::new();
                    let mut geometry: VertexBuffers<_, u16> = VertexBuffers::new();

                    let stroke_options = StrokeOptions::default()
                        .with_line_width(bez.thickness * scale)
                        .with_line_cap(LineCap::Round)
                        .with_tolerance(0.001);

                    tesselator
                        .tessellate_path(
                            &path,
                            &stroke_options,
                            &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                                ColorVertex {
                                    position: vertex.position().to_array(),
                                    color: bez.color.to_linear_rgb(),
                                }
                            }),
                        )
                        .expect("Error during tessellation");

                    let before = (triangle_indices.len(), triangle_vertices.len());
                    triangle_indices.extend(geometry.indices.into_iter());
                    triangle_vertices.append(&mut geometry.vertices);
                    let after = (triangle_indices.len(), triangle_vertices.len());

                    render_order.push(RenderSettings {
                        pipeline: triangle_pipeline,
                        bind_group: None,
                        vertices_buffer_id: 2,
                        indices_buffer_id: 3,
                        indices_range: (before.0 as u32, after.0 as u32),
                        vertices_range: (before.1 as u64, after.1 as u64),
                    });
                }
                Object::Img(img) => {
                    let tex = &images[&img.id];
                    let (w, h) = (tex.width(), tex.height());
                    let img_aspect_ratio = h as f32 / w as f32;
                    let res_aspect_ratio = resolution.width as f32 / resolution.height as f32;
                    let (w, h) = (1.0, img_aspect_ratio * res_aspect_ratio);
                    let subrect = if let Some(subrect) = &img.subrect {
                        subrect
                    } else {
                        &Rect {
                            x: 0.0,
                            y: 0.0,
                            w: 1.0,
                            h: 1.0,
                        }
                    };
                    texture_vertices.extend(
                        [
                            TextureVertex {
                                position: [-w / 2.0, -h / 2.0],
                                tex_coords: [subrect.x, 1.0 - subrect.y],
                            },
                            TextureVertex {
                                position: [w / 2.0, -h / 2.0],
                                tex_coords: [subrect.w, 1.0 - subrect.y],
                            },
                            TextureVertex {
                                position: [w / 2.0, h / 2.0],
                                tex_coords: [subrect.w, 1.0 - subrect.h],
                            },
                            TextureVertex {
                                position: [-w / 2.0, h / 2.0],
                                tex_coords: [subrect.x, 1.0 - subrect.h],
                            },
                        ]
                        .into_iter()
                        .map(|texture_vertex| TextureVertex {
                            position: transformation.apply_to(texture_vertex.position),
                            tex_coords: texture_vertex.tex_coords,
                        }),
                    );

                    let texture_settings = RenderSettings {
                        pipeline: texture_pipeline,
                        bind_group: Some(&texture_pipeline_bind_groups[&img.id]),
                        vertices_buffer_id: 0,
                        indices_buffer_id: 1,
                        vertices_range: (texture_vertices_index, texture_vertices_index + 4),
                        indices_range: (0, 6),
                    };
                    texture_vertices_index += 4;
                    render_order.push(texture_settings);
                }
                Object::Text(_) => panic!("booglanim does not support text yet, bogly boogly jiggly joogly"),
            }
        }
        let texture_vertices = slice_to_buffer(device, &texture_vertices, VERTEX);
        let texture_indicies = slice_to_buffer(device, RECT, INDEX);
        let triangle_vertices = slice_to_buffer(device, &triangle_vertices, VERTEX);
        let triangle_indices = slice_to_buffer(device, &triangle_indices, INDEX);

        Self {
            buffers: vec![
                texture_vertices,
                texture_indicies,
                triangle_vertices,
                triangle_indices,
            ],
            render_order,
        }
    }
}
