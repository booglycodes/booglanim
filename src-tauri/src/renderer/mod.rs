use crate::{
    interface::VideoDescription,
    signals::{ExportVideo, SetFrame, Signal},
};

use self::{
    renderers::{Renderers, RenderingError},
    video::export_video,
};

use image::{ImageBuffer, Rgba};
use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    thread,
    time::Instant,
};
use tauri::Manager;
use winit::dpi::PhysicalSize;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod pipelines;
mod render_data;
mod renderers;
mod shader_structs;
mod texture;
mod video;

const RESOLUTION: (u32, u32) = (1920, 1080);

pub fn run(signal_rx: Receiver<Signal>) {
    let (width, height) = RESOLUTION;
    let buf = vec![0; width as usize * height as usize];
    let black = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, buf).unwrap();

    // wait for media resources to be updated before spawning the rendering window
    let media_resources;
    loop {
        if let Signal::UpdateMediaResources(new_media_resources) =
            signal_rx.recv().expect("sender closed!")
        {
            media_resources = new_media_resources;
            break;
        }
    }

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("booglanim")
        .build(&event_loop)
        .unwrap();

    let renderers = Arc::new(pollster::block_on(Renderers::new(
        window,
        &media_resources.images,
        PhysicalSize::new(1920, 1080),
    )));

    let mut last_frame_update = Instant::now();
    let mut latest_image = None;
    let media_resources = Arc::new(Mutex::new(media_resources));
    let video_description = Arc::new(Mutex::new(VideoDescription {
        frames: Vec::new(),
        sounds: Vec::new(),
        fps: 16,
    }));
    let mut reverse = false;
    let mut playing = true;
    let mut frame = 0;
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } => {
                let mut window_renderer = renderers.window_renderer.lock().unwrap();
                if window_id != window_renderer.window().id() {
                    return;
                }
                match event {
                    WindowEvent::Resized(physical_size) => {
                        window_renderer.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        // new_inner_size is &&mut so w have to dereference it twice
                        window_renderer.resize(**new_inner_size);
                    }
                    _ => {}
                }
            }
            Event::RedrawRequested(window_id) => {
                {
                    let window_renderer = renderers.window_renderer.lock().unwrap();
                    if window_id != window_renderer.window().id() {
                        return;
                    }
                }

                while let Ok(signal) = signal_rx.try_recv() {
                    match signal {
                        Signal::ExportVideo(ExportVideo { app_handle, path }) => {
                            let renderers = renderers.clone();
                            let media_resources = media_resources.clone();
                            let video_description = video_description.clone();
                            thread::spawn(move || {
                                if let (
                                    Ok(image_renderer),
                                    Ok(video_description),
                                    Ok(media_resources),
                                ) = (
                                    renderers.image_renderer.try_lock(),
                                    video_description.try_lock(),
                                    media_resources.try_lock(),
                                ) {
                                    pollster::block_on(export_video(
                                        &image_renderer,
                                        &video_description.frames,
                                        &media_resources.images,
                                        video_description.fps,
                                        |frame| {
                                            app_handle.emit_all("encoded-frame", frame).unwrap()
                                        },
                                        path,
                                    ));
                                }
                            });
                        }
                        Signal::SetPlayback(playback) => {
                            reverse = playback.reverse;
                            playing = playback.playing;
                            if let Some(playback_frame) = playback.frame {
                                if let Ok(video_description) = video_description.try_lock() {
                                    frame = match playback_frame {
                                        SetFrame::At(playback_frame) => playback_frame,
                                        SetFrame::Forward(next) => {
                                            (frame + next).min(video_description.frames.len())
                                        }
                                        SetFrame::Back(prev) => frame.saturating_sub(prev),
                                    };
                                }
                            }
                        }
                        Signal::UpdateVideoDescription(new_video_description) => {
                            if let Ok(mut video_description) = video_description.try_lock() {
                                *video_description = new_video_description;
                            }
                        }
                        Signal::UpdateMediaResources(new_media_resources) => {
                            if let (Ok(mut image_renderer), Ok(mut media_resources)) = (
                                renderers.image_renderer.try_lock(),
                                media_resources.try_lock(),
                            ) {
                                *media_resources = new_media_resources;
                                image_renderer.refresh_texture_pipeline(&media_resources.images);
                            }
                        }
                    }
                }

                let res = {
                    if let (Ok(video_description), Ok(media_resources)) =
                        (video_description.try_lock(), media_resources.try_lock())
                    {
                        let res = pollster::block_on(
                            renderers
                                .render(&video_description.frames[frame], &media_resources.images),
                        );
                        if frame >= video_description.frames.len() - 1 && !reverse {
                            frame = video_description.frames.len() - 1;
                            playing = false;
                        }
                        if frame == 0 && reverse {
                            playing = false;
                        }
                        if playing
                            && (Instant::now() - last_frame_update).as_secs_f64()
                                > 1.0 / (video_description.fps as f64)
                        {
                            last_frame_update = Instant::now();
                            if reverse {
                                frame -= 1
                            } else {
                                frame += 1
                            };
                        }
                        res
                    } else {
                        Err(RenderingError::RendererLockError)
                    }
                };

                let res = match res {
                    // rendered fine, no problems
                    Ok(img) => {
                        latest_image = Some(img);
                        Ok(())
                    }

                    Err(RenderingError::SurfaceError(e)) => Err(e),

                    // one or more of the renderers is busy
                    Err(RenderingError::RendererLockError) => {
                        // try to just render the window with the latest image, if it exists. Otherwise, just render a black screen.
                        match latest_image.as_ref() {
                            Some(latest_image) => renderers
                                .window_renderer
                                .try_lock()
                                .unwrap()
                                .render(latest_image),
                            None => renderers.window_renderer.try_lock().unwrap().render(&black),
                        }
                    }
                };

                match res {
                    Ok(_) => {}
                    // Reconfigure the surface if it's lost or outdated
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let mut window_renderer = renderers.window_renderer.lock().unwrap();
                        let size = window_renderer.size();
                        window_renderer.resize(size);
                    }

                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,

                    // idk
                    Err(wgpu::SurfaceError::Timeout) => {
                        println!("surface timeout")
                    }
                }
            }
            Event::RedrawEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually request it.
                renderers
                    .window_renderer
                    .lock()
                    .unwrap()
                    .window()
                    .request_redraw();
            }
            _ => {}
        }
    });
}
