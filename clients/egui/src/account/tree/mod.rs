mod node;
mod response;
mod state;

pub use self::node::TreeNode;

use eframe::egui;
use lb::blocking::Lb;
use lb::model::file::File;
use lb::Uuid;

use self::response::NodeResponse;
use self::state::*;

pub struct FileTree {
    pub root: TreeNode,
    pub state: TreeState,
    core: Lb,
}

impl FileTree {
    pub fn new(all_metas: Vec<File>, core: &Lb) -> Self {
        let root = create_root_node(all_metas);

        let mut state = TreeState::default();
        state.expanded.insert(root.file.id);

        Self { root, state, core: core.clone() }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> NodeResponse {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let mut r = egui::Frame::none().show(ui, |ui| {
            let result = self.root.show(ui, &mut self.state);
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

        let dragging_rect = self.state.is_dragging_rect(r.response.rect);
        let released = ui.input(|i| self.state.update_dnd(i));
        if dragging_rect {
            if released {
                let maybe_pos = ui.ctx().pointer_interact_pos();
                self.state.dropped(maybe_pos);
            } else {
                self.draw_drag_info_by_cursor(ui);
            }
        }
        ui.expand_to_include_rect(ui.available_rect_before_wrap());

        while let Ok(update) = self.state.update_rx.try_recv() {
            match update {
                TreeUpdate::ExportFile((exported_file, dest)) => {
                    match self.core.export_file(
                        exported_file.id,
                        dest.clone(),
                        true,
                        &Some(Box::new(|info| println!("{:?}", info))),
                    ) {
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

    fn draw_drag_info_by_cursor(&self, ui: &mut egui::Ui) {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);

        // Paint a caption under the cursor in a layer above.
        let layer_id = egui::LayerId::new(egui::Order::Tooltip, self.state.id);

        let hover_pos = ui.input(|i| i.pointer.hover_pos().unwrap());
        let mut end = hover_pos;
        end.x += 70.0;
        end.y += 50.0;

        ui.allocate_ui_at_rect(egui::Rect::from_two_pos(hover_pos, end), |ui| {
            ui.with_layer_id(layer_id, |ui| {
                egui::Frame::none()
                    .fill(ui.visuals().extreme_bg_color.gamma_multiply(0.6))
                    .rounding(3.0)
                    .inner_margin(egui::Margin::symmetric(12.0, 7.0))
                    .show(ui, |ui| {
                        ui.label(self.state.drag_caption());
                    });
            })
        });
    }

    pub fn remove(&mut self, f: &File) {
        if let Some(node) = self.root.find_mut(f.parent) {
            if let Some(mut removed) = node.remove(f.id) {
                clear_children(&mut self.state, &mut removed);
            }
        }
    }

    pub fn get_selected_files(&self) -> Vec<File> {
        self.state
            .selected
            .iter()
            .map(|id| self.root.find(*id).unwrap().file.clone())
            .collect()
    }

    /// expand the parents of the file and select it
    pub fn reveal_file(&mut self, id: Uuid, ctx: &egui::Context) {
        self.state.selected.clear();
        self.state.selected.insert(id);
        self.state.request_scroll = Some(id);

        if let Ok(mut curr) = self.core.get_file_by_id(id) {
            loop {
                let parent = self.core.get_file_by_id(curr.parent).unwrap();
                self.state.expanded.insert(parent.id);
                if parent == curr {
                    break;
                }
                curr = parent;
            }
            ctx.request_repaint();
        }
    }
}

pub fn create_root_node(all_metas: Vec<File>) -> TreeNode {
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
