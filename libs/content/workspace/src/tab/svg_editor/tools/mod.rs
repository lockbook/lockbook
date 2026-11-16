use super::coordinates::{CanvasCoordinates, DocumentPoint, ScreenPoint};
use crate::tab::input_controller::{InputControllerEvent, ToolPayload};
use crate::tab::svg_editor::CanvasAction;
use crate::tab::svg_editor::toolbar::ToolContext;

/// Screen-space actions are normalized at execution using the current viewport.
/// This keeps coordinates fresh after preceding viewport actions in the same frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPointer {
    pub document_pos: DocumentPoint,
    pub screen_pos: ScreenPoint,
    pub force: Option<f32>,
    pub id: Option<egui::TouchId>,
}

impl CanvasPointer {
    fn new(payload: ToolPayload, coordinates: CanvasCoordinates) -> Self {
        let screen_pos = ScreenPoint(payload.pos);
        Self {
            document_pos: coordinates.document_point(screen_pos),
            screen_pos,
            force: payload.force,
            id: payload.id,
        }
    }
}

pub mod eraser;
mod path_builder;
pub mod pen;
pub mod selection;
pub mod shapes;

pub trait InputControllerTool {
    type ToolEvent;
    fn controller_event_to_tool_event(
        &self, controller_event: InputControllerEvent<CanvasAction>,
    ) -> Option<CanvasAction>;
    fn handle_tool_event(
        &mut self, ui: &mut egui::Ui, event: Self::ToolEvent, ctx: &mut ToolContext,
    );
    fn show_hover_point(
        &self, ui: &mut egui::Ui, pointer: CanvasPointer, ctx: &mut ToolContext<'_>,
    );

    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext);
}

// Object-safe input conversion and tool UI; execution stays in the canvas.
pub trait DynInputControllerTool {
    fn controller_event_to_tool_event(
        &self, event: InputControllerEvent<CanvasAction>,
    ) -> Option<CanvasAction>;
    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>);
    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext);
}

impl<T: InputControllerTool> DynInputControllerTool for T {
    fn controller_event_to_tool_event(
        &self, event: InputControllerEvent<CanvasAction>,
    ) -> Option<CanvasAction> {
        InputControllerTool::controller_event_to_tool_event(self, event)
    }

    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>) {
        let pointer = CanvasPointer::new(
            ToolPayload { pos, force: None, id: None },
            ctx.viewport_settings.coordinates(),
        );
        InputControllerTool::show_hover_point(self, ui, pointer, ctx);
    }

    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext) {
        InputControllerTool::show_tool_ui(self, ui, ctx);
    }
}
