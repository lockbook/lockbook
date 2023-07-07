use std::time::Instant;

use eframe::egui;
use egui_extras::RetainedImage;

use crate::theme::Icon;
use crate::widgets::separator;

use super::{FileTree, Tab, TabContent, TabFailure};

pub struct Workspace {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub backdrop: RetainedImage,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            backdrop: RetainedImage::from_image_bytes("logo-backdrop", LOGO_BACKDROP).unwrap(),
        }
    }

    pub fn open_tab(&mut self, id: lb::Uuid, name: &str, path: &str) {
        let now = Instant::now();
        self.tabs.push(Tab {
            id,
            name: name.to_owned(),
            path: path.to_owned(),
            failure: None,
            content: None,
            last_changed: now,
            last_saved: now,
        });
        self.active_tab = self.tabs.len() - 1;
    }

    pub fn get_mut_tab_by_id(&mut self, id: lb::Uuid) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn goto_tab_id(&mut self, id: lb::Uuid) -> bool {
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.id == id {
                self.active_tab = i;
                return true;
            }
        }
        false
    }

    pub fn goto_tab(&mut self, i: usize) {
        if i == 0 || self.tabs.is_empty() {
            return;
        }
        let n_tabs = self.tabs.len();
        self.active_tab = if i == 9 || i >= n_tabs { n_tabs - 1 } else { i - 1 };
    }
}

enum TabLabelResponse {
    Clicked,
    Closed,
}

fn tab_label(ui: &mut egui::Ui, t: &Tab, is_active: bool) -> Option<TabLabelResponse> {
    let mut lbl_resp = None;

    let padding = egui::vec2(15.0, 15.0);
    let wrap_width = ui.available_width();

    let text: egui::WidgetText = (&t.name).into();
    let text = text.into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body);

    let x_icon = Icon::CLOSE.size(16.0);

    let w = text.size().x + padding.x * 3.0 + x_icon.size + 1.0;
    let h = text.size().y + padding.y * 2.0;

    let (rect, resp) = ui.allocate_exact_size((w, h).into(), egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let visuals = &ui.style().interact(&resp).clone();

        let close_btn_pos =
            egui::pos2(rect.max.x - padding.x - x_icon.size, rect.center().y - x_icon.size / 2.0);

        let close_btn_rect =
            egui::Rect::from_min_size(close_btn_pos, egui::vec2(x_icon.size, x_icon.size))
                .expand(2.0);

        let mut close_hovered = false;
        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        if let Some(pos) = pointer_pos {
            if close_btn_rect.contains(pos) {
                close_hovered = true;
            }
        }

        let bg = if resp.hovered() && !close_hovered {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            ui.visuals().widgets.noninteractive.bg_fill
        };
        ui.painter().rect(rect, 0.0, bg, egui::Stroke::NONE);

        let text_pos = egui::pos2(rect.min.x + padding.x, rect.center().y - 0.5 * text.size().y);

        text.paint_with_visuals(ui.painter(), text_pos, visuals);

        if close_hovered {
            ui.painter().rect(
                close_btn_rect,
                0.0,
                ui.visuals().widgets.hovered.bg_fill,
                egui::Stroke::NONE,
            );
        }

        let icon_draw_pos = egui::pos2(
            rect.max.x - padding.x - x_icon.size - 1.0,
            rect.center().y - x_icon.size / 4.1 - 1.0,
        );

        let icon: egui::WidgetText = (&x_icon).into();
        icon.into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body)
            .paint_with_visuals(ui.painter(), icon_draw_pos, visuals);

        let close_resp = ui.interact(
            close_btn_rect,
            egui::Id::new(format!("close-btn-{}", t.id)),
            egui::Sense::click(),
        );
        // First, we check if the close button was clicked.
        if close_resp.clicked() {
            lbl_resp = Some(TabLabelResponse::Closed);
        } else {
            // Then, we check if the tab label was clicked so that a close button click
            // wouldn't also count here.
            let resp = resp.interact(egui::Sense::click());
            if resp.clicked() {
                lbl_resp = Some(TabLabelResponse::Clicked);
            } else if resp.middle_clicked() {
                lbl_resp = Some(TabLabelResponse::Closed);
            }
        }

        if is_active {
            ui.painter().hline(
                rect.min.x + 0.5..=rect.max.x - 1.0,
                rect.max.y - 2.0,
                egui::Stroke::new(4.0, ui.visuals().widgets.active.bg_fill),
            );
        }

        let sep_stroke = if resp.hovered() && !close_hovered {
            egui::Stroke::new(1.0, egui::Color32::TRANSPARENT)
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke
        };
        ui.painter().vline(rect.max.x, rect.y_range(), sep_stroke);
    }

    lbl_resp
}

impl super::AccountScreen {
    pub fn show_workspace(&mut self, frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        ui.set_enabled(!self.is_any_modal_open());

        ui.centered_and_justified(|ui| {
            if self.workspace.is_empty() {
                self.workspace
                    .backdrop
                    .show_size(ui, egui::vec2(360.0, 360.0));
            } else {
                self.show_tabs(frame, ui);
            }
        });
    }

