use std::cmp::Ordering;

use eframe::egui;

use crate::model::DocType;
use crate::theme::Icon;

use super::response::*;
use super::state::*;

pub struct TreeNode {
    pub file: lb::File,
    pub doc_type: Option<DocType>,
    pub children: Vec<TreeNode>,

    depth: u8,
    primary_press: Option<egui::Pos2>,
    hovering_drop: bool,
}

impl From<(lb::File, u8)> for TreeNode {
    fn from(data: (lb::File, u8)) -> Self {
        let (file, depth) = data;
        let doc_type = match file.file_type {
            lb::FileType::Folder => None,
            lb::FileType::Document => Some(DocType::from_name(&file.name)),
            lb::FileType::Link { .. } => todo!(),
        };

        Self {
            file,
            doc_type,
            children: Vec::new(),
            depth,
            primary_press: None,
            hovering_drop: false,
        }
    }
}

impl TreeNode {
    pub fn populate_from(&mut self, all_metas: &Vec<lb::File>) {
        self.children = all_metas
            .iter()
            .filter_map(|f| {
                if f.parent == self.file.id {
                    let mut node = TreeNode::from((f.clone(), self.depth + 1));
                    node.populate_from(all_metas);
                    Some(node)
                } else {
                    None
                }
            })
            .collect();
        self.children.sort();
    }

    pub fn show(
        &mut self, ui: &mut egui::Ui, state: &mut TreeState,
    ) -> egui::InnerResponse<NodeResponse> {
        let (mut resp, mut node_resp) = if state.renaming.id == Some(self.file.id) {
            let mut node_resp = NodeResponse::default();
            let resp = ui
                .horizontal(|ui| {
                    ui.add_space(self.depth_inset() + 5.0);

                    let resp = egui::TextEdit::singleline(&mut state.renaming.tmp_name)
                        .margin(egui::vec2(6.0, 6.0))
                        .hint_text("Name...")
                        .id(egui::Id::new("rename_field"))
                        .show(ui)
                        .response;

                    if resp.lost_focus() || resp.clicked_elsewhere() {
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            node_resp.rename_request =
                                Some((self.file.id, state.renaming.tmp_name.clone()));
                        }
                        state.renaming = NodeRenamingState::default();
                        ui.ctx().request_repaint();
                    } else if !resp.has_focus() {
                        resp.request_focus();
                    }
                })
                .response;

            (resp, node_resp)
        } else {
            self.draw_normal(ui, state)
        };

        // Draw any children, if expanded, and merge their responses.
        if state.expanded.contains(&self.file.id) {
            for node in self.children.iter_mut() {
                let child_resp = node.show(ui, state);
                node_resp = node_resp.union(child_resp.inner);
                resp = resp.union(child_resp.response);
            }
        }

