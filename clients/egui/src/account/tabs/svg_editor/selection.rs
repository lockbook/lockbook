use bezier_rs::Bezier;
use eframe::egui;

use super::{node_by_id, pointer_interests_path, Buffer};

pub struct Selection {
    last_pos: Option<egui::Pos2>,
    selected_element: Option<String>,
    dragging_pos: Option<(egui::Pos2, egui::Pos2)>,
}

impl Selection {
    pub fn new() -> Self {
        Selection { last_pos: None, dragging_pos: None, selected_element: None }
    }

    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, working_rect: egui::Rect, buffer: &mut Buffer,
    ) {
        let pos = match ui.ctx().pointer_hover_pos() {
            Some(cp) => {
                if !working_rect.contains(cp) || !ui.is_enabled() {
                    return;
                }
                cp
            }
            None => return,
        };

        // todo: skip this if the cursor is inside the bb of the selected el
        let mut hovered_element = None;
        for (id, path) in buffer.paths.iter() {
            if pointer_interests_path(path, pos, self.last_pos, 10.0) {
                ui.output_mut(|r| r.cursor_icon = egui::CursorIcon::Grab);
                hovered_element = Some(id);
                break;
            }
        }

        ui.input_mut(|w| {
            if let Some(event) = w.events.last() {
                // println!("{:#?}", event);
                match event {
                    egui::Event::PointerButton { pos: _, button, pressed, modifiers: _ } => {
                        if !pressed {
                            self.dragging_pos = None;
                        } else {
                            self.dragging_pos = Some((pos, egui::Pos2::ZERO));
                        }

                        if *pressed && matches!(button, egui::PointerButton::Primary) {
                            if let Some(path) = hovered_element {
                                self.selected_element = Some(path.to_string());
                            }
                        }
                    }
                    egui::Event::PointerGone => {
                        self.dragging_pos = None;
                    }
                    egui::Event::PointerMoved(new_pos) => {
                        if let Some(dp) = &mut self.dragging_pos {
                            dp.1 = *new_pos;
                        }
                    }
                    _ => {}
                }
            }
        });

        if let Some(path_id) = &self.selected_element {
            let path = buffer.paths.get(path_id).unwrap();

            if let Some(dp) = self.dragging_pos {
                let delta_x = dp.1.x - dp.0.x;
                let delta_y = dp.1.y - dp.0.y;
                if let Some(node) = node_by_id(&mut buffer.current, path_id.to_string()) {
                    node.set_attr("transform", format!("matrix(1,0,0,1,{delta_x},{delta_y} )"))
                }
            }
        }
        self.last_pos = Some(pos);
    }
}
