use self::renderer::Renderer;
use crate::{app_data::AppData, signals::{UpdateMediaResourcesSignal, EncodeVideoSignal}};
use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    time::Instant, borrow::BorrowMut,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod pipelines;
mod renderer;
mod shader_structs;
mod texture;
mod render_data;

pub fn run(
    app_data: Arc<Mutex<AppData>>,
    update_media_resources_signal_rx: Receiver<UpdateMediaResourcesSignal>,
    encode_video_signal_rx: Receiver<EncodeVideoSignal>,
) {
    // wait for media resources to be updated before spawning the rendering window
    update_media_resources_signal_rx.recv().unwrap();

    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("booglanim")
        .build(&event_loop)
        .unwrap();
    let mut renderer = {
        let app_data = app_data.lock().unwrap();
        pollster::block_on(Renderer::new(window, &app_data.media_resources.images))
    };
    let mut last_frame_update = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == renderer.window().id() => {
                match event {
                    WindowEvent::Resized(physical_size) => {
                        renderer.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        // new_inner_size is &&mut so w have to dereference it twice
                        renderer.resize(**new_inner_size);
                    }
                    _ => {}
                }
            }
            Event::RedrawRequested(window_id) if window_id == renderer.window().id() => {
                let mut app_data = app_data.lock().unwrap();
                if let Ok(_) = update_media_resources_signal_rx.try_recv() {
                    renderer.refresh_texture_pipeline(&app_data.media_resources.images);
                    while let Ok(_) = update_media_resources_signal_rx.try_recv() {}
                }

                if app_data.frames.len() == 0 {
                    return;
                }

                if let Ok(signal) = encode_video_signal_rx.try_recv() {
                    pollster::block_on(renderer.encode(app_data.borrow_mut(), signal.app_handle, signal.path));
                    while let Ok(_) = encode_video_signal_rx.try_recv() {}
                }

                match pollster::block_on(renderer.render(
                    &app_data.frames[app_data.frame],
                    &app_data.media_resources.images,
                )) {
                    Ok(_) => {}

                    // Reconfigure the surface if it's lost or outdated
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        renderer.resize(renderer.size())
                    }

                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,

                    // idk
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                }
                if app_data.frame >= app_data.frames.len() - 1 {
                    app_data.frame = app_data.frames.len() - 1;
                    app_data.playing = false;
                }
                if app_data.playing
                    && (Instant::now() - last_frame_update).as_secs_f64()
                        > 1.0 / (app_data.fps as f64)
                {
                    app_data.frame += 1;
                    last_frame_update = Instant::now();
                }
            }
            Event::RedrawEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                renderer.window().request_redraw();
            }
            _ => {}
        }
    });
}
