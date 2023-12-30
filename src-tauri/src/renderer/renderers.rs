use std::collections::HashMap;
use std::iter;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use super::pipelines::{screen_pipeline, texture_pipeline, triangle_pipeline};
use super::render_data::RenderData;
use super::shader_structs::TextureVertex;
use super::texture::Texture;
use image::{DynamicImage, ImageBuffer, Rgba};
use serde_json::Value;
use video_rs::{Encoder, EncoderSettings, Locator, Time};
use wgpu::{BindGroup, BindGroupLayout, RenderPipeline, TextureFormat, TextureView};

use ndarray::{Array3, Dim};

use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub struct Renderers {
    pub image_renderer: Mutex<ImageRenderer>,
    pub window_renderer: Mutex<WindowRenderer>,
    pub latest_image: Option<ImageBuffer<image::Rgba<u8>, Vec<u8>>>,
}

pub enum RenderingError {
    SurfaceError(wgpu::SurfaceError),
    RendererLockError,
}

impl Renderers {
    pub async fn new(
        window: Window,
        images: &HashMap<u64, DynamicImage>,
        size: PhysicalSize<u32>,
    ) -> Self {
        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let window_renderer = WindowRenderer::new(&instance, window).await;
        let image_renderer =
            ImageRenderer::new(&instance, window_renderer.config.format, images, size).await;

        Self {
            window_renderer: Mutex::new(window_renderer),
            image_renderer: Mutex::new(image_renderer),
            latest_image: None,
        }
    }

    pub async fn render(
        &self,
        frame: &Vec<Value>,
        images: &HashMap<u64, DynamicImage>,
    ) -> Result<ImageBuffer<image::Rgba<u8>, Vec<u8>>, RenderingError> {
        fn lock_renderer<'a, T>(
            renderer: &'a Mutex<T>,
        ) -> Result<MutexGuard<'a, T>, RenderingError> {
            renderer
                .try_lock()
                .map_err(|_| RenderingError::RendererLockError)
        }
        let image_renderer = lock_renderer(&self.image_renderer)?;
        let window_renderer = lock_renderer(&self.window_renderer)?;
        let img = image_renderer.render(frame, images).await;
        window_renderer
            .render(&img)
            .map_err(|x| RenderingError::SurfaceError(x))?;
        Ok(img)
    }
}

const RECT: &[u16; 6] = &[0, 1, 2, 0, 2, 3];

pub struct WindowRenderer {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    screen_bind_group_layout: BindGroupLayout,
    screen_pipeline: RenderPipeline,
    window: Window,
}

impl WindowRenderer {
    pub async fn new(instance: &wgpu::Instance, window: Window) -> Self {
        let size = window.inner_size();

        // # Safety
        // The surface needs to live as long as the window that created it.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let (screen_pipeline, screen_bind_group_layout) = screen_pipeline(&device, surface_format);

        Self {
            surface,
            config,
            screen_pipeline,
            screen_bind_group_layout,
            device,
            queue,
            size,
            window,
        }
    }

    pub fn render(
        &self,
        buffer: &ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view: TextureView = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let tex = Texture::from_image_buffer(&self.device, &self.queue, buffer, None);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.screen_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&tex.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&tex.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let window_ratio = self.size.width as f32 / self.size.height as f32;
        let aspect_ratio = buffer.width() as f32 / buffer.height() as f32;

        let screen_verts = if window_ratio < aspect_ratio {
            vec![
                TextureVertex {
                    position: [-1.0, -window_ratio / aspect_ratio],
                    tex_coords: [0.0, 1.0],
                },
                TextureVertex {
                    position: [1.0, -window_ratio / aspect_ratio],
                    tex_coords: [1.0, 1.0],
                },
                TextureVertex {
                    position: [1.0, window_ratio / aspect_ratio],
                    tex_coords: [1.0, 0.0],
                },
                TextureVertex {
                    position: [-1.0, window_ratio / aspect_ratio],
                    tex_coords: [0.0, 0.0],
                },
            ]
        } else {
            vec![
                TextureVertex {
                    position: [-aspect_ratio / window_ratio, -1.0],
                    tex_coords: [0.0, 1.0],
                },
                TextureVertex {
                    position: [aspect_ratio / window_ratio, -1.0],
                    tex_coords: [1.0, 1.0],
                },
                TextureVertex {
                    position: [aspect_ratio / window_ratio, 1.0],
                    tex_coords: [1.0, 0.0],
                },
                TextureVertex {
                    position: [-aspect_ratio / window_ratio, 1.0],
                    tex_coords: [0.0, 0.0],
                },
            ]
        };

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(screen_verts.as_slice()),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(RECT),
                usage: wgpu::BufferUsages::INDEX,
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.005,
                            g: 0.005,
                            b: 0.005,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            render_pass.set_pipeline(&self.screen_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw_indexed(0..RECT.len() as u32, 0, 0..1);
        }
        self.queue.submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.size
    }

    pub fn window(&self) -> &Window {
        &self.window
    }
}

