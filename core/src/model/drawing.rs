
#[derive(Serialize, Deserialize, Debug)]
pub struct Drawing {
    pub dimens: Page,
    pub events: Vec<Event>
}

pub struct Page {
    pub transformation: Transformation
}

pub struct Transformation {
    pub translation: Point,
    pub scale: f32
}

pub struct Event {
    pub stroke: Option<Stroke>
}

pub struct Stroke {
    pub color: u32,
    pub points: Vec<f32>
}

pub struct Point {
    pub x: f32,
    pub y: f32
}

impl Drawing {
    fn new() -> Self {
        Drawing {
            dimens: Page {
                transformation: Transformation {
                    translation: Point {
                        x: 0.0,
                        y: 0.0
                    },
                    scale: 1.0
                }
            },
            events: vec![]
        }
    }
}
