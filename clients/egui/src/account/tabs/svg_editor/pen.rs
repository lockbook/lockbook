use eframe::egui;
use std::collections::VecDeque;

pub enum PenEvent {
    Draw(egui::Pos2, usize),
    End(egui::Pos2, usize),
}

/// Build a cubic bézier path with Catmull-Rom smoothing and Ramer–Douglas–Peucker compression
pub struct CubicBezBuilder {
    /// store the 4 past points
    prev_points_window: VecDeque<egui::Pos2>,
    points: Vec<egui::Pos2>,
    pub data: String,
}

impl CubicBezBuilder {
    pub fn new() -> Self {
        CubicBezBuilder {
            prev_points_window: VecDeque::from(vec![]),
            points: vec![],
            data: String::new(),
        }
    }

    fn catmull_to(&mut self, dest: egui::Pos2, is_last_point: bool) {
        let is_first_point = self.prev_points_window.is_empty();

        if is_first_point {
            self.data = format!("M {} {}", dest.x, dest.y);

            // repeat the first pos twice to avoid later index arithmetic
            self.prev_points_window.push_back(dest);
        };

        if is_last_point {
            if let Some(last) = self.prev_points_window.back() {
                self.prev_points_window.push_back(*last);
            }
        }

        self.prev_points_window.push_back(dest);

        if self.prev_points_window.len() < 4 {
            return;
        }

        let (p0, p1, p2, p3) = (
            self.prev_points_window[0],
            self.prev_points_window[1],
            self.prev_points_window[2],
            self.prev_points_window[3],
        );

        let cp1x = p1.x + (p2.x - p0.x) / 6.0; // * k, k is tension which is set to 1, 0 <= k <= 1
        let cp1y = p1.y + (p2.y - p0.y) / 6.0;

        let cp2x = p2.x - (p3.x - p1.x) / 6.0;
        let cp2y = p2.y - (p3.y - p1.y) / 6.0;

        self.data
            .push_str(format!("C {cp1x},{cp1y},{cp2x},{cp2y},{},{}", p2.x, p2.y).as_str());

        // shift the window foreword
        self.prev_points_window.pop_front();
    }

    pub fn cubic_to(&mut self, dest: egui::Pos2) {
        self.points.push(dest);
        self.catmull_to(dest, false);
    }

    pub fn finish(&mut self, pos: egui::Pos2) {
        self.points.push(pos);
        self.catmull_to(pos, false); // todo: get rid of the double call if possible
        self.catmull_to(pos, true);

        self.simplify(2.5);

        self.data = String::default();
        self.prev_points_window = VecDeque::default();

        self.points.clone().iter().enumerate().for_each(|(i, p)| {
            self.catmull_to(*p, false);
            if i == self.points.len() - 1 {
                self.catmull_to(*p, true);
            };
        });
    }

    pub fn clear(&mut self) {
        self.prev_points_window = VecDeque::default();
        self.data = String::default();
        self.points = vec![];
    }

    /// Ramer–Douglas–Peucker algorithm courtesy of @author: Michael-F-Bryan
    /// https://github.com/Michael-F-Bryan/arcs/blob/master/core/src/algorithms/line_simplification.rs
    fn simplify(&mut self, tolerance: f32) {
        if self.points.len() <= 2 {
            return;
        }

        let mut buffer = Vec::new();

        // push the first point
        buffer.push(self.points[0]);
        // then simplify every point in between the start and end
        self.simplify_points(&self.points[..], tolerance, &mut buffer);
        // and finally the last one
        buffer.push(*self.points.last().unwrap());

        self.points = buffer;
    }

    fn simplify_points(&self, points: &[egui::Pos2], tolerance: f32, buffer: &mut Vec<egui::Pos2>) {
        if points.len() < 2 {
            return;
        }
        let first = points.first().unwrap();
        let last = points.last().unwrap();
        let rest = &points[1..points.len() - 1];

        let line_segment = Line::new(*first, *last);

        if let Some((ix, distance)) =
            self.max_by_key(rest, |p| line_segment.perpendicular_distance_to(*p))
        {
            if distance > tolerance {
                // note: index is the index into `rest`, but we want it relative
                // to `point`
                let ix = ix + 1;

                self.simplify_points(&points[..=ix], tolerance, buffer);
                buffer.push(points[ix]);
                self.simplify_points(&points[ix..], tolerance, buffer);
            }
        }
    }

    fn max_by_key<T, F, K>(&self, items: &[T], mut key_func: F) -> Option<(usize, K)>
    where
        F: FnMut(&T) -> K,
        K: PartialOrd,
    {
        let mut best_so_far = None;

        for (i, item) in items.iter().enumerate() {
            let key = key_func(item);

            let is_better = match best_so_far {
                Some((_, ref best_key)) => key > *best_key,
                None => true,
            };

            if is_better {
                best_so_far = Some((i, key));
            }
        }
        best_so_far
    }
}

struct Line {
    start: egui::Pos2,
    end: egui::Pos2,
}

impl Line {
    fn new(start: egui::Pos2, end: egui::Pos2) -> Self {
        Line { start, end }
    }

    fn length(&self) -> f32 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;

        // Using the Pythagorean theorem to calculate the length
        (dx * dx + dy * dy).sqrt()
    }

    fn perpendicular_distance_to(&self, point: egui::Pos2) -> f32 {
        const SOME_SMALL_NUMBER: f32 = std::f32::EPSILON * 100.0;

        let side_a = self.start - point;
        let side_b = self.end - point;

        let area = (side_a.x * side_b.y - side_a.y * side_b.x) / 2.0;

        // area = base * height / 2
        let base_length = self.length();

        if base_length.abs() < SOME_SMALL_NUMBER {
            side_a.length()
        } else {
            area.abs() * 2.0 / base_length
        }
    }
}
