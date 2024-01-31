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
