//! Canvas context state and commands belong to Rust. Native hosts only
//! present the quick actions and report the tapped action's bit value.
use egui::{Id, Pos2};
use resvg::usvg::Transform;

use super::element::BoundedElement;
use super::toolbar::ToolContext;
use super::tools::InputControllerTool;
use super::tools::selection::{SelectionEvent, is_selectable};
use super::viewport::{get_rect_identity_transform, transform_canvas};
use super::{SVGEditor, Tool};
use crate::tab::{ContextMenuTarget, ExtendedInput, ExtendedOutput};

#[repr(u32)]
#[derive(Clone, Copy, Debug)]
pub enum CanvasAction {
    Copy = 1,
    Delete = 2,
    More = 4,
    Paste = 8,
    ImportImage = 16,
    SelectAll = 32,
    ZoomToFit = 64,
}

#[derive(Default)]
pub(super) struct CanvasContextMenu {
    position: Pos2,
    pub(super) rect: Option<egui::Rect>,
    quick: bool,
    advanced: bool,
    pending_insert: Option<Pos2>,
}

/// Paste / ImportImage are the only commands requiring host services.
/// No selection, request id or insertion position crosses back from the host.
pub fn take_host_command(ctx: &egui::Context) -> u32 {
    ctx.memory_mut(|m| m.data.remove_temp(Id::unique("canvas_host_command")))
        .unwrap_or(0)
}

fn map_position(t: Transform, p: Pos2) -> Pos2 {
    egui::pos2(t.sx * p.x + t.kx * p.y + t.tx, t.ky * p.x + t.sy * p.y + t.ty)
}

impl SVGEditor {
    /// Low bits describe the actions shown; the corresponding high bits
    /// describe enabled actions. The native host need not inspect selection.
    fn context_actions(&self) -> u32 {
        use CanvasAction::*;
        let selected = self
            .toolbar
            .selection
            .selected_elements
            .iter()
            .any(|s| self.buffer.elements.get(&s.id).is_some_and(is_selectable));
        let visible = if selected {
            Copy as u32 | Delete as u32 | More as u32
        } else {
            Paste as u32 | ImportImage as u32 | SelectAll as u32 | ZoomToFit as u32
        };
        let mut enabled = visible;
        if self.read_only {
            enabled &= !(Delete as u32 | Paste as u32 | ImportImage as u32);
        }
        if !self.buffer.elements.values().any(is_selectable) {
            enabled &= !(SelectAll as u32 | ZoomToFit as u32);
        }
        visible | enabled << 16
    }

    pub(super) fn request_context_menu(&mut self, ui: &mut egui::Ui, position: Pos2) {
        let mut ctx = ToolContext {
            painter: &mut self.painter,
            buffer: &mut self.buffer,
            history: &mut self.history,
            settings: &mut self.settings,
            viewport_settings: &mut self.viewport_settings,
            read_only: self.read_only,
        };
        self.toolbar.active_tool_mut().process_controller_event(
            ui,
            crate::tab::input_controller::InputControllerEvent::ToolCancel,
            &mut ctx,
        );
        let selected = self.toolbar.selection.select_at(position, &mut self.buffer);
        if selected {
            self.toolbar.active_tool = Tool::Selection;
        }
        self.toolbar.selection.contextual_menu = true;
        self.context_menu.position = map_position(
            self.viewport_settings
                .master_transform
                .invert()
                .unwrap_or_default(),
            position,
        );
        self.context_menu.advanced = false;
        let native = matches!(
            ui.ctx().os(),
            egui::os::OperatingSystem::Android | egui::os::OperatingSystem::IOS
        );
        self.context_menu.quick = !native;
        if native {
            ui.ctx()
                .set_context_menu(position, ContextMenuTarget::Canvas(self.context_actions()));
        }
    }

