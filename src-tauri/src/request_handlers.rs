use crate::{
    app_data::{AppData, MediaResources},
    signals::{DisplaySignal, EncodeVideoSignal, UpdateMediaResourcesSignal},
};
use anyhow::{anyhow, Context, Result};
use image::{codecs::png::PngEncoder, DynamicImage, ImageEncoder};
use serde_json::Value;
use std::{
    collections::HashMap,
    fmt::Debug,
    fs::File,
    io::Cursor,
    iter,
    sync::{mpsc::Sender, Arc, RwLock},
};

fn errstr(e: impl Debug) -> String {
    format!("{:?}", e)
}

#[tauri::command]
pub fn add_frames(
    data: tauri::State<Arc<RwLock<AppData>>>,
    mut frames: Vec<Vec<Value>>,
) -> Result<(), String> {
    let mut data = data.write().map_err(errstr)?;
    data.frames.append(&mut frames);
    Ok(())
}

#[tauri::command]
pub fn update_media_resources(
    app_data: tauri::State<Arc<RwLock<AppData>>>,
    update_media_resources_signal_tx: tauri::State<Sender<UpdateMediaResourcesSignal>>,
    res: Vec<(u64, String)>,
    fps: usize,
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

        let res: Result<HashMap<_, _>> = res
            .into_iter()
            .map(|(id, path)| path_to_resource(path).map(|i| (id, i)))
            .collect();

        let mut app_data = app_data.write().unwrap();
        app_data.playing = false;
        app_data.frame = 0;
        app_data.frames = vec![];
        app_data.media_resources = MediaResources {
            images: res?,
            sounds: HashMap::new(),
        };
        app_data.fps = fps;
        update_media_resources_signal_tx
            .send(UpdateMediaResourcesSignal)
            .map_err(|e| anyhow!(errstr(e)))
    })()
    .map_err(errstr)
}

#[tauri::command]
pub fn play(display_signal_tx: tauri::State<Sender<DisplaySignal>>) -> Result<(), String> {
    display_signal_tx
        .send(DisplaySignal {
            playing: true,
            frame: None,
        })
        .map_err(errstr)
}

#[tauri::command]
pub fn pause(display_signal_tx: tauri::State<Sender<DisplaySignal>>) -> Result<(), String> {
    display_signal_tx
        .send(DisplaySignal {
            playing: false,
            frame: None,
        })
        .map_err(errstr)
}

#[tauri::command]
pub fn stop(display_signal_tx: tauri::State<Sender<DisplaySignal>>) -> Result<(), String> {
    display_signal_tx
        .send(DisplaySignal {
            playing: false,
            frame: Some(0),
        })
        .map_err(errstr)
}

#[tauri::command]
pub fn export(
    app_handle: tauri::AppHandle,
    encode_video_signal_tx: tauri::State<Sender<EncodeVideoSignal>>,
    path: String,
) -> Result<(), String> {
    encode_video_signal_tx
        .send(EncodeVideoSignal { app_handle, path })
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
