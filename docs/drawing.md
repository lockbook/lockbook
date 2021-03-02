# Drawing Data Format

Data definiton:
```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Drawing {
    pub scale: f32,
    pub translation_x: f32,
    pub translation_y: f32,
    pub strokes: Vec<Stroke>,
    pub theme: Option<HashMap<ColorAlias, ColorRGB>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Stroke {
    pub points_x: Vec<f32>,
    pub points_y: Vec<f32>,
    pub points_girth: Vec<f32>,
    pub color: ColorAlias,
    pub alpha: u8,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ColorAlias {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ColorRGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
```