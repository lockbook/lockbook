mod node;
mod response;
mod state;

use std::thread;

pub use self::node::TreeNode;

use eframe::egui;

use self::response::NodeResponse;
use self::state::*;

pub struct FileTree {
    pub root: TreeNode,
    pub state: TreeState,
    core: lb::Core,
}

impl FileTree {
    pub fn new(all_metas: Vec<lb::File>, core: &lb::Core) -> Self {
        let root = create_root_node(all_metas);

        let mut state = TreeState::default();
        state.expanded.insert(root.file.id);

        Self { root, state, core: core.clone() }
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
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let mut is_hovered = false;
        let mut r = egui::Frame::none().show(ui, |ui| {
            let result = self.root.show(ui, &mut self.state);
            is_hovered = result.response.hovered();
            result.inner
        });

        let empty_space_res = ui.interact(
            ui.available_rect_before_wrap(),
            egui::Id::from("tree-empty-space"),
            egui::Sense::click(),
        );

        empty_space_res.context_menu(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(4.0, 4.0);

            if ui.button("New Document").clicked() {
                r.inner.new_file = Some(true);
                ui.close_menu();
            }
            if ui.button("New Drawing").clicked() {
                r.inner.new_drawing = Some(true);
                ui.close_menu();
            }
            if ui.button("New Folder").clicked() {
                r.inner.new_folder_modal = Some(self.root.file.clone());
                ui.close_menu();
            }
        });

        if self.state.is_dragging() {
            if ui.input(|i| i.pointer.any_released()) {
                let maybe_pos = ui.ctx().pointer_interact_pos();
                self.state.dropped(maybe_pos);
            } else {
                self.draw_drag_info_by_cursor(ui);
            }
        } else if is_hovered && ui.input(|i| i.pointer.primary_down()) {
            // todo(steve): prep drag only if a file is clicked
            self.state.dnd.is_primary_down = true;
            if ui.input(|i| i.pointer.is_moving()) {
                self.state.dnd.has_moved = true;
            }
        }
        ui.expand_to_include_rect(ui.available_rect_before_wrap());

        while let Ok(update) = self.state.update_rx.try_recv() {
            match update {
                TreeUpdate::RevealFileDone((expanded_files, selected)) => {
                    self.state.request_scroll = true;

                    expanded_files.iter().for_each(|f| {
                        self.state.expanded.insert(*f);
                    });
                    self.state.selected.clear();
                    self.state.selected.insert(selected);
                }
                TreeUpdate::ExportFile((exported_file, dest)) => {
                    match self
                        .core
                        .export_file(exported_file.id, dest.clone(), true, None)
                    {
                        Ok(_) => {
                            r.inner.export_file = Some(Ok((exported_file, dest)));
                        }
                        Err(err) => r.inner.export_file = Some(Err(err)),
                    }
                }
            }
        }
        r.inner
    }

    fn draw_drag_info_by_cursor(&mut self, ui: &mut egui::Ui) {
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

    /// expand the parents of the file and select it
    // todo: remove this, duplicate of expand_to()
    pub fn reveal_file(&mut self, id: lb::Uuid, core: &lb::Core) {
        let core = core.clone();
        let update_tx = self.state.update_tx.clone();
        thread::spawn(move || {
            let mut curr = core.get_file_by_id(id).unwrap();
            let mut expanded = vec![];
            loop {
                let parent = core.get_file_by_id(curr.parent).unwrap();
                expanded.push(parent.id);
                if parent == curr {
                    break;
                }
                curr = parent;
            }

            update_tx
                .send(TreeUpdate::RevealFileDone((expanded, id)))
                .unwrap();
        });
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