    fn show_tabs(&mut self, frame: &mut eframe::Frame, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        ui.vertical(|ui| {
            if self.workspace.tabs.len() > 1 {
                ui.horizontal(|ui| {
                    for (i, maybe_resp) in self
                        .workspace
                        .tabs
                        .iter()
                        .enumerate()
                        .map(|(i, t)| tab_label(ui, t, self.workspace.active_tab == i))
                        .collect::<Vec<Option<TabLabelResponse>>>()
                        .iter()
                        .enumerate()
                    {
                        if let Some(resp) = maybe_resp {
                            match resp {
                                TabLabelResponse::Clicked => {
                                    self.workspace.active_tab = i;
                                    frame.set_window_title(&self.workspace.tabs[i].name);
                                }
                                TabLabelResponse::Closed => {
                                    self.close_tab(i);
                                    frame.set_window_title(match self.workspace.current_tab() {
                                        Some(tab) => &tab.name,
                                        None => "Lockbook",
                                    });
                                }
                            }
                            ui.ctx().request_repaint();
                        }
                    }
                });

                separator(ui);
            }

            ui.centered_and_justified(|ui| {
                if let Some(tab) = self.workspace.tabs.get_mut(self.workspace.active_tab) {
                    if let Some(fail) = &tab.failure {
                        match fail {
                            TabFailure::DeletedFromSync => {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(50.0);
                                    ui.label(&format!(
                                        "This file ({}) was deleted after syncing.",
                                        tab.path
                                    ));

                                    ui.add_space(10.0);
                                    ui.label("Would you like to restore it?");

                                    ui.add_space(15.0);
                                    if ui.button("Yes, Restore Me").clicked() {
                                        restore_tab(&self.core, &mut self.tree, tab);
                                    }
                                });
                            }
                            TabFailure::SimpleMisc(msg) => {
                                ui.label(msg);
                            }
                            TabFailure::Unexpected(msg) => {
                                ui.label(msg);
                            }
                        };
                    } else if let Some(content) = &mut tab.content {
                        match content {
                            TabContent::Drawing(draw) => draw.show(ui),
                            TabContent::Markdown(md) => {
                                let resp = md.show(ui);
                                // The editor signals a text change when the buffer is initially
                                // loaded. Since we use that signal to trigger saves, we need to
                                // check that this change was not from the initial frame.
                                if resp.text_updated && md.past_first_frame() {
                                    tab.last_changed = Instant::now();
                                }
                            }
                            TabContent::PlainText(txt) => txt.show(ui),
                            TabContent::Image(img) => img.show(ui),
                        };
                    } else {
                        ui.spinner();
                    }
                }
            });
        });
    }

    pub fn save_all_tabs(&self) {
        for (i, _) in self.workspace.tabs.iter().enumerate() {
            self.save_tab(i);
        }
    }

    pub fn save_tab(&self, i: usize) {
        if let Some(tab) = self.workspace.tabs.get(i) {
            if tab.is_dirty() {
                if let Some(save_req) = tab.make_save_request() {
                    self.save_req_tx.send(save_req).unwrap();
                }
            }
        }
    }

    pub fn close_tab(&mut self, i: usize) {
        self.save_tab(i);
        let ws = &mut self.workspace;
        ws.tabs.remove(i);
        let n_tabs = ws.tabs.len();
        if ws.active_tab >= n_tabs && n_tabs > 0 {
            ws.active_tab = n_tabs - 1;
        }
    }
}

fn restore_tab(core: &lb::Core, tree: &mut FileTree, tab: &mut Tab) {
    let file = match core.create_at_path(&tab.path) {
        Ok(f) => f,
        Err(err) => {
            tab.failure = Some(TabFailure::Unexpected(format!("{:?}", err)));
            return;
        }
    };

    // We create a new file to restore a document, so the tab needs the new ID.
    tab.id = file.id;

    if let Some(content) = &tab.content {
        // Save the document content.
        let save_result = if let TabContent::Drawing(d) = content {
            core.save_drawing(file.id, &d.drawing)
                .map_err(TabFailure::from)
        } else {
            let maybe_bytes = match content {
                TabContent::Markdown(md) => Some(md.editor.buffer.current.text.as_bytes()),
                TabContent::PlainText(txt) => Some(txt.content.as_bytes()),
                TabContent::Image(img) => Some(img.bytes.as_slice()),
                _ => None,
            };

            if let Some(bytes) = maybe_bytes {
                // todo(steve)
                core.write_document(file.id, bytes)
                    .map_err(|err| TabFailure::Unexpected(format!("{:?}", err)))
            } else {
                Ok(())
            }
        };

        // Set a new TabFailure if the content couldn't successfully be saved.
        tab.failure = save_result.err();

        // Ensure each parent folder is in the tree and then expand to the file.
        match get_parents(core, file.id) {
            Ok(parents) => {
                let mut node = &mut tree.root;
                for p in parents {
                    if node.find(p.id).is_none() {
                        node.insert(p.clone());
                    }
                    node = node.find_mut(p.id).unwrap();
                }
                tree.expand_to(file.id);
            }
            Err(msg) => tab.failure = Some(TabFailure::Unexpected(msg)),
        };
    }
}

// Gets all parents except root in descending order.
fn get_parents(core: &lb::Core, id: lb::Uuid) -> Result<Vec<lb::File>, String> {
    let mut parents = Vec::new();
    let mut id = id;
    loop {
        let file = core
            .get_file_by_id(id)
            .map_err(|err| format!("{:?}", err))?;
        if file.id == file.parent {
            break;
        }
        id = file.parent;
        parents.insert(0, file);
    }
    Ok(parents)
}

const LOGO_BACKDROP: &[u8] = include_bytes!("../../lockbook-backdrop.png");
