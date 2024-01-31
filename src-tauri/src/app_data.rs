use image::DynamicImage;
use std::collections::HashMap;

use crate::frame::FrameDescription;

pub struct Audio;

#[derive(Default)]
pub struct MediaResources {
    pub images: HashMap<u64, DynamicImage>,
    pub sounds: HashMap<u64, Audio>,
}

pub struct AppData {
    pub playing: bool,
    pub frame: usize,
    pub frames: Vec<FrameDescription>,
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
