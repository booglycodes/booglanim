// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use request_handlers::{
    export, next_frame, pause, play, prev_frame, reverse, stop, to_base64_png,
    update_media_resources, update_video_description,
};

use std::{sync::mpsc, thread};

mod interface;
mod renderer;
mod request_handlers;
mod signals;

/// There are 2 main parts of this app, the wgpu renderer and the tauri applicaton.
///
/// The tauri application is responsible for handling all the user interaction,
/// The renderer is responsible for rendering the resulting video
fn main() {
    video_rs::init().expect("failed to initialize video-rs");
    let (signal_tx, signal_rx) = mpsc::channel();
    {
        thread::spawn(move || {
            tauri::Builder::default()
                .manage(signal_tx)
                .invoke_handler(tauri::generate_handler![
                    update_video_description,
                    update_media_resources,
                    to_base64_png,
                    export,
                    play,
                    pause,
                    stop,
                    next_frame,
                    prev_frame,
                    reverse
                ])
                .any_thread()
                .run(tauri::generate_context!())
                .expect("error while running tauri application");
        });
    }
    renderer::run(signal_rx);
}