        egui::InnerResponse::new(node_resp, resp)
    }

    fn draw_normal(
        &mut self, ui: &mut egui::Ui, state: &mut TreeState,
    ) -> (egui::Response, NodeResponse) {
        let mut node_resp = NodeResponse::default();

        let mut resp = self.draw_icon_and_text(ui, state);

        if resp.hovered()
            && ui.input(|i| i.pointer.any_pressed())
            && ui.input(|i| i.pointer.primary_down())
        {
            self.primary_press = ui.input(|i| i.pointer.press_origin());

            if ui.input(|i| i.modifiers.ctrl) {
                state.toggle_selected(self.file.id);
            } else if !state.selected.contains(&self.file.id) {
                state.selected.clear();
                state.selected.insert(self.file.id);
            }
        } else if ui.input(|i| i.pointer.any_released()) {
            if let Some(pos) = self.primary_press {
                // Mouse was released over an item on which it was originally pressed.
                if resp.hovered()
                    && resp.rect.contains(pos)
                    && state.selected.len() > 1
                    && state.selected.contains(&self.file.id)
                    && !ui.input(|i| i.modifiers.ctrl)
                {
                    state.selected.retain(|id| *id == self.file.id);
                }
            }
            self.primary_press = None;
        }

        resp = ui.interact(resp.rect, resp.id, egui::Sense::click());
        if resp.double_clicked() {
            if self.file.is_folder() {
                if !state.expanded.remove(&self.file.id) {
                    state.expanded.insert(self.file.id);
                }
            } else {
                node_resp.open_requests.insert(self.file.id); // Signal that this document was opened this frame.
            }
        }

        if let Some(pos) = state.dnd.dropped {
            if resp.rect.contains(pos) {
                node_resp.dropped_on =
                    Some(if self.file.is_folder() { self.file.id } else { self.file.parent });
                if self.file.is_folder() {
                    state.expanded.insert(self.file.id);
                }
                state.dnd.dropped = None;
            }
        }

        resp = resp.context_menu(|ui| self.context_menu(ui, &mut node_resp, state));

        (resp, node_resp)
    }

    fn draw_icon_and_text(&mut self, ui: &mut egui::Ui, state: &mut TreeState) -> egui::Response {
        let text_height = ui.text_style_height(&egui::TextStyle::Body);
        let padding = ui.spacing().button_padding;

        let depth_inset = self.depth_inset() + 5.0;
        let wrap_width = ui.available_width();

        let icon = if self.file.is_folder() { Icon::FOLDER_OPEN } else { self.icon() };

        let icon: egui::WidgetText = (&icon).into();
        let icon = icon.into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body);

        let text: egui::WidgetText = (&self.file.name).into();
        let text = text.into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body);

        let width = (depth_inset + padding.x * 2.0 + icon.size().x + 5.0 + text.size().x)
            .max(ui.available_size_before_wrap().x);
        if width > state.max_node_width {
            state.max_node_width = width;
            ui.ctx().request_repaint();
        }

        let desired_size = egui::vec2(state.max_node_width, text_height + padding.y * 2.0);

        let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            let bg = if state.selected.contains(&self.file.id) {
                ui.visuals().widgets.active.bg_fill
            } else if resp.hovered() {
                ui.visuals().widgets.hovered.bg_fill
            } else {
                ui.visuals().panel_fill
            };

            ui.painter().rect(rect, 0.0, bg, egui::Stroke::NONE);

            let icon_pos =
                egui::pos2(rect.min.x + depth_inset, rect.center().y - icon.size().y / 4.0 - 1.0);

            let text_pos = egui::pos2(
                rect.min.x + depth_inset + padding.x + icon.size().x,
                rect.center().y - 0.5 * text.size().y,
            );

            let visuals = ui.style().interact(&resp);

            icon.paint_with_visuals(ui.painter(), icon_pos, visuals);
            text.paint_with_visuals(ui.painter(), text_pos, visuals);
        }

        let is_drop_target = self.file.is_folder()
            && resp.hovered()
            && state.is_dragging()
            && !state.selected.contains(&self.file.id);

        if !self.hovering_drop && is_drop_target {
            self.hovering_drop = true;
            ui.ctx().request_repaint();
        } else if self.hovering_drop && !is_drop_target {
            self.hovering_drop = false;
            ui.ctx().request_repaint();
        }

        resp
    }

    fn depth_inset(&self) -> f32 {
        (self.depth as f32) * 20.0
    }

    fn icon(&self) -> Icon {
        if self.hovering_drop {
            Icon::ARROW_CIRCLE_DOWN
        } else if let Some(typ) = &self.doc_type {
            match typ {
                DocType::Markdown | DocType::PlainText => Icon::DOC_TEXT,
                DocType::Drawing => Icon::DRAW,
                DocType::Image(_) => Icon::IMAGE,
                DocType::Code(_) => Icon::CODE,
                _ => Icon::DOC_UNKNOWN,
            }
        } else {
            Icon::FOLDER
        }
    }

    fn context_menu(
        &mut self, ui: &mut egui::Ui, node_resp: &mut NodeResponse, state: &mut TreeState,
    ) {
        state.selected.clear();
        state.selected.insert(self.file.id);

        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.close_menu();
        }

        if ui.button("New Document").clicked() {
            node_resp.new_doc_modal = Some(self.file.clone());
            ui.close_menu();
        }
        if ui.button("New Folder").clicked() {
            node_resp.new_folder_modal = Some(self.file.clone());
            ui.close_menu();
        }
        if ui.button("Rename").clicked() {
            state.renaming = NodeRenamingState::new(&self.file);

            let name = &state.renaming.tmp_name;
            let end_pos = name.rfind('.').unwrap_or(name.len());

            let mut rename_edit_state = egui::text_edit::TextEditState::default();
            rename_edit_state.set_ccursor_range(Some(egui::text_edit::CCursorRange {
                primary: egui::text::CCursor::new(end_pos),
                secondary: egui::text::CCursor::new(0),
            }));
            egui::TextEdit::store_state(ui.ctx(), egui::Id::new("rename_field"), rename_edit_state);

            ui.close_menu();
        }
        if ui.button("Export").clicked() {
            ui.close_menu();
        }
        if ui.button("Delete").clicked() {
            node_resp.delete_request = true;
            ui.close_menu();
        }
    }

    pub fn insert(&mut self, meta: lb::File) -> bool {
        if let Some(parent) = self.find_mut(meta.parent) {
            let node = TreeNode::from((meta, parent.depth + 1));
            for (i, child) in parent.children.iter().enumerate() {
                if node < *child {
                    parent.children.insert(i, node);
                    return true;
                }
            }
            parent.children.push(node);
            return true;
        }
        false
    }

    pub fn insert_node(&mut self, node: Self) {
        let mut node = node;
        node.file.parent = self.file.id;
        node.depth = self.depth + 1;

        for (i, child) in self.children.iter().enumerate() {
            if node < *child {
                self.children.insert(i, node);
                return;
            }
        }

        self.children.push(node);
    }

    pub fn remove(&mut self, id: lb::Uuid) -> Option<TreeNode> {
        for (i, node) in self.children.iter().enumerate() {
            if node.file.id == id {
                return Some(self.children.remove(i));
            }
        }
        None
    }

    pub fn find(&self, id: lb::Uuid) -> Option<&TreeNode> {
        if self.file.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(node) = child.find(id) {
                return Some(node);
            }
        }
        None
    }

    pub fn find_mut(&mut self, id: lb::Uuid) -> Option<&mut TreeNode> {
        if self.file.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(node) = child.find_mut(id) {
                return Some(node);
            }
        }
        None
    }
}

impl PartialEq for TreeNode {
    fn eq(&self, other: &Self) -> bool {
        self.file.id == other.file.id
    }
}

impl Eq for TreeNode {}

impl Ord for TreeNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.file.is_folder() && !other.file.is_folder() {
            Ordering::Less
        } else if other.file.is_folder() && !self.file.is_folder() {
            Ordering::Greater
        } else {
            self.file.name.cmp(&other.file.name)
        }
    }
}

impl PartialOrd for TreeNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
