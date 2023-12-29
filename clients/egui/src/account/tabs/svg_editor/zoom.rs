use std::collections::HashSet;

use eframe::egui;
use minidom::Element;

use super::Buffer;

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


        if ui.input(|r| r.key_down(egui::Key::PlusEquals)) {
            
        } else if ui.input(|r| r.key_down(egui::Key::Minus)) {
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
