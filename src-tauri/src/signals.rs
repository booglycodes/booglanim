use std::collections::HashMap;

use image::DynamicImage;
use tauri::AppHandle;

use crate::interface::VideoDescription;

pub struct Audio;

#[derive(Default)]
pub struct MediaResources {
    pub images: HashMap<u32, DynamicImage>,
    pub sounds: HashMap<u32, Audio>,
}

pub enum Signal {
    ExportVideo(ExportVideo),
    SetPlayback(Playback),
    UpdateVideoDescription(VideoDescription),
    UpdateMediaResources(MediaResources)
}

pub struct ExportVideo {
    pub app_handle : AppHandle<>, 
    pub path : String
}

pub enum SetFrame {
    At(usize),
    Forward(usize),
    Back(usize)
}

pub struct Playback {
    pub playing : bool,
    pub reverse : bool,
    pub frame : Option<SetFrame>
}