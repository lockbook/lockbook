//! Canvas context state and commands belong to Rust. Native hosts only
//! present the quick actions and report the tapped action's bit value.
use egui::{Id, Pos2};
use lb_rs::Uuid;
use resvg::usvg::Transform;

use super::element::BoundedElement;
use super::popover::{self, ActionLabel, SelectionAction};
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

pub(super) struct CanvasContextMenu {
    owner: Uuid,
    native: bool,
    selection: Vec<Uuid>,
    tool: Tool,
    cancel_tool: bool,
    position: Pos2,
    pub(super) rect: Option<egui::Rect>,
    quick: bool,
    advanced: bool,
    pending_insert: Option<Pos2>,
    diagnostic_frames_remaining: u8,
}

impl Default for CanvasContextMenu {
    fn default() -> Self {
        Self {
            owner: Uuid::new_v4(),
            native: false,
            selection: vec![],
            tool: Tool::default(),
            cancel_tool: false,
            position: Pos2::ZERO,
            rect: None,
            quick: false,
            advanced: false,
            pending_insert: None,
            diagnostic_frames_remaining: 0,
        }
    }
}

/// A presentation command, not an action. It cannot collide with action bits.
pub const DISMISS_NATIVE_MENU: u32 = 1 << 31;

#[derive(Clone, Default)]
struct PendingImport {
    owner: Uuid,
    valid: bool,
}

fn import_pending(ctx: &egui::Context) -> bool {
    ctx.data(|d| {
        d.get_temp::<PendingImport>(Id::unique("canvas_pending_import"))
            .is_some()
    })
}

