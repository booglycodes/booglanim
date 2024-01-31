use std::collections::HashMap;

use crate::color::Color;

use super::shader_structs::{ColorVertex, TextureVertex};

use bytemuck::Pod;
use image::DynamicImage;
use lyon::lyon_tessellation::{
    BuffersBuilder, LineCap, StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
};
use lyon::math::{point, Point};
use lyon::path::Path;
use serde_json::Value;
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

fn character_relative_to_canvas(
    p: (f32, f32),
    scale: f32,
    character: (f32, f32),
    resolution_aspect_ratio: f32,
) -> (f32, f32) {
    (
        scale * p.0 + character.0,
        resolution_aspect_ratio * -scale * p.1 + character.1,
    )
}

fn mid(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}

const RECT: &[u16; 6] = &[0, 1, 2, 0, 2, 3];
impl<'a> RenderData<'a> {
    pub fn new(
        device: &Device,
        resolution: PhysicalSize<u32>,
        frame: &Vec<Value>,
        images: &HashMap<u64, DynamicImage>,
        triangle_pipeline: &'a RenderPipeline,
        texture_pipeline: &'a RenderPipeline,
        texture_pipeline_bind_groups: &'a HashMap<u64, BindGroup>,
    ) -> Self {
        (|| {
            let mut texture_vertices_index = 0;
            let mut texture_vertices = vec![];

            let mut triangle_vertices = vec![];
            let mut triangle_indices = vec![];

            let mut render_order = vec![];
            for thing in frame {
                let visible = (|| Some(thing.get("visible")?.as_bool()?))().unwrap();
                if !visible {
                    continue;
                }

                fn pt(val: &serde_json::Value) -> Option<(f32, f32)> {
                    Some((
                        val.get("x")?.as_f64()? as f32,
                        val.get("y")?.as_f64()? as f32,
                    ))
                }

                fn rgb(val: &serde_json::Value) -> Option<(u8, u8, u8)> {
                    Some((
                        (val.get("r")?.as_i64()?.abs() % 256) as u8,
                        (val.get("g")?.as_i64()?.abs() % 256) as u8,
                        (val.get("b")?.as_i64()?.abs() % 256) as u8,
                    ))
                }

                let img = thing.get("img")?.as_u64()?;
                let (x, y) = pt(thing.get("pos")?)?;
                let scale = thing.get("scale")?.as_f64()? as f32;

                let image = &images[&img];
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
                    bind_group: Some(texture_pipeline_bind_groups.get(&img).unwrap()),
                    vertices_buffer_id: 0,
                    indices_buffer_id: 1,
                    vertices_range: (texture_vertices_index, texture_vertices_index + 4),
                    indices_range: (0, 6),
                };
                texture_vertices_index += 4;

                if thing.get("limbs").is_none() {
                    render_order.push(texture_settings);
                    continue;
                }

                if thing.get("limbsInFront")?.as_bool()? {
                    render_order.push(texture_settings);
                }

                let mut limbs = vec![];
                for limb in thing.get("limbs")?.as_array()? {
                    let mut l = vec![];
                    for joint in limb.get("points")?.as_array()? {
                        l.push(character_relative_to_canvas(
                            pt(joint)?,
                            scale,
                            (x, y),
                            resolution_aspect_ratio,
                        ));
                    }

                    limbs.push((
                        l,
                        rgb(limb.get("color")?)?,
                        limb.get("thickness")?.as_f64()? as f32 * scale,
                    ));
                }

                let mut i = 0;
                let mut v = 0;

                let triangle_vertices_index = triangle_vertices.len();
                let triangle_indices_index = triangle_indices.len();
                for (limb, (r, g, b), thickness) in limbs {
                    let color = Color::new(r, g, b);
                    fn pt((x, y): (f32, f32)) -> Point {
                        point(x, y)
                    }

                    let mut path_builder = Path::builder();
                    path_builder.begin(pt(limb[0]));
                    path_builder.cubic_bezier_to(
                        pt(mid(limb[0], limb[1])),
                        pt(mid(limb[1], limb[2])),
                        pt(limb[2]),
                    );
                    path_builder.end(false);
                    let path = path_builder.build();

                    let mut tesselator = StrokeTessellator::new();
                    let mut geometry: VertexBuffers<_, u16> = VertexBuffers::new();

                    let stroke_options = StrokeOptions::default()
                        .with_line_width(thickness)
                        .with_line_cap(LineCap::Round)
                        .with_tolerance(0.001);

                    tesselator
                        .tessellate_path(
                            &path,
                            &stroke_options,
                            &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| {
                                ColorVertex {
                                    position: vertex.position().to_array(),
                                    color: color.to_linear_rgb(),
                                }
                            }),
                        )
                        .expect("Error during tessellation");

                    i += geometry.indices.len();
                    triangle_indices.extend(geometry.indices.iter().map(|x| x + v as u16));
                    v += geometry.vertices.len();
                    triangle_vertices.append(&mut geometry.vertices);
                }

                if thing.get("limbs").is_some() {
                    render_order.push(RenderSettings {
                        pipeline: triangle_pipeline,
                        bind_group: None,
                        vertices_buffer_id: 2,
                        indices_buffer_id: 3,
                        vertices_range: (
                            triangle_vertices_index as u64,
                            triangle_vertices_index as u64 + v as u64,
                        ),
                        indices_range: (
                            triangle_indices_index as u32,
                            triangle_indices_index as u32 + i as u32,
                        ),
                    });
                }

                if !(thing.get("limbsInFront")?.as_bool()?) {
                    render_order.push(texture_settings);
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
