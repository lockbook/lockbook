use crate::tab::input_controller::{InputControllerEvent, ToolPayload};
use crate::tab::svg_editor::coordinates::{CanvasCoordinates, DocumentPoint, ScreenPoint};
use crate::tab::svg_editor::toolbar::ToolContext;

/// Canvas tools receive both coordinate spaces at their input boundary.
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CanvasToolEvent {
    Start(CanvasPointer),
    Run(CanvasPointer),
    PredictedRun(CanvasPointer),
    End(CanvasPointer),
    Hover(CanvasPointer),
    Cancel,
}

impl CanvasToolEvent {
    /// Normalize at dispatch time, after any preceding viewport events.
    fn from_controller(
        event: InputControllerEvent, coordinates: CanvasCoordinates,
    ) -> Option<Self> {
        Some(match event {
            InputControllerEvent::ToolStart(payload) => {
                Self::Start(CanvasPointer::new(payload, coordinates))
            }
            InputControllerEvent::ToolRun(payload) => {
                Self::Run(CanvasPointer::new(payload, coordinates))
            }
            InputControllerEvent::ToolEnd(payload) => {
                Self::End(CanvasPointer::new(payload, coordinates))
            }
            InputControllerEvent::ToolHover(payload) => {
                Self::Hover(CanvasPointer::new(payload, coordinates))
            }
            InputControllerEvent::ToolPredictedRun(pos, force) => Self::PredictedRun(
                CanvasPointer::new(ToolPayload { pos, force, id: None }, coordinates),
            ),
            InputControllerEvent::ToolCancel
            | InputControllerEvent::ViewportChangeWithToolCancel => Self::Cancel,
            InputControllerEvent::ViewportChange(_) | InputControllerEvent::Gesture(_) => {
                return None;
            }
        })
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
        &self, controller_event: CanvasToolEvent,
    ) -> Option<Self::ToolEvent>;
    fn handle_tool_event(
        &mut self, ui: &mut egui::Ui, event: Self::ToolEvent, ctx: &mut ToolContext,
    );
    fn show_hover_point(
        &self, ui: &mut egui::Ui, pointer: CanvasPointer, ctx: &mut ToolContext<'_>,
    );

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
        let Some(event) =
            CanvasToolEvent::from_controller(event, ctx.viewport_settings.coordinates())
        else {
            return;
        };
        if let Some(ev) = self.controller_event_to_tool_event(event) {
            self.handle_tool_event(ui, ev, ctx);
        }
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
