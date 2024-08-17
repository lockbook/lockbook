use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Drawing {
    pub scale: f32,
    pub translation_x: f32,
    pub translation_y: f32,
    pub strokes: Vec<Stroke>,
    pub theme: Option<HashMap<ColorAlias, ColorRGB>>,
}

impl Default for Drawing {
    fn default() -> Self {
        Drawing { scale: 1.0, translation_x: 0.0, translation_y: 0.0, strokes: vec![], theme: None }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Stroke {
    pub points_x: Vec<f32>,
    pub points_y: Vec<f32>,
    pub points_girth: Vec<f32>,
    pub color: ColorAlias,
    pub alpha: f32,
}

impl Stroke {
    pub fn new(color: ColorAlias) -> Self {
        Self {
            points_x: Vec::new(),
            points_y: Vec::new(),
            points_girth: Vec::new(),
            color,
            alpha: 1.0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.points_x.is_empty() && self.points_y.is_empty() && self.points_girth.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct ColorRGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
