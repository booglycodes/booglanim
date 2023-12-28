use image::DynamicImage;
use serde_json::Value;
use std::collections::HashMap;


pub struct Audio;

#[derive(Default)]
pub struct MediaResources {
    pub images: HashMap<u64, DynamicImage>,
    pub sounds: HashMap<u64, Audio>,
}

pub struct AppData {
    pub playing: bool,
    pub frame: usize,
    pub frames: Vec<Vec<Value>>,
    pub media_resources: MediaResources,
    pub fps: usize,
}

impl AppData {
    pub fn new() -> Self {
        Self {
            playing: false,
            frame: 0,
            frames: vec![],
            media_resources: Default::default(),
            fps: 0,
        }
    }
}
