use crate::tab::input_controller::InputControllerEvent;
use crate::tab::svg_editor::CanvasAction;
use crate::tab::svg_editor::toolbar::ToolContext;

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
    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>);

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
        InputControllerTool::show_hover_point(self, ui, pos, ctx);
    }

    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext) {
        InputControllerTool::show_tool_ui(self, ui, ctx);
    }
}
