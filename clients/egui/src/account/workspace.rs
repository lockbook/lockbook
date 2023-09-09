use std::time::Instant;

use eframe::egui;
use egui_extras::RetainedImage;

use crate::widgets::separator;
use crate::{theme::Icon, widgets::Button};

use super::modals::ErrorModal;
use super::OpenModal;
use super::{tabs::SaveRequestContent, AccountUpdate, FileTree, Tab, TabContent, TabFailure};

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

        if self.workspace.is_empty() {
            self.show_empty_workspace(ui);
        } else {
            ui.centered_and_justified(|ui| self.show_tabs(frame, ui));
        }

        if self.settings.read().unwrap().zen_mode {
            let mut min = ui.clip_rect().left_bottom();
            min.y -= 37.0; // 37 is approximating the height of the button
            let max = ui.clip_rect().left_bottom();

            let rect = egui::Rect { min, max };
            ui.allocate_ui_at_rect(rect, |ui| {
                let zen_mode_btn = Button::default()
                    .icon(&Icon::SHOW_SIDEBAR)
                    .frame(true)
                    .show(ui);
                if zen_mode_btn.clicked() {
                    self.settings.write().unwrap().zen_mode = false;
                    if let Err(err) = self.settings.read().unwrap().to_file() {
                        self.modals.error = Some(ErrorModal::new(err));
                    }
                }
                zen_mode_btn.on_hover_text("Show side panel");
            });
        }
    }

    fn show_empty_workspace(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(ui.clip_rect().height() / 3.0);
            self.workspace
                .backdrop
                .show_size(ui, egui::vec2(100.0, 100.0));

            ui.label(egui::RichText::new("Welcome to your Lockbook").size(40.0));
            ui.label(
                "Right click on your file tree to explore all that your lockbook has to offer",
            );

            ui.add_space(40.0);

            ui.visuals_mut().widgets.inactive.bg_fill = ui.visuals().widgets.active.bg_fill;
            ui.visuals_mut().widgets.hovered.bg_fill = ui.visuals().widgets.active.bg_fill;

            let text_stroke =
                egui::Stroke { color: ui.visuals().extreme_bg_color, ..Default::default() };
            ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
            ui.visuals_mut().widgets.active.fg_stroke = text_stroke;
            ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;

            if Button::default()
                .text("New document")
                .frame(true)
                .show(ui)
                .clicked()
            {
                self.update_tx.send(OpenModal::NewDoc(None).into()).unwrap();
                ui.ctx().request_repaint();
            }
            ui.visuals_mut().widgets.inactive.fg_stroke =
                egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };
            ui.visuals_mut().widgets.hovered.fg_stroke =
                egui::Stroke { color: ui.visuals().widgets.active.bg_fill, ..Default::default() };
            if Button::default().text("New folder").show(ui).clicked() {
                self.update_tx
                    .send(OpenModal::NewFolder(None).into())
                    .unwrap();
                ui.ctx().request_repaint();
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
                                    self.tree.reveal_file(self.workspace.tabs[i].id, &self.core);
                                }
                                TabLabelResponse::Closed => {
                                    self.close_tab(ui.ctx(), i);
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

    pub fn save_all_tabs(&self, ctx: &egui::Context) {
        for (i, _) in self.workspace.tabs.iter().enumerate() {
            self.save_tab(ctx, i);
        }
    }

    pub fn save_tab(&self, ctx: &egui::Context, i: usize) {
        if let Some(tab) = self.workspace.tabs.get(i) {
            if tab.is_dirty() {
                if let Some(save_req) = tab.make_save_request() {
                    let core = self.core.clone();
                    let update_tx = self.update_tx.clone();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || {
                        let content = save_req.content;
                        let id = save_req.id;

                        let result = match content {
                            SaveRequestContent::Text(s) => core.write_document(id, s.as_bytes()),
                            SaveRequestContent::Draw(d) => core.save_drawing(id, &d),
                        }
                        .map(|_| Instant::now());

                        update_tx
                            .send(AccountUpdate::SaveResult(id, result))
                            .unwrap();
                        ctx.request_repaint();
                    });
                }
            }
        }
    }

    pub fn close_tab(&mut self, ctx: &egui::Context, i: usize) {
        self.save_tab(ctx, i);
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
