mod node;
mod response;
mod state;

pub use self::node::TreeNode;

use eframe::egui;

use self::response::NodeResponse;
use self::state::*;

pub struct FileTree {
    pub root: TreeNode,
    state: TreeState,
}

impl FileTree {
    pub fn new(all_metas: Vec<lb::File>) -> Self {
        let root = create_root_node(all_metas);

        let mut state = TreeState::default();
        state.expanded.insert(root.file.id);

        Self { root, state }
    }

    pub fn expand_to(&mut self, id: lb::Uuid) {
        if let Some(node) = self.root.find(id) {
            // Select only the target file.
            self.state.selected.clear();
            self.state.selected.insert(id);

            // Expand all target file parents.
            let mut id = node.file.parent;
            while let Some(node) = self.root.find(id) {
                self.state.expanded.insert(id);
                if node.file.id == self.root.file.id {
                    break;
                }
                id = node.file.parent;
            }
        } else {
            eprintln!("couldn't find node with id {}", id);
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> NodeResponse {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

            let r = egui::Frame::none().show(ui, |ui| self.root.show(ui, &mut self.state).inner);

            if self.state.is_dragging() {
                if ui.input(|i| i.pointer.any_released()) {
                    let maybe_pos = ui.ctx().pointer_interact_pos();
                    self.state.dropped(maybe_pos);
                } else {
                    self.draw_drag_info_by_cursor(ui);
                }
            } else if r.response.dragged() && ui.input(|i| i.pointer.primary_down()) {
                // todo(steve): prep drag only if a file is clicked
                self.state.dnd.is_primary_down = true;
                if ui.input(|i| i.pointer.is_moving()) {
                    self.state.dnd.has_moved = true;
                }
            }

            r.inner
        })
        .inner
    }

    fn draw_drag_info_by_cursor(&self, ui: &mut egui::Ui) {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);

        // Paint a caption under the cursor in a layer above.
        let layer_id = egui::LayerId::new(egui::Order::Tooltip, self.state.id);

        let hover_pos = ui.input(|i| i.pointer.hover_pos().unwrap());
        let mut end = hover_pos;
        end.x += 70.0;
        end.y += 50.0;

        let response = ui
            .allocate_ui_at_rect(egui::Rect::from_two_pos(hover_pos, end), |ui| {
                ui.with_layer_id(layer_id, |ui| {
                    egui::Frame::none()
                        .rounding(3.0)
                        .inner_margin(1.0)
                        .fill(ui.visuals().widgets.active.fg_stroke.color)
                        .show(ui, |ui| {
                            egui::Frame::none()
                                .rounding(3.0)
                                .inner_margin(egui::style::Margin::symmetric(12.0, 7.0))
                                .fill(ui.visuals().faint_bg_color)
                                .show(ui, |ui| {
                                    ui.label(self.state.drag_caption());
                                });
                        });
                })
            })
            .response;

        if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
            // todo: make sure dragging doesn't expand scroll area to infinity and beyond. respect the initial max width and height;

            if pointer_pos.y < 30.0 {
                ui.scroll_with_delta(egui::vec2(0., 30.0));
            }
            if pointer_pos.y < 100.0 {
                ui.scroll_with_delta(egui::vec2(0., 10.0));
            }
            ui.scroll_to_rect(response.rect, None);
        }
    }

    pub fn remove(&mut self, f: &lb::File) {
        if let Some(node) = self.root.find_mut(f.parent) {
            if let Some(mut removed) = node.remove(f.id) {
                clear_children(&mut self.state, &mut removed);
            }
        }
    }

    pub fn get_selected_files(&self) -> Vec<lb::File> {
        self.state
            .selected
            .iter()
            .map(|id| self.root.find(*id).unwrap().file.clone())
            .collect()
    }
}

pub fn create_root_node(all_metas: Vec<lb::File>) -> TreeNode {
    let mut all_metas = all_metas;

    let root_meta = match all_metas.iter().position(|fm| fm.parent == fm.id) {
        Some(i) => all_metas.swap_remove(i),
        None => panic!("unable to find root in metadata list"),
    };

    let mut root = TreeNode::from((root_meta, 0));
    root.populate_from(&all_metas);
    root
}

fn clear_children(state: &mut TreeState, node: &mut TreeNode) {
    state.selected.remove(&node.file.id);
    for child in &mut node.children {
        clear_children(state, child);
    }
}
