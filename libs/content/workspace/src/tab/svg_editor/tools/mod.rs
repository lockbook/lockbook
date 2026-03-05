use crate::tab::svg_editor::{roger::RogerEvent, toolbar::ToolContext};

pub mod eraser;
mod path_builder;
pub mod pen;
pub mod selection;
pub mod shapes;

pub trait RogerTool {
    type ToolEvent;
    fn roger_to_tool_event(&self, roger_event: RogerEvent) -> Option<Self::ToolEvent>;
    fn handle_tool_event(
        &mut self, ui: &mut egui::Ui, event: Self::ToolEvent, ctx: &mut ToolContext,
    );
    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>);

    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext);
}

// Object-safe version that erases the event type
pub trait DynRogerTool {
    fn process_roger_event(&mut self, ui: &mut egui::Ui, event: RogerEvent, ctx: &mut ToolContext);
    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>);
    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext);
}

impl<T: RogerTool> DynRogerTool for T {
    fn process_roger_event(&mut self, ui: &mut egui::Ui, event: RogerEvent, ctx: &mut ToolContext) {
        if let Some(ev) = self.roger_to_tool_event(event) {
            self.handle_tool_event(ui, ev, ctx);
        }
    }

    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>) {
        RogerTool::show_hover_point(self, ui, pos, ctx);
    }

    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext) {
        RogerTool::show_tool_ui(self, ui, ctx);
    }
}
