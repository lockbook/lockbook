use eframe::egui;
use egui_extras::RetainedImage;

use super::{Tab, TabContent};

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

    pub fn open_tab(&mut self, id: lb::Uuid, name: &str) {
        self.tabs
            .push(Tab { id, name: name.to_owned(), failure: None, content: None });
        self.active_tab = self.tabs.len() - 1;
    }

    pub fn get_mut_tab_by_id(&mut self, id: lb::Uuid) -> Option<&mut Tab> {
        for tab in &mut self.tabs {
            if tab.id == id {
                return Some(tab);
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn close_current_tab(&mut self) {
        self.tabs.remove(self.active_tab);
        let n_tabs = self.tabs.len();
        if self.active_tab >= n_tabs && n_tabs > 0 {
            self.active_tab = n_tabs - 1;
        }
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
        ui.vertical(|ui| {
            if self.workspace.tabs.len() > 1 {
                ui.horizontal(|ui| {
                    for (i, t) in self.workspace.tabs.iter().enumerate() {
                        if ui
                            .selectable_label(self.workspace.active_tab == i, &t.name)
                            .clicked()
                        {
                            self.workspace.active_tab = i;
                            frame.set_window_title(&t.name);
                        }
                    }
                });
                ui.separator();
                ui.add_space(5.0);
            }

            ui.centered_and_justified(|ui| {
                if let Some(tab) = self.workspace.tabs.get_mut(self.workspace.active_tab) {
                    if let Some(fail) = &tab.failure {
                        fail.show(ui);
                    } else if let Some(content) = &mut tab.content {
                        match content {
                            TabContent::Drawing(draw) => draw.show(ui),
                            TabContent::Markdown(md) => md.show(ui),
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

    pub fn save_current_tab(&self) {
        if let Some(tab) = self.workspace.current_tab() {
            if let Some(content) = &tab.content {
                if let TabContent::Drawing(d) = content {
                    self.core.save_drawing(tab.id, &d.drawing).unwrap(); // TODO
                } else {
                    let maybe_bytes = match content {
                        TabContent::Markdown(md) => Some(md.content.as_bytes()),
                        TabContent::PlainText(txt) => Some(txt.content.as_bytes()),
                        _ => None,
                    };

                    if let Some(bytes) = maybe_bytes {
                        self.core.write_document(tab.id, bytes).unwrap(); // TODO
                    }
                }
            }
        }
    }
}

const LOGO_BACKDROP: &[u8] = include_bytes!("../../lockbook-backdrop.png");
