use anyhow::{anyhow, Context, Result};
use image::{codecs::png::PngEncoder, DynamicImage, ImageEncoder};

use std::{collections::HashMap, fmt::Debug, fs::File, io::Cursor, iter, sync::mpsc::Sender};

use crate::{
    interface::VideoDescription,
    signals::{ExportVideo, MediaResources, Playback, SetFrame, Signal},
};

fn errstr(e: impl Debug) -> String {
    format!("{:?}", e)
}

#[tauri::command]
pub fn update_video_description(
    signal_tx: tauri::State<Sender<Signal>>,
    description: VideoDescription,
) -> Result<(), String> {
    signal_tx
        .send(Signal::UpdateVideoDescription(description))
        .map_err(errstr)
}

#[tauri::command]
pub fn update_media_resources(
    signal_tx: tauri::State<Sender<Signal>>,
    res: Vec<(u32, String)>,
) -> Result<(), String> {
    (|| -> Result<_> {
        let supported_image_types = [".png", ".bmp", ".jpg", ".jpeg", ".webp"];
        let path_to_resource = |path: String| -> Result<DynamicImage> {
            let pathbuf = dirs::home_dir().unwrap().join(&path);

            if path.ends_with(".json") {
                let json: HashMap<String, serde_json::Value> = serde_json::from_reader(
                    File::open(pathbuf).with_context(|| format!("can't open {}", path))?,
                )?;
                let img_data = json
                    .get("img")
                    .ok_or(anyhow!("missing image resource"))?
                    .as_str()
                    .ok_or(anyhow!("image resource isn't a string"))?;
                let base64 = base64_simd::STANDARD;
                let img_data_decoded = base64.decode_to_vec(img_data)?;
                Ok(image::load(
                    Cursor::new(img_data_decoded),
                    image::ImageFormat::Png,
                )?)
            } else if supported_image_types
                .into_iter()
                .any(|ext| path.ends_with(ext))
            {
                Ok(image::open(pathbuf).with_context(|| format!("can't open {}", path))?)
            } else {
                Err(anyhow!(
                    "File extension {:?} not recognized. Should be one of {:?}",
                    pathbuf.extension(),
                    supported_image_types
                        .iter()
                        .cloned()
                        .chain(iter::once(".json"))
                        .collect::<Vec<&str>>()
                ))
            }
        };

        let images: Result<HashMap<_, _>> = res
            .into_iter()
            .map(|(id, path)| path_to_resource(path).map(|i| (id, i)))
            .collect();

        signal_tx.send(Signal::UpdateMediaResources(MediaResources {
            images: images?,
            sounds: HashMap::new(),
        }))?;

        signal_tx.send(Signal::SetPlayback(Playback {
            playing: false,
            reverse: false,
            frame: Some(SetFrame::At(0)),
        }))?;
        
        Ok(())
    })()
    .map_err(errstr)
}

fn playback(
    signal_tx: tauri::State<Sender<Signal>>,
    playing: bool,
    reverse: bool,
    frame: Option<SetFrame>,
) -> Result<(), String> {
    signal_tx
        .send(Signal::SetPlayback(Playback {
            playing,
            reverse,
            frame,
        }))
        .map_err(errstr)
}

#[tauri::command]
pub fn play(signal_tx: tauri::State<Sender<Signal>>) -> Result<(), String> {
    playback(signal_tx, true, false, None)
}

#[tauri::command]
pub fn pause(signal_tx: tauri::State<Sender<Signal>>) -> Result<(), String> {
    playback(signal_tx, false, false, None)
}

#[tauri::command]
pub fn stop(signal_tx: tauri::State<Sender<Signal>>) -> Result<(), String> {
    playback(signal_tx, false, false, Some(SetFrame::At(0)))
}

#[tauri::command]
pub fn next_frame(signal_tx: tauri::State<Sender<Signal>>) -> Result<(), String> {
    playback(signal_tx, false, false, Some(SetFrame::Forward(1)))
}

#[tauri::command]
pub fn prev_frame(signal_tx: tauri::State<Sender<Signal>>) -> Result<(), String> {
    playback(signal_tx, false, false, Some(SetFrame::Back(1)))
}

#[tauri::command]
pub fn reverse(signal_tx: tauri::State<Sender<Signal>>) -> Result<(), String> {
    playback(signal_tx, true, true, None)
}

#[tauri::command]
pub fn export(
    app_handle: tauri::AppHandle,
    signal_tx: tauri::State<Sender<Signal>>,
    path: String,
) -> Result<(), String> {
    signal_tx
        .send(Signal::ExportVideo(ExportVideo { app_handle, path }))
        .map_err(errstr)
}

#[tauri::command]
pub fn to_base64_png(path: String) -> String {
    let image = image::open(path).unwrap();
    let mut png_data = Vec::new();
    let encoder = PngEncoder::new(&mut png_data);
    let (width, height, color_type) = (image.width(), image.height(), image.color());
    encoder
        .write_image(image.as_bytes(), width, height, color_type)
        .unwrap();
    let base64 = base64_simd::STANDARD;
    base64.encode_to_string(png_data)
}
