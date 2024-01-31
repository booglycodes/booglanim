use serde::Deserialize;
use serde_json::Value;

use crate::color::Color;

#[derive(Deserialize)]
pub struct Settings {
    #[serde(default = "bg")]
    pub bg: Color,
}

fn bg() -> Color {
    Color::new(0, 0, 0)
}

#[derive(Deserialize)]
pub struct FrameDescription {
    pub things: Vec<Value>,
    pub settings: Settings,
}
