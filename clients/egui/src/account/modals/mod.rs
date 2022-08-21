mod confirm_delete;
mod error;
mod help;
mod new_file;
mod search;
mod settings;

pub use confirm_delete::ConfirmDeleteModal;
pub use error::ErrorModal;
pub use help::HelpModal;
pub use new_file::{NewFileModal, NewFileParams};
pub use search::SearchModal;
pub use settings::SettingsModal;

use eframe::egui;

#[derive(Default)]
pub struct Modals {
    pub error: Option<Box<ErrorModal>>,
    pub settings: Option<Box<SettingsModal>>,
    pub new_file: Option<Box<NewFileModal>>,
    pub search: Option<Box<SearchModal>>,
    pub help: Option<Box<HelpModal>>,
    pub confirm_delete: Option<Box<ConfirmDeleteModal>>,
}

impl super::AccountScreen {
    pub fn show_any_modals(&mut self, ctx: &egui::Context, x_offset: f32) {
        show(ctx, x_offset, &mut self.modals.error);

        show(ctx, x_offset, &mut self.modals.help);

        if let Some(response) = show(ctx, x_offset, &mut self.modals.settings) {
            if response.closed {
                self.save_settings();
            }
        }

        if let Some(response) = show(ctx, x_offset, &mut self.modals.search) {
            if let Some(submission) = response.inner {
                self.open_file(submission.id, ctx);
                if submission.close {
                    self.modals.search = None;
                }
            }
        }

        if let Some(response) = show(ctx, x_offset, &mut self.modals.new_file) {
            if let Some(submission) = response.inner {
                self.create_file(submission);
            }
        }

        if let Some(response) = show(ctx, x_offset, &mut self.modals.confirm_delete) {
            if let Some((answer, files)) = response.inner {
                if answer {
                    self.delete_files(ctx, files);
                } else {
                    self.modals.confirm_delete = None;
                }
            }
        }
    }

    pub fn is_any_modal_open(&self) -> bool {
        let m = &self.modals;
        m.settings.is_some()
            || m.new_file.is_some()
            || m.search.is_some()
            || m.help.is_some()
            || m.confirm_delete.is_some()
    }

    pub fn close_something(&mut self) -> bool {
        let m = &mut self.modals;
        if m.settings.is_some() {
            m.settings = None;
            self.save_settings();
            return true;
        }
        if m.new_file.is_some() {
            m.new_file = None;
            return true;
        }
        if m.search.is_some() {
            m.search = None;
            return true;
        }
        if m.help.is_some() {
            m.help = None;
            return true;
        }
        if m.confirm_delete.is_some() {
            m.confirm_delete = None;
            return true;
        }
        false
    }
}

pub trait Modal {
    const ANCHOR: egui::Align2 = egui::Align2::CENTER_CENTER;
    const Y_OFFSET: f32 = 0.0;

    type Response;

    fn title(&self) -> &str;
    fn show(&mut self, ui: &mut egui::Ui) -> Self::Response;
}

pub fn show<M: Modal>(
    ctx: &egui::Context, x_offset: f32, maybe_modal: &mut Option<Box<M>>,
) -> Option<ModalResponse<M::Response>> {
    if let Some(d) = maybe_modal {
        let dr = show_modal(ctx, x_offset, d);
        if dr.closed {
            *maybe_modal = None;
        }
        Some(dr)
    } else {
        None
    }
}

pub struct ModalResponse<R> {
    pub inner: R,
    pub closed: bool,
}

fn show_modal<M: Modal>(
    ctx: &egui::Context, x_offset: f32, d: &mut Box<M>,
) -> ModalResponse<M::Response> {
    let mut is_open = true;

    let title = d.title();

    let win_resp = egui::Window::new(title)
        .anchor(M::ANCHOR, egui::vec2(x_offset, M::Y_OFFSET))
        .title_bar(!title.is_empty())
        .open(&mut is_open)
        .collapsible(false)
        .default_width(400.0)
        .default_height(f32::INFINITY)
        .show(ctx, |ui| d.show(ui))
        .unwrap(); // Will never be `None` because `is_open` will always start as `true`.

    // Indicate the window closed if the user clicked outside its area.
    if win_resp.response.clicked_elsewhere() {
        is_open = false;
    }

    // The inner response will never be `None` because our Modals are not collapsible.
    let inner = win_resp.inner.unwrap();

    ModalResponse { inner, closed: !is_open }
}
