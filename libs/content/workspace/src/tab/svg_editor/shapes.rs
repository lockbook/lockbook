use std::time::{Duration, Instant};

use bezier_rs::Subpath;
use lb_rs::model::svg::{
    diff::DiffState,
    element::{Element, Path, Stroke},
};
use resvg::usvg::Transform;
use tracing::error;

use crate::{
    tab::svg_editor::{
        InsertElement,
        toolbar::ToolContext,
        util::{is_multi_touch, pos_to_dvec},
    },
    theme::icons::Icon,
};

#[derive(Default)]
pub struct ShapesTool {
    pub active_shape: ShapeType,
    build_touch_id: Option<egui::TouchId>,
    first_build_frame: Option<std::time::Instant>,
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
enum ShapeEvent {
    Build((egui::Pos2, Option<egui::TouchId>)),
    End,
    Cancel,
}

impl ShapesTool {
    pub fn handle_input(&mut self, ui: &mut egui::Ui, shapes_ctx: &mut ToolContext) {
        let is_multi_touch = is_multi_touch(ui);

        ui.input(|r| {
            for e in r.events.iter() {
                if let Some(shape_event) = self.map_ui_event(e, shapes_ctx, is_multi_touch) {
                    error!("{:?}", shape_event);
                    self.handle_shape_event(&shape_event, shapes_ctx);
                    if shape_event == ShapeEvent::Cancel || shape_event == ShapeEvent::End {
                        break;
                    }
                }
            }
        });

        *shapes_ctx.allow_viewport_changes = !self.is_building;
    }

    fn map_ui_event(
        &self, event: &egui::Event, shapes_ctx: &mut ToolContext, is_multi_touch: bool,
    ) -> Option<ShapeEvent> {
        match *event {
            egui::Event::PointerMoved(pos) => {
                // if you have a mouse and finger on the screen then there's
                // a touch event along with pointer moved event. disregard
                // the mouse event in this case.
                if self.is_building && !shapes_ctx.is_touch_frame {
                    return Some(ShapeEvent::Build((pos, None)));
                }
            }
            egui::Event::PointerButton { pos, button, pressed, modifiers: _ } => {
                if button != egui::PointerButton::Primary {
                    return None;
                }

                // the integration sends both a touch event and pointer event,
                // let's start building in the touch event so that we can
                // register the touch id that starts the shape.
                if shapes_ctx.is_touch_frame {
                    return None;
                }

                return if pressed {
                    Some(ShapeEvent::Build((pos, None)))
                } else {
                    Some(ShapeEvent::End)
                };
            }
            egui::Event::Touch { device_id: _, id, phase, pos, force } => {
                if phase == egui::TouchPhase::Cancel {
                    return Some(ShapeEvent::Cancel);
                }

                match phase {
                    egui::TouchPhase::Start | egui::TouchPhase::Move => {
                        if let Some(first_build) = self.first_build_frame {
                            // multpile finger touches within 500ms cancels shape building
                            if is_multi_touch
                                && force.is_none()
                                && !shapes_ctx.settings.pencil_only_drawing
                                && Instant::now() - first_build < Duration::from_millis(500)
                            {
                                return Some(ShapeEvent::Cancel);
                            }
                        }

                        if shapes_ctx.settings.pencil_only_drawing && force.is_some() {
                            return Some(ShapeEvent::Build((pos, Some(id))));
                        }
                        if !shapes_ctx.settings.pencil_only_drawing && !is_multi_touch {
                            return Some(ShapeEvent::Build((pos, Some(id))));
                        }
                    }
                    egui::TouchPhase::End => {
                        if let Some(touch_id) = self.build_touch_id {
                            if touch_id == id {
                                return Some(ShapeEvent::End);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        None
    }

    fn handle_shape_event(&mut self, event: &ShapeEvent, shapes_ctx: &mut ToolContext) {
        match event {
            ShapeEvent::End => {
                if let Some(current_id) = self.current_id {
                    shapes_ctx
                        .history
                        .save(super::Event::Insert(vec![InsertElement { id: current_id }]));
                }
                self.reset_build();
            }
            ShapeEvent::Cancel => {
                self.reset_build();
            }
            ShapeEvent::Build((pos, touch_id)) => {
                if self.build_origin.is_none() {
                    self.build_origin = Some(*pos);
                }
                if self.build_touch_id.is_none() {
                    self.build_touch_id = *touch_id;
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
        self.build_touch_id = None;
        self.current_id = None;
    }
}
