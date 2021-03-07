use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Hash, R)]
pub struct ColorRGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

macro_rules! hashmap {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = ::std::collections::HashMap::new();
         $( map.insert($key, $val); )*
         map
    }}
}

pub const DEFAULT_THEME: HashMap<ColorAlias, ColorRGB> = hashmap![
    ColorAlias::White => ColorRGB{r: 0xFF, g: 0xFF, b: 0xFF},
    ColorAlias::Black => ColorRGB{r: 0x88, g: 0x88, b: 0x88},
    ColorAlias::Red => ColorRGB{r: 0xFF, g: 0x00, b: 0x00},
    ColorAlias::Green => ColorRGB{r: 0x00, g: 0xFF, b: 0x00},
    ColorAlias::Yellow => ColorRGB{r: 0xFF, g: 0xFF, b: 0x00},
    ColorAlias::Blue => ColorRGB{r: 0x00, g: 0x00, b: 0xFF},
    ColorAlias::Magenta => ColorRGB{r: 0xFF, g: 0x00, b: 0xFF},
    ColorAlias::Cyan => ColorRGB{r: 0x00, g: 0xFF, b: 0xFF}
];

