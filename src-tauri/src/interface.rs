/// This contains all the structs that are the interface between the UI
/// and the Renderer. The UI will send requests in the form of a vector of `FrameDescription`s
/// to make into video. Thus the UI's json requests need to conform to the format defined
/// by these structures.
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_linear_rgb(&self) -> [f32; 3] {
        [self.r, self.g, self.b].map(|x| {
            let x = x as f32 / 255.0;
            if x > 0.04045 {
                ((x + 0.055) / 1.055).powf(2.4)
            } else {
                x / 12.92
            }
        })
    }

    pub fn to_wgpu_color(&self) -> wgpu::Color {
        let [r, g, b] = self.to_linear_rgb().map(|x| x.into());
        wgpu::Color { r, g, b, a: 1.0 }
    }
}

#[derive(Deserialize)]
pub enum Object {
    Transform(Transform),
    Bezier(Bezier),
    Img(Img),
    Text(Text),
}

#[derive(Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Deserialize)]
pub struct Transform {
    pub layer: i32,
    pub scale: f32,
    pub rot: f32,
    pub pos: Point,
    pub visible: bool,
    pub children: Vec<Object>,
}

#[derive(Deserialize)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Deserialize)]
pub struct Text {
    pub content: String,
    pub alignment: Alignment,
    pub width: f32,
}

#[derive(Deserialize)]
pub struct Img {
    pub id: u32,
    pub subrect: Rect,
}

#[derive(Deserialize)]
pub struct Bezier {
    pub thickness: f32,
    pub color: Color,
    pub points: [Point; 3],
}

#[derive(Deserialize)]
pub struct AudioDescription {
    pub id: u32,
    pub start: usize,
    #[serde(default)]
    pub end: Option<usize>,
    #[serde(default)]
    pub looping: bool,
}

#[derive(Deserialize)]
pub struct Camera {
    pub pos: Point,
    pub zoom: f32,
}

#[derive(Deserialize)]
pub struct Settings {
    #[serde(default = "bg")]
    pub bg: Color,
    #[serde(default = "camera")]
    pub camera: Camera,
}

fn bg() -> Color {
    Color::new(0, 0, 0)
}

fn camera() -> Camera {
    Camera {
        pos: Point { x: 0.0, y: 0.0 },
        zoom: 1.0,
    }
}

#[derive(Deserialize)]
pub struct FrameDescription {
    pub things: Vec<Transform>,
    pub settings: Settings,
}


#[derive(Deserialize)]
pub struct VideoDescription {
    pub frames : Vec<FrameDescription>,
    pub sounds : Vec<AudioDescription>,
    pub fps : usize
}