    pub(super) fn process_context_actions(&mut self, ui: &mut egui::Ui) {
        for event in ui.ctx().read_events() {
            match event {
                crate::Event::CanvasAction(action) => self.context_action(ui, action),
                crate::Event::CanvasContent(content) => {
                    if !self.read_only {
                        if let Some(position) = self.context_menu.pending_insert.take() {
                            let position =
                                map_position(self.viewport_settings.master_transform, position);
                            ui.ctx()
                                .push_event(crate::Event::CanvasPaste { content, position });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn context_action(&mut self, ui: &mut egui::Ui, action: u32) {
        use CanvasAction::*;
        if action.count_ones() != 1 || (self.context_actions() >> 16) & action == 0 {
            return;
        }
        self.context_menu.quick = false;
        match action {
            a if a == More as u32 => {
                self.context_menu.advanced = true;
                self.toolbar.active_tool = Tool::Selection;
            }
            a if a == Paste as u32 || a == ImportImage as u32 => {
                self.context_menu.pending_insert = Some(self.context_menu.position);
                ui.ctx().memory_mut(|m| {
                    m.data
                        .insert_temp(Id::unique("canvas_host_command"), action)
                });
            }
            a if a == ZoomToFit as u32 => {
                let bounds = self
                    .buffer
                    .elements
                    .values()
                    .filter(|e| is_selectable(e))
                    .fold(egui::Rect::NOTHING, |bounds, e| bounds.union(e.bounding_box()));
                if bounds.min.is_finite()
                    && bounds.max.is_finite()
                    && (bounds.width() > 0. || bounds.height() > 0.)
                {
                    if let Some(t) = get_rect_identity_transform(
                        self.viewport_settings.container_rect,
                        bounds,
                        0.7,
                        self.viewport_settings.container_rect.center(),
                    ) {
                        transform_canvas(&mut self.buffer, &mut self.viewport_settings, t);
                    }
                }
            }
            _ => {
                let event = match action {
                    a if a == Copy as u32 => SelectionEvent::Copy,
                    a if a == Delete as u32 => SelectionEvent::Delete,
                    a if a == SelectAll as u32 => {
                        self.toolbar.active_tool = Tool::Selection;
                        SelectionEvent::SelectAll
                    }
                    _ => return,
                };
                self.toolbar.selection.handle_tool_event(
                    ui,
                    event,
                    &mut ToolContext {
                        painter: &mut self.painter,
                        buffer: &mut self.buffer,
                        history: &mut self.history,
                        settings: &mut self.settings,
                        viewport_settings: &mut self.viewport_settings,
                        read_only: self.read_only,
                    },
                );
            }
        }
        ui.ctx().request_repaint();
    }

    pub(super) fn show_context_menu(&mut self, ui: &mut egui::Ui) {
        if !self.context_menu.quick && !self.context_menu.advanced {
            self.context_menu.rect = None;
            return;
        }
        let pos = map_position(self.viewport_settings.master_transform, self.context_menu.position);
        let bounds = self.viewport_settings.container_rect;
        let pos = egui::pos2(
            pos.x
                .clamp(bounds.left(), (bounds.right() - 220.).max(bounds.left())),
            pos.y
                .clamp(bounds.top(), (bounds.bottom() - 60.).max(bounds.top())),
        );
        let mut action = None;
        let spec = self.context_actions();
        let advanced = self.context_menu.advanced;
        let result = egui::Area::new(Id::unique("canvas_context_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .constrain_to(bounds)
            .show(ui.ctx(), |ui| {
                egui::Frame::window(ui.style()).show(ui, |ui| {
                    if advanced {
                        self.toolbar.selection.show_advanced(
                            ui,
                            &mut ToolContext {
                                painter: &mut self.painter,
                                buffer: &mut self.buffer,
                                history: &mut self.history,
                                settings: &mut self.settings,
                                viewport_settings: &mut self.viewport_settings,
                                read_only: self.read_only,
                            },
                        );
                    } else {
                        ui.horizontal(|ui| {
                            for (kind, label) in [
                                (CanvasAction::Copy, "Copy"),
                                (CanvasAction::Delete, "Delete"),
                                (CanvasAction::More, "›"),
                                (CanvasAction::Paste, "Paste"),
                                (CanvasAction::ImportImage, "Import image"),
                                (CanvasAction::SelectAll, "Select all"),
                                (CanvasAction::ZoomToFit, "Zoom to fit elements"),
                            ] {
                                let bit = kind as u32;
                                if spec & bit != 0
                                    && ui
                                        .add_enabled(
                                            spec >> 16 & bit != 0,
                                            egui::Button::new(label),
                                        )
                                        .clicked()
                                {
                                    action = Some(bit);
                                }
                            }
                        });
                    }
                });
            });
        self.context_menu.rect = Some(result.response.rect);
        if let Some(action) = action {
            self.context_action(ui, action);
        }
    }

    /// Used by legacy desktop clipboard delivery as well as native imports.
    pub(super) fn pending_paste_position(&mut self) -> Option<Pos2> {
        self.context_menu
            .pending_insert
            .take()
            .map(|p| map_position(self.viewport_settings.master_transform, p))
    }
}
