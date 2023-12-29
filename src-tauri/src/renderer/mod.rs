use self::renderer::Renderer;
use crate::{
    app_data::AppData,
    signals::{EncodeVideoSignal, UpdateMediaResourcesSignal},
};
use std::{
    borrow::BorrowMut,
    sync::{mpsc::Receiver, Arc, Mutex},
    thread,
    time::Instant,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod pipelines;
mod render_data;
mod renderer;
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
    let renderer = {
        let app_data = app_data.lock().unwrap();
        Arc::new(Mutex::new(pollster::block_on(Renderer::new(
            &window,
            &app_data.media_resources.images,
        ))))
    };
    let mut last_frame_update = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } => {
                if window_id != window.id() {
                    return;
                }
                let renderer = renderer.try_lock();
                if let Ok(mut renderer) = renderer {
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
            }
            Event::RedrawRequested(window_id) => {
                {
                    let renderer = renderer.try_lock();
                    if let Ok(mut renderer) = renderer {
                        if window_id != window.id() {
                            return;
                        }
                        // if we get a signal that media resources has updated, reconstruct the texture pipeline
                        // so that the correct images are displayed.
                        let app_data = app_data.lock().unwrap();
                        if let Ok(_) = update_media_resources_signal_rx.try_recv() {
                            renderer.refresh_texture_pipeline(&app_data.media_resources.images);
                            while let Ok(_) = update_media_resources_signal_rx.try_recv() {}
                        }
                    } else {
                        // if we can't acquire the renderer, it's because we're encoding video right now.
                        // just return so that the event loop keeps running and the operating system doesn't think
                        // we're stuck.
                        return; 
                    }
                }

                // if we get a signal to encode video, encode in a new thread.
                // this way we don't block and hang the event loop.
                if let Ok(signal) = encode_video_signal_rx.try_recv() {
                    let renderer = renderer.clone();
                    let app_data = app_data.clone();
                    thread::spawn(move || {
                        let mut renderer = renderer.lock().unwrap();
                        let mut app_data = app_data.lock().unwrap();
                        pollster::block_on(renderer.encode(
                            app_data.borrow_mut(),
                            signal.app_handle,
                            signal.path,
                        ));
                    });
                    while let Ok(_) = encode_video_signal_rx.try_recv() {}
                    // renderer will be locked until video is encoded, so we return here to avoid getting stuck
                    // when the code tries to render a frame below (it won't be able to until the above thread has completed).
                    return; 
                }

                let mut renderer = renderer.lock().unwrap();
                let mut app_data = app_data.lock().unwrap();
                if app_data.frames.len() == 0 {
                    return;
                }
                match pollster::block_on(renderer.render(
                    &app_data.frames[app_data.frame],
                    &app_data.media_resources.images,
                )) {
                    // rendered fine, no problems
                    Ok(_) => {}

                    // Reconfigure the surface if it's lost or outdated
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        let size = renderer.size();
                        renderer.resize(size);
                    }

                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,

                    // idk
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                }

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
                window.request_redraw();
            }
            _ => {}
        }
    });
}
