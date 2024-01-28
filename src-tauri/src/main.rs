// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use app_data::AppData;
use request_handlers::{add_frames, pause, play, stop, export, to_base64_png, update_media_resources};

use std::{sync::{mpsc, Arc, RwLock}, thread};

mod app_data;
mod renderer;
mod signals;
mod request_handlers;

/// There are 2 main parts of this app, the wgpu renderer and the tauri applicaton.
///
/// The tauri application is responsible for handling all the user interaction,
/// The renderer is responsible for rendering the resulting video
fn main() {
    video_rs::init().expect("failed to initialize video-rs");
    let data = AppData::new();
    let (update_media_resources_signal_tx, update_media_resources_signal_rx) = mpsc::channel();
    let (encode_video_signal_tx, encode_video_signal_rx) = mpsc::channel();
    let (display_signal_tx, display_signal_rx) = mpsc::channel();
    let data = Arc::new(RwLock::new(data));
    {
        let data = data.clone();
        thread::spawn(move || {
            tauri::Builder::default()
                .manage(data)
                .manage(update_media_resources_signal_tx)
                .manage(encode_video_signal_tx)
                .manage(display_signal_tx)
                .invoke_handler(tauri::generate_handler![
                    add_frames,
                    update_media_resources,
                    to_base64_png,
                    play,
                    pause,
                    stop,
                    export
                ])
                .any_thread()
                .run(tauri::generate_context!())
                .expect("error while running tauri application");
        });
    }
    renderer::run(data, update_media_resources_signal_rx, encode_video_signal_rx, display_signal_rx);
}