/// Drain completions even when the original document has closed. Only one
/// host operation is outstanding, so stateless native results cannot be
/// mistaken for a later operation in a different document.
pub fn process_host_results(ctx: &egui::Context, mut current: Option<&mut SVGEditor>) {
    let owner = current.as_ref().map(|svg| svg.context_menu.owner);
    ctx.memory_mut(|m| {
        let native = m
            .data
            .get_temp::<Uuid>(Id::unique("canvas_native_menu_owner"));
        if native.is_some() && native != owner {
            m.data
                .remove::<Uuid>(Id::unique("canvas_native_menu_owner"));
            m.data.insert_temp(
                Id::unique("context_menu"),
                (Pos2::ZERO, ContextMenuTarget::Canvas(DISMISS_NATIVE_MENU)),
            );
        }
        if let Some(mut pending) = m
            .data
            .get_temp::<PendingImport>(Id::unique("canvas_pending_import"))
        {
            if Some(pending.owner) != owner {
                if m.data
                    .remove_temp::<u32>(Id::unique("canvas_host_command"))
                    .is_some()
                {
                    m.data
                        .remove::<PendingImport>(Id::unique("canvas_pending_import"));
                } else {
                    pending.valid = false;
                    m.data
                        .insert_temp(Id::unique("canvas_pending_import"), pending);
                }
            }
        }
    });
    for event in ctx.pop_events_where(&mut |e| matches!(e, crate::Event::CanvasContent(_))) {
        let crate::Event::CanvasContent(content) = event else { continue };
        let pending = ctx.memory_mut(|m| {
            m.data
                .remove_temp::<PendingImport>(Id::unique("canvas_pending_import"))
        });
        if let (Some(pending), Some(svg)) = (pending, current.as_deref_mut()) {
            if pending.valid && pending.owner == svg.context_menu.owner && !svg.read_only {
                if let Some(position) = svg.context_menu.pending_insert.take() {
                    let position = map_position(svg.viewport_settings.master_transform, position);
                    let empty = match &content {
                        crate::tab::CanvasPasteContent::Svg(s) => s.is_empty(),
                        crate::tab::CanvasPasteContent::Image(bytes) => bytes.is_empty(),
                    };
                    if !empty {
                        ctx.push_event(crate::Event::CanvasPaste { content, position });
                    }
                }
            }
        }
    }
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
    fn context_actions(&self, ctx: &egui::Context) -> u32 {
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
        if import_pending(ctx) {
            enabled &= !(Paste as u32 | ImportImage as u32);
        }
        if self.read_only {
            enabled &= !(Delete as u32 | Paste as u32 | ImportImage as u32);
        }
        if !self.buffer.elements.values().any(is_selectable) {
            enabled &= !(SelectAll as u32 | ZoomToFit as u32);
        }
        visible | enabled << 16
    }

    pub(super) fn request_context_menu(&mut self, ui: &mut egui::Ui, position: Pos2) {
        self.log_context_menu(ui.ctx(), "open requested");
        self.dismiss_context_menu(ui.ctx(), "replaced by context request");
        self.toolbar
            .close_all_popovers(&mut self.settings, &mut self.cfg);
        self.toolbar.selection.dismiss_popover();
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
        self.context_menu.selection = self.current_context_selection();
        self.context_menu.tool = self.toolbar.active_tool;
        self.present_quick_context_menu(ui.ctx());
    }

    fn present_quick_context_menu(&mut self, ctx: &egui::Context) {
        self.context_menu.advanced = false;
        self.context_menu.rect = None;
        let native =
            matches!(ctx.os(), egui::os::OperatingSystem::Android | egui::os::OperatingSystem::IOS);
        self.context_menu.quick = !native;
        self.context_menu.native = native;
        self.context_menu.diagnostic_frames_remaining = 4;
        self.log_context_menu(ctx, "opened");
        if native {
            ctx.memory_mut(|m| {
                m.data
                    .insert_temp(Id::unique("canvas_native_menu_owner"), self.context_menu.owner)
            });
            ctx.set_context_menu(
                map_position(self.viewport_settings.master_transform, self.context_menu.position),
                ContextMenuTarget::Canvas(self.context_actions(ctx)),
            );
        }
    }

    pub(super) fn process_context_actions(&mut self, ui: &mut egui::Ui) {
        for event in ui.ctx().read_events() {
            match event {
                crate::Event::CanvasAction(action) => self.context_action(ui, action),
                _ => {}
            }
        }
    }

    fn context_action(&mut self, ui: &mut egui::Ui, action: u32) {
        use CanvasAction::*;
        tracing::info!(target: "workspace::canvas_context_menu", action,
            frame = ui.ctx().cumulative_frame_nr(), "canvas menu action received");
        if action == 0 {
            if self.context_menu.native {
                self.dismiss_context_menu(ui.ctx(), "native dismissal callback");
            }
            return;
        }
        if !(self.context_menu.quick || self.context_menu.native) {
            return;
        }
        if action.count_ones() != 1 || (self.context_actions(ui.ctx()) >> 16) & action == 0 {
            return;
        }
        self.dismiss_context_menu(ui.ctx(), "action selected");
        match action {
            a if a == More as u32 => {
                self.context_menu.advanced = true;
                self.toolbar.active_tool = Tool::Selection;
            }
            a if a == Paste as u32 || a == ImportImage as u32 => {
                self.context_menu.pending_insert = Some(self.context_menu.position);
                ui.ctx().memory_mut(|m| {
                    m.data.insert_temp(
                        Id::unique("canvas_pending_import"),
                        PendingImport { owner: self.context_menu.owner, valid: true },
                    )
                });
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
        self.context_menu.selection = self.current_context_selection();
        self.context_menu.tool = self.toolbar.active_tool;
        ui.ctx().request_repaint();
    }

    fn current_context_selection(&self) -> Vec<Uuid> {
        self.toolbar
            .selection
            .selected_elements
            .iter()
            .filter_map(|s| {
                self.buffer
                    .elements
                    .get(&s.id)
                    .filter(|e| is_selectable(e))
                    .map(|_| s.id)
            })
            .collect()
    }

    fn log_context_menu(&self, ctx: &egui::Context, reason: &'static str) {
        tracing::info!(target: "workspace::canvas_context_menu",
            reason, editor = %self.context_menu.owner,
            frame = ctx.cumulative_frame_nr(), pass = ctx.cumulative_pass_nr(),
            quick = self.context_menu.quick, native = self.context_menu.native,
            advanced = self.context_menu.advanced, rect = ?self.context_menu.rect,
            position = ?self.context_menu.position, tool = ?self.toolbar.active_tool,
            selected = self.toolbar.selection.selected_elements.len(),
            selection_contextual = self.toolbar.selection.contextual_menu,
            selection_tooltip = ?self.toolbar.selection.layout.container_tooltip,
            toolbar_popover = self.toolbar.has_visible_popover(),
            "canvas menu lifecycle");
    }

    pub fn dismiss_context_menu(&mut self, ctx: &egui::Context, reason: &'static str) {
        if self.context_menu.quick || self.context_menu.native || self.context_menu.advanced {
            self.log_context_menu(ctx, reason);
        }
        if self.context_menu.native {
            ctx.set_context_menu(Pos2::ZERO, ContextMenuTarget::Canvas(DISMISS_NATIVE_MENU));
        }
        ctx.memory_mut(|m| {
            if m.data
                .get_temp::<Uuid>(Id::unique("canvas_native_menu_owner"))
                == Some(self.context_menu.owner)
            {
                m.data
                    .remove::<Uuid>(Id::unique("canvas_native_menu_owner"));
            }
        });
        self.context_menu.native = false;
        self.context_menu.quick = false;
        self.context_menu.advanced = false;
        self.context_menu.rect = None;
    }

    pub fn deactivate_context_menu(&mut self, ctx: &egui::Context) {
        self.dismiss_context_menu(ctx, "document deactivated");
        self.context_menu.pending_insert = None;
        self.context_menu.cancel_tool = true;
        self.input_controller.reset();
        ctx.memory_mut(|m| {
            if let Some(mut pending) = m
                .data
                .get_temp::<PendingImport>(Id::unique("canvas_pending_import"))
            {
                if pending.owner == self.context_menu.owner {
                    // If the host hasn't taken the command, no completion is owed.
                    if m.data
                        .remove_temp::<u32>(Id::unique("canvas_host_command"))
                        .is_some()
                    {
                        m.data
                            .remove::<PendingImport>(Id::unique("canvas_pending_import"));
                    } else {
                        pending.valid = false;
                        m.data
                            .insert_temp(Id::unique("canvas_pending_import"), pending);
                    }
                }
            }
        });
    }

    fn validate_context_menu(&mut self, ctx: &egui::Context) {
        if (self.context_menu.quick || self.context_menu.advanced || self.context_menu.native)
            && (self.context_menu.selection != self.current_context_selection()
                || self.context_menu.tool != self.toolbar.active_tool
                || self.toolbar.has_visible_popover())
        {
            tracing::info!(target: "workspace::canvas_context_menu",
                selection_changed = self.context_menu.selection != self.current_context_selection(),
                tool_changed = self.context_menu.tool != self.toolbar.active_tool,
                previous_tool = ?self.context_menu.tool,
                toolbar_popover = self.toolbar.has_visible_popover(),
                "canvas menu validation failed");
            self.dismiss_context_menu(ctx, "validation failed");
        }
    }

    pub(super) fn handle_context_menu_input(&mut self, ui: &mut egui::Ui) {
        self.validate_context_menu(ui.ctx());
        if self.context_menu.cancel_tool {
            self.context_menu.cancel_tool = false;
            self.cancel_context_tool(ui);
        }
        if !(self.context_menu.quick || self.context_menu.advanced || self.context_menu.native) {
            return;
        }
        let events = ui.input(|i| i.events.clone());
        for event in &events {
            if matches!(
                event,
                egui::Event::PointerButton { .. }
                    | egui::Event::Touch { .. }
                    | egui::Event::MouseWheel { .. }
                    | egui::Event::Zoom(_)
                    | egui::Event::WindowFocused(_)
            ) {
                tracing::info!(target: "workspace::canvas_context_menu",
                    frame = ui.ctx().cumulative_frame_nr(), pass = ui.ctx().cumulative_pass_nr(),
                    ?event, "canvas menu input while open");
            }
        }
        let outside_press = events.iter().any(|e| match e {
            egui::Event::Touch { phase: egui::TouchPhase::Start, pos, .. }
            | egui::Event::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: true,
                pos,
                ..
            } => !self.context_menu.rect.is_some_and(|r| r.contains(*pos)),
            _ => false,
        });
        // Trackpads can send phase-only Start/End notifications after a
        // secondary click. Those do not move the viewport or dismiss the menu.
        let viewport_change = events.iter().any(|e| match e {
            egui::Event::MouseWheel { delta, .. } => *delta != egui::Vec2::ZERO,
            egui::Event::Zoom(factor) => *factor != 1.0,
            _ => false,
        });
        let focus_lost = events
            .iter()
            .any(|e| matches!(e, egui::Event::WindowFocused(false)));
        let multi_touch = ui.ctx().read_events().iter().any(|e| match e {
            crate::Event::MultiTouchGesture { translation_delta, zoom_factor, .. } => {
                *translation_delta != egui::Vec2::ZERO || *zoom_factor != 1.0
            }
            _ => false,
        });
        let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        // Selection controls remain useful while inspecting the selected
        // elements at another zoom level, just like the regular selection row.
        // Empty-space menus are transient and close when navigating the canvas.
        let selection_menu = !self.context_menu.selection.is_empty();
        let dismiss_for_navigation = !selection_menu && (viewport_change || multi_touch);
        if outside_press || focus_lost || dismiss_for_navigation || escape {
            tracing::info!(target: "workspace::canvas_context_menu",
                outside_press, focus_lost, viewport_change, multi_touch, selection_menu, escape,
                "canvas menu input dismissal");
            for event in ui.ctx().read_events() {
                if let crate::Event::MultiTouchGesture {
                    translation_delta,
                    zoom_factor,
                    rotation_delta,
                    ..
                } = event
                {
                    tracing::info!(target: "workspace::canvas_context_menu",
                        ?translation_delta, zoom_factor, rotation_delta,
                        "canvas menu multitouch input");
                }
            }
            self.dismiss_context_menu(ui.ctx(), "input dismissal");
            self.cancel_context_tool(ui);
            if outside_press {
                self.input_controller.suppress_context_press(&events);
            }
            ui.ctx().request_repaint();
        }
    }

    pub(super) fn suspend_context_menu(&mut self, ui: &mut egui::Ui) {
        if self.context_menu.quick || self.context_menu.native || self.context_menu.advanced {
            tracing::info!(target: "workspace::canvas_context_menu",
                sizing_pass = ui.is_sizing_pass(), opacity = ui.opacity(),
                "canvas menu parent UI disabled");
            self.dismiss_context_menu(ui.ctx(), "canvas UI disabled");
            self.cancel_context_tool(ui);
            self.input_controller.reset();
        }
    }

    fn cancel_context_tool(&mut self, ui: &mut egui::Ui) {
        self.toolbar.active_tool_mut().process_controller_event(
            ui,
            crate::tab::input_controller::InputControllerEvent::ToolCancel,
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

    pub(super) fn show_context_menu(&mut self, ui: &mut egui::Ui) {
        self.validate_context_menu(ui.ctx());
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
        let spec = self.context_actions(ui.ctx());
        let advanced = self.context_menu.advanced;
        let mut expanded = advanced;
        let style = ui.style().clone();
        let diagnose = self.context_menu.diagnostic_frames_remaining > 0;
        let previous_rect = self.context_menu.rect;
        let result = egui::Area::new(Id::unique("canvas_context_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            // Leave one physical pixel for egui's rounded Area position.
            .constrain_to(bounds.shrink(ui.ctx().pixels_per_point().recip()))
            .show(ui.ctx(), |ui| {
                if diagnose {
                    tracing::info!(target: "workspace::canvas_context_menu",
                        frame = ui.ctx().cumulative_frame_nr(), pass = ui.ctx().cumulative_pass_nr(),
                        sizing_pass = ui.is_sizing_pass(), enabled = ui.is_enabled(),
                        opacity = ui.opacity(), requested_position = ?pos, ?bounds,
                        "canvas menu render pass");
                }
                // Areas start from the context style; inherit the canvas style
                // just like the selection popover's child UI does.
                ui.set_style(style);
                popover::apply_style(ui);
                if spec & CanvasAction::Copy as u32 != 0 {
                    let controls = popover::selection_controls(ui, &mut expanded, self.read_only, |ui| {
                        self.toolbar.selection.show_advanced(ui, &mut ToolContext {
                            painter: &mut self.painter,
                            buffer: &mut self.buffer,
                            history: &mut self.history,
                            settings: &mut self.settings,
                            viewport_settings: &mut self.viewport_settings,
                            read_only: self.read_only,
                        });
                    });
                    action = match controls.action {
                        Some(SelectionAction::Copy) => Some(CanvasAction::Copy as u32),
                        Some(SelectionAction::Delete) => Some(CanvasAction::Delete as u32),
                        _ => None,
                    };
                } else {
                    action = popover::action_row(ui, |ui| {
                        let actions = [
                            (CanvasAction::Paste, ActionLabel::Text("Paste")),
                            (CanvasAction::ImportImage, ActionLabel::Text("Import image")),
                            (CanvasAction::SelectAll, ActionLabel::Text("Select all")),
                            (CanvasAction::ZoomToFit, ActionLabel::Text("Zoom to fit elements")),
                        ]
                        .into_iter()
                        .filter_map(|(kind, label)| {
                            let bit = kind as u32;
                            (spec & bit != 0).then_some((bit, label, spec >> 16 & bit != 0))
                        });
                        popover::action_buttons(ui, actions)
                    })
                    .inner
                    .inner;
                }
            });
        self.context_menu.rect = Some(result.response.rect);
        if diagnose {
            self.context_menu.diagnostic_frames_remaining -= 1;
            tracing::info!(target: "workspace::canvas_context_menu",
                frame = ui.ctx().cumulative_frame_nr(), ?previous_rect,
                rect = ?result.response.rect, "canvas menu rendered bounds");
        }
        if expanded != advanced {
            if expanded {
                self.context_menu.quick = false;
                self.context_menu.advanced = true;
            } else {
                self.present_quick_context_menu(ui.ctx());
            }
            ui.ctx().request_repaint();
        }
        if let Some(action) = action {
            self.context_action(ui, action);
        }
    }
}

#[cfg(test)]
mod tests;
