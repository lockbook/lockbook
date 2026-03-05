use bezier_rs::Subpath;
use lb_rs::model::svg::{
    diff::DiffState,
    element::{Element, Path, Stroke},
};
use resvg::usvg::Transform;

use crate::{
    tab::svg_editor::{
        Event, InsertElement, roger::RogerEvent, toolbar::ToolContext, util::pos_to_dvec,
    },
    theme::icons::Icon,
};

#[derive(Default)]
pub struct ShapesTool {
    pub active_shape: ShapeType,
    build_origin: Option<egui::Pos2>,
    current_id: Option<lb_rs::Uuid>,
    is_building: bool,
    pub active_stroke: Stroke,
}

#[derive(Default, PartialEq, Clone, Copy)]
pub enum ShapeType {
    Rectangle,
    Circle,
    #[default]
    Line,
}

impl ShapeType {
    pub fn icon(&self) -> Icon {
        match self {
            ShapeType::Rectangle => Icon::RECTANGLE,
            ShapeType::Circle => Icon::CIRCLE,
            ShapeType::Line => Icon::LINE,
        }
    }
}
#[derive(PartialEq, Debug)]
pub enum ShapeEvent {
    Build(egui::Pos2),
    End,
    Cancel,
}

pub fn from_roger_to_shape_event(event: RogerEvent) -> Option<ShapeEvent> {
    match event {
        RogerEvent::ToolStart(payload) | RogerEvent::ToolRun(payload) => {
            Some(ShapeEvent::Build(payload.pos))
        }
        RogerEvent::ToolEnd(_) => Some(ShapeEvent::End),
        RogerEvent::ToolCancel => Some(ShapeEvent::Cancel),
        _ => None,
    }
}

impl ShapesTool {
    pub fn handle_shape_event(&mut self, event: &ShapeEvent, shapes_ctx: &mut ToolContext) {
        match event {
            ShapeEvent::End => {
                if let Some(current_id) = self.current_id {
                    shapes_ctx
                        .history
                        .save(Event::Insert(vec![InsertElement { id: current_id }]));
                }
                self.reset_build();
            }
            ShapeEvent::Cancel => {
                self.reset_build();
            }
            ShapeEvent::Build(pos) => {
                if self.build_origin.is_none() {
                    self.build_origin = Some(*pos);
                }

                if self.current_id.is_none() {
                    self.current_id = Some(lb_rs::Uuid::new_v4());
                }
                let build_origin = self.build_origin.unwrap();
                let current_id = self.current_id.unwrap();
                self.is_building = true;

                if let Some(Element::Path(p)) = shapes_ctx.buffer.elements.get_mut(&current_id) {
                    p.diff_state.data_changed = true;
                    let anchor1 = pos_to_dvec(build_origin);
                    let anchor2 = pos_to_dvec(*pos);

                    p.data = match self.active_shape {
                        ShapeType::Rectangle => Subpath::new_rect(anchor1, anchor2),
                        ShapeType::Circle => {
                            Subpath::new_ellipse(anchor2.min(anchor1), anchor2.max(anchor1))
                        }
                        ShapeType::Line => Subpath::new_line(anchor1, anchor2),
                    };

                    // change path data here
                } else {
                    // todo: add stroke ui controls

                    let el = Element::Path(Path {
                        data: Subpath::new(vec![], false),
                        visibility: resvg::usvg::Visibility::Visible,
                        fill: None,
                        stroke: Some(self.active_stroke),
                        transform: Transform::identity(),
                        opacity: 1.0,
                        diff_state: DiffState::default(),
                        deleted: false,
                    });

                    shapes_ctx.buffer.elements.insert_before(0, current_id, el);
                }
            }
        }
    }

    fn reset_build(&mut self) {
        self.build_origin = None;
        self.is_building = false;
        self.current_id = None;
    }
}