pub struct ImageRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    texture_pipeline: RenderPipeline,
    texture_pipeline_bind_groups: HashMap<u64, BindGroup>,
    triangle_pipeline: RenderPipeline,
    texture_view: TextureView,
    texture: wgpu::Texture,
    format: TextureFormat,
}

impl ImageRenderer {
    pub async fn new(
        instance: &wgpu::Instance,
        format: TextureFormat,
        images: &HashMap<u64, DynamicImage>,
        size: PhysicalSize<u32>,
    ) -> Self {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&Default::default(), None)
            .await
            .unwrap();

        let texture_desc = wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            label: None,
            view_formats: &[format],
        };
        let texture = device.create_texture(&texture_desc);
        let texture_view = texture.create_view(&Default::default());

        let (texture_pipeline, texture_pipeline_bind_groups) =
            texture_pipeline(&device, &queue, format, &images);
        let triangle_pipeline = triangle_pipeline(&device, format);
        Self {
            size,
            texture,
            texture_view,
            device,
            queue,
            texture_pipeline,
            triangle_pipeline,
            texture_pipeline_bind_groups,
            format,
        }
    }

    pub async fn render(
        &self,
        frame: &Vec<Value>,
        images: &HashMap<u64, DynamicImage>,
    ) -> ImageBuffer<image::Rgba<u8>, Vec<u8>> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        let render_data = RenderData::new(
            &self.device,
            self.size,
            frame,
            images,
            &self.triangle_pipeline,
            &self.texture_pipeline,
            &self.texture_pipeline_bind_groups,
        );

        let output_buffer_size = (std::mem::size_of::<u32>() as u32
            * self.size.width
            * self.size.height) as wgpu::BufferAddress;
        let output_buffer_desc = wgpu::BufferDescriptor {
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST
            // this tells wpgu that we want to read this buffer from the cpu
            | wgpu::BufferUsages::MAP_READ,
            label: None,
            mapped_at_creation: false,
        };
        let output_buffer = self.device.create_buffer(&output_buffer_desc);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            for settings in &render_data.render_order {
                render_pass.set_pipeline(settings.pipeline);
                if let Some(bind_group) = settings.bind_group {
                    render_pass.set_bind_group(0, bind_group, &[]);
                }
                render_pass.set_index_buffer(
                    render_data.buffers[settings.indices_buffer_id].slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                render_pass.set_vertex_buffer(
                    0,
                    render_data.buffers[settings.vertices_buffer_id]
                        .slice(settings.vertices_range.0..settings.vertices_range.1),
                );
                render_pass.draw_indexed(
                    settings.indices_range.0..settings.indices_range.1,
                    0,
                    0..1,
                );
            }
        }

        let texture_size = wgpu::Extent3d {
            width: self.size.width,
            height: self.size.height,
            depth_or_array_layers: 1,
        };

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(std::mem::size_of::<u32>() as u32 * self.size.width),
                    rows_per_image: Some(self.size.height),
                },
            },
            texture_size,
        );

        self.queue.submit(iter::once(encoder.finish()));

        let buffer = {
            let buffer_slice = output_buffer.slice(..);

            // NOTE: We have to create the mapping THEN device.poll() before await
            // the future. Otherwise the application will freeze.
            let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });
            self.device.poll(wgpu::Maintain::Wait);
            rx.receive().await.unwrap().unwrap();

            let data = buffer_slice.get_mapped_range();

            let mut buf = vec![];
            data.clone_into(&mut buf);

            let buffer =
                ImageBuffer::<Rgba<u8>, _>::from_raw(self.size.width, self.size.height, buf)
                    .unwrap();

            buffer
        };
        output_buffer.unmap();
        buffer
    }

    pub async fn encode(
        &self,
        frames: &Vec<Vec<Value>>,
        images: &HashMap<u64, DynamicImage>,
        fps: usize,
        mut on_frame_complete: impl FnMut(usize) -> (),
        path: String,
    ) {
        let duration = Time::from_nth_of_a_second(fps);
        let mut position = Time::zero();

        let destination: Locator = PathBuf::from(path).into();
        let settings = EncoderSettings::for_h264_yuv420p(
            self.size.width as usize,
            self.size.height as usize,
            true,
        );
        let mut encoder = Encoder::new(&destination, settings).unwrap();
        for (frame_index, frame) in frames.iter().enumerate().into_iter() {
            let frame = self.render(frame, images).await;
            position = position.aligned_with(&duration).add();
            let mut buf = frame.as_raw().clone();
            let mut i = 0;
            buf.retain(|_| {
                i += 1;
                i % 4 != 0
            });
            let shape = Dim((self.size.height as usize, self.size.width as usize, 3));
            let frame = Array3::from_shape_vec(shape, buf).unwrap();
            encoder.encode(&frame, &position).unwrap();
            on_frame_complete(frame_index);
        }
        encoder.finish().unwrap();
    }

    pub fn refresh_texture_pipeline(&mut self, images: &HashMap<u64, DynamicImage>) {
        (self.texture_pipeline, self.texture_pipeline_bind_groups) =
            texture_pipeline(&self.device, &self.queue, self.format, images);
    }
}
