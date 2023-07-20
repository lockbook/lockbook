mod accept_share;
mod confirm_delete;
mod create_share;
mod error;
mod file_picker;
mod help;
mod new_file;
mod search;
mod settings;

pub use accept_share::AcceptShareModal;
pub use confirm_delete::ConfirmDeleteModal;
pub use create_share::{CreateShareModal, CreateShareParams};
pub use error::ErrorModal;
pub use file_picker::FilePicker;
pub use file_picker::FilePickerAction;
pub use help::HelpModal;
pub use new_file::{NewDocModal, NewFileParams, NewFolderModal};
pub use search::SearchModal;
pub use settings::{SettingsModal, SettingsResponse};

use super::OpenModal;
use eframe::egui;

#[derive(Default)]
pub struct Modals {
    pub accept_share: Option<AcceptShareModal>,
    pub confirm_delete: Option<ConfirmDeleteModal>,
    pub create_share: Option<CreateShareModal>,
    pub error: Option<ErrorModal>,
    pub file_picker: Option<FilePicker>,
    pub help: Option<HelpModal>,
    pub new_doc: Option<NewDocModal>,
    pub new_folder: Option<NewFolderModal>,
    pub search: Option<SearchModal>,
    pub settings: Option<SettingsModal>,
}

impl super::AccountScreen {
    pub fn show_any_modals(&mut self, ctx: &egui::Context, x_offset: f32) {
        show(ctx, x_offset, &mut self.modals.error);

        show(ctx, x_offset, &mut self.modals.help);

        if let Some(response) = show(ctx, x_offset, &mut self.modals.accept_share) {
            if let Some(submission) = response.inner {
                if submission.is_accept {
                    self.update_tx
                        .send(OpenModal::PickShareParent(submission.target).into())
                        .unwrap();
                    self.modals.accept_share = None;
                } else {
                    self.delete_share(submission.target);

                    // close and reopen the modal to force a state reload and make the deleted share disappear
                    self.modals.accept_share = None;
                    self.update_tx.send(OpenModal::AcceptShare.into()).unwrap();
                }
            }
        }

        if let Some(response) = show(ctx, x_offset, &mut self.modals.settings) {
            if response.closed {
                self.save_settings();
            } else if let Some(inner) = response.inner {
                use SettingsResponse::*;
                match inner {
                    SuccessfullyUpgraded => self.refresh_sync_status(ctx),
                    ToggleToolbarVisibility(_) => {
                        self.modals.settings = None;
                        self.reload_markdown_tabs(ctx)
                    }
                }
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

        if let Some(response) = show(ctx, x_offset, &mut self.modals.new_doc) {
            if let Some(submission) = response.inner {
                self.create_file(submission);
            }
        }

        if let Some(response) = show(ctx, x_offset, &mut self.modals.new_folder) {
            if let Some(submission) = response.inner {
                self.create_file(submission);
            }
        }

        if let Some(response) = show(ctx, x_offset, &mut self.modals.create_share) {
            if let Some(submission) = response.inner {
                self.create_share(submission)
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

        if let Some(response) = show(ctx, x_offset, &mut self.modals.file_picker) {
            if let Some(submission) = response.inner {
                match submission.action {
                    FilePickerAction::AcceptShare(target) => {
                        self.accept_share(ctx, target, submission.parent)
                    }
                    FilePickerAction::DroppedFile(target) => {
                        self.dropped_file(ctx, target, submission.parent)
                    }
                }
            }
        }
    }

    pub fn is_any_modal_open(&self) -> bool {
        let m = &self.modals;
        m.settings.is_some()
            || m.accept_share.is_some()
            || m.new_doc.is_some()
            || m.new_folder.is_some()
            || m.create_share.is_some()
            || m.file_picker.is_some()
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
        if m.new_doc.is_some() {
            m.new_doc = None;
            return true;
        }
        if m.new_folder.is_some() {
            m.new_folder = None;
            return true;
        }
        if m.create_share.is_some() {
            m.create_share = None;
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
        if m.accept_share.is_some() {
            m.confirm_delete = None;
            return true;
        }
        if m.file_picker.is_some() {
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
    ctx: &egui::Context, x_offset: f32, maybe_modal: &mut Option<M>,
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
    ctx: &egui::Context, x_offset: f32, d: &mut M,
) -> ModalResponse<M::Response> {
    let mut is_open = true;

    let title = d.title();

    let frame = egui::Frame::window(&ctx.style()).inner_margin(egui::style::Margin {
        left: 0.0,
        bottom: 0.0,
        ..ctx.style().spacing.window_margin
    });

    let win_resp = egui::Window::new(title)
        .anchor(M::ANCHOR, egui::vec2(x_offset, M::Y_OFFSET))
        .title_bar(!title.is_empty())
        .open(&mut is_open)
        .collapsible(false)
        .resizable(false)
        .default_width(400.0)
        .default_height(f32::INFINITY)
        .frame(frame)
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
