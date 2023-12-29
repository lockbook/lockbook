use std::collections::HashSet;

use eframe::egui;
use minidom::Element;

use super::{util::parse_transform, Buffer};

pub struct Zoom {}

const ZOOM_STEP: f32 = 1.5;
pub const G_CONTAINER_ID: &str = "lb:zoom_container";

impl Zoom {
    pub fn new() -> Self {
        Zoom {}
    }

    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, working_rect: egui::Rect, buffer: &mut Buffer,
    ) {
        let pos = match ui.ctx().pointer_hover_pos() {
            Some(cp) => {
                if ui.is_enabled() {
                    cp
                } else {
                    return;
                }
            }
            None => egui::Pos2::ZERO,
        };

        let dx = ui.input(|r| r.scroll_delta.x) as f64;
        let dy = ui.input(|r| r.scroll_delta.y) as f64;

        if dx.abs() > 0.0 || dy.abs() > 0.0 {
            let original_matrix =
                parse_transform(buffer.current.attr("transform").unwrap_or_default());

            let new_transform = if ui.input(|r| r.modifiers.ctrl) {
                let multiplier = if dy > 0.0 { 1.25 } else { 0.75 };
                let original_matrix: Vec<f32> = original_matrix
                    .iter()
                    .map(|x| (multiplier * x) as f32)
                    .collect();

                format!(
                    "matrix({},{},{},{},{},{} )",
                    original_matrix[0],
                    original_matrix[1],
                    original_matrix[2],
                    original_matrix[3],
                    original_matrix[4],
                    original_matrix[5]
                )
            } else {
                format!("matrix(1,0,0,1,{},{} )", dx + original_matrix[4], dy + original_matrix[5])
            };

            buffer.current.set_attr("transform", new_transform);
            buffer.needs_path_map_update = true;
        }
    }
}

pub fn verify_zoom_g(buffer: &mut Buffer) {
    if buffer.current.attr("id").unwrap_or_default() != G_CONTAINER_ID {
        let mut g = Element::builder("g", "").attr("id", G_CONTAINER_ID).build();
        let mut moved_ids = HashSet::new();
        buffer.current.children().for_each(|child| {
            g.append_child(child.clone());
            moved_ids.insert(child.attr("id").unwrap_or_default().to_string());
        });

        moved_ids.iter().for_each(|id| {
            buffer.current.remove_child(id);
        });

        buffer.current = g;
    }
}
