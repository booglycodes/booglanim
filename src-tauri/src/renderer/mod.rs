use self::renderers::{Renderers, RenderingError};
use crate::{
    app_data::AppData,
    signals::{EncodeVideoSignal, UpdateMediaResourcesSignal},
};
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

    let renderers = {
        let app_data = app_data.lock().unwrap();
        Arc::new(pollster::block_on(Renderers::new(
            window,
            &app_data.media_resources.images,
            PhysicalSize::new(1920, 1080),
        )))
    };

    let mut last_frame_update = Instant::now();
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
                    if let Ok(_) = update_media_resources_signal_rx.try_recv() {
                        if let Ok(mut image_renderer) = renderers.image_renderer.try_lock() {
                            let app_data = app_data.lock().unwrap();
                            image_renderer
                                .refresh_texture_pipeline(&app_data.media_resources.images);
                        }
                        while let Ok(_) = update_media_resources_signal_rx.try_recv() {}
                    }
                }

                // if we get a signal to encode video, encode in a new thread.
                // this way we don't block and hang the event loop.
                if let Ok(signal) = encode_video_signal_rx.try_recv() {
                    let renderer = renderers.clone();
                    let app_data = app_data.clone();
                    thread::spawn(move || {
                        let image_renderer = renderer.image_renderer.lock().unwrap();
                        let app_data = app_data.lock().unwrap();
                        pollster::block_on(image_renderer.encode(
                            &app_data.frames,
                            &app_data.media_resources.images,
                            app_data.fps,
                            |frame| signal.app_handle.emit_all("encoded-frame", frame).unwrap(),
                            signal.path,
                        ));
                    });
                    while let Ok(_) = encode_video_signal_rx.try_recv() {}
                    // renderer will be locked until video is encoded, so we return here to avoid getting stuck
                    // when the code tries to render a frame below (it won't be able to until the above thread has completed).
                    return;
                }

                let mut app_data = app_data.lock().unwrap();
                if app_data.frames.len() == 0 {
                    return;
                }

                match pollster::block_on(renderers.render(
                    &app_data.frames[app_data.frame],
                    &app_data.media_resources.images,
                )) {
                    // rendered fine, no problems
                    Ok(_) => {}

                    // Reconfigure the surface if it's lost or outdated
                    Err(RenderingError::SurfaceError(
                        wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated,
                    )) => {
                        let mut window_renderer = renderers.window_renderer.lock().unwrap();
                        let size = window_renderer.size();
                        window_renderer.resize(size);
                    }

                    // The system is out of memory, we should probably quit
                    Err(RenderingError::SurfaceError(wgpu::SurfaceError::OutOfMemory)) => {
                        *control_flow = ControlFlow::Exit
                    }

                    // idk
                    Err(RenderingError::SurfaceError(wgpu::SurfaceError::Timeout)) => {
                        log::warn!("Surface timeout")
                    }

                    // one or more of the renderers is busy
                    Err(RenderingError::RendererLockError) => return,
                };

                // advance frames, if there are frames remaining, and we're playing, and it's been long
                // enough since the last frame update.
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
