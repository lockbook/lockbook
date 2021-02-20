use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Drawing {
    pub current_view: CurrentView,
    pub events: Vec<Event>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CurrentView {
    pub transformation: Transformation,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Transformation {
    pub translation: Point,
    pub scale: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Event {
    pub stroke: Option<Stroke>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Stroke {
    pub color: i32,
    pub points: Vec<f32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
