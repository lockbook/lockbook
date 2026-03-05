use crate::tab::svg_editor::{roger::RogerEvent, toolbar::ToolContext};

pub mod eraser;
mod path_builder;
pub mod pen;
pub mod selection;
pub mod shapes;

pub trait RogerTool {
    type ToolEvent;
    fn roger_to_tool_event(&self, roger_event: RogerEvent) -> Option<Self::ToolEvent>;
    fn handle_tool_event(&mut self, event: Self::ToolEvent, ctx: &mut ToolContext);
    fn show_hover_point(&self, ui: &mut egui::Ui, pos: egui::Pos2, ctx: &mut ToolContext<'_>);

    fn show_tool_ui(&mut self, ui: &mut egui::Ui, ctx: &mut ToolContext);
}
