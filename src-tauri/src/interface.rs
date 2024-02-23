/// This contains all the structs that are the interface between the UI
/// and the Renderer. The UI will send requests in the form of a vector of `FrameDescription`s
/// to make into video. Thus the UI's json requests need to conform to the format defined
/// by these structs.
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
pub struct Node {
    pub z: f32,
    pub transform : Transform,
    pub visible: bool,
    pub children: Vec<Container>,
}

#[derive(Deserialize)]
pub enum Container {
    Node(Node),
    Leaf(Object)
}

#[derive(Deserialize)]
pub enum Object {
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
    pub pos : Point,
    pub scale : Point,
    pub angle : f32,
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            pos: Point { x: 0.0, y: 0.0 },
            scale: Point { x: 1.0, y: 1.0 },
            angle: 0.0,
        }
    }

    pub fn to_transformation(&self) -> Transformation2D {
        let displacement = Transformation2D(
            [
                [1.0, 0.0, self.pos.x],
                [0.0, 1.0, self.pos.y],
                [0.0, 0.0, 1.0]
            ]
        );
        let scale = Transformation2D(
            [
                [self.scale.x, 0.0, 0.0],
                [0.0, self.scale.y, 0.0],
                [0.0, 0.0, 1.0]
            ]
        );
        let rotation = Transformation2D(
            [
                [self.angle.to_radians().cos(), -self.angle.to_radians().sin(), 0.0],
                [self.angle.to_radians().sin(), self.angle.to_radians().cos(), 0.0],
                [0.0, 0.0, 1.0],
            ]
        );
        return displacement.multiply(&scale.multiply(&rotation));
    }
}

pub struct Transformation2D(pub [[f32; 3]; 3]);

impl Transformation2D {
    pub fn multiply(&self, other: &Transformation2D) -> Transformation2D {
        let mut result = [[0.0; 3]; 3];

        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    result[i][j] += self.0[i][k] * other.0[k][j];
                }
            }
        }

        Transformation2D(result)
    }

    pub fn apply_to(&self, pt: [f32; 2]) -> [f32; 2] {
        let result: Vec<f32> = self.0.iter().map(|row| {
            row.iter().zip(&[pt[0], pt[1], 1.0]).map(|(a, b)| a * b).sum()
        }).collect();
        [result[0] / result[2], result[1] / result[2]]
    }
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
    pub subrect: Option<Rect>,
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
    pub things: Vec<Node>,
    pub settings: Settings,
}


#[derive(Deserialize)]
pub struct VideoDescription {
    pub frames : Vec<FrameDescription>,
    pub sounds : Vec<AudioDescription>,
    pub fps : usize
}
