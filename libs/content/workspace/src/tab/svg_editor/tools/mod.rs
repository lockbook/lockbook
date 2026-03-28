use crate::tab::svg_editor::{input_controller::InputControllerEvent, toolbar::ToolContext};

pub mod eraser;
mod path_builder;
pub mod pen;
pub mod selection;
pub mod shapes;

pub trait InputControllerTool {
    type ToolEvent;
    fn controller_event_to_tool_event(
        &self, controller_event: InputControllerEvent,
    ) -> Option<Self::ToolEvent>;
    fn handle_tool_event(
        &mut self, ui: &mut egui::Ui, event: Self::ToolEvent, ctx: &mut ToolContext,
    );
    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>);

    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext);
}

// Object-safe version that erases the event type
pub trait DynInputControllerTool {
    fn process_controller_event(
        &mut self, ui: &mut egui::Ui, event: InputControllerEvent, ctx: &mut ToolContext,
    );
    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>);
    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext);
}

impl<T: InputControllerTool> DynInputControllerTool for T {
    fn process_controller_event(
        &mut self, ui: &mut egui::Ui, event: InputControllerEvent, ctx: &mut ToolContext,
    ) {
        if let Some(ev) = self.controller_event_to_tool_event(event) {
            self.handle_tool_event(ui, ev, ctx);
        }
    }

    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>) {
        InputControllerTool::show_hover_point(self, ui, pos, ctx);
    }

    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext) {
        InputControllerTool::show_tool_ui(self, ui, ctx);
    }
}
