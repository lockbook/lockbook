use std::collections::HashMap;
use web_time::{Duration, Instant};

use lb_rs::Uuid;
use lb_rs::model::svg::element::Element;

use super::DeleteElement;
use super::toolbar::ToolContext;
use super::util::{is_multi_touch, pointer_intersects_element};

pub struct Eraser {
    pub radius: f32,
    delete_candidates: HashMap<Uuid, f32>,
    last_pos: Option<egui::Pos2>,
    is_building: bool,
    build_touch_id: Option<egui::TouchId>,
    first_build_frame: Option<web_time::Instant>,
}

impl Default for Eraser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PartialEq, Debug)]
pub enum EraseEvent {
    Build((egui::Pos2, Option<egui::TouchId>)),
    End,
    Cancel,
}

pub const DEFAULT_ERASER_RADIUS: f32 = 5.0;

impl Eraser {
    pub fn new() -> Self {
        Eraser {
            delete_candidates: HashMap::default(),
            radius: DEFAULT_ERASER_RADIUS,
            last_pos: None,
            is_building: false,
            build_touch_id: None,
            first_build_frame: None,
        }
    }

    pub fn handle_input(&mut self, ui: &mut egui::Ui, eraser_ctx: &mut ToolContext) {
        if eraser_ctx.toolbar_has_interaction {
            return;
        }
        let is_multi_touch = is_multi_touch(ui);

        ui.input(|r| {
            for e in r.events.iter() {
                if let Some(erase_event) = self.map_ui_event(e, eraser_ctx, is_multi_touch) {
                    self.handle_erase_event(&erase_event, eraser_ctx);
                    if erase_event == EraseEvent::Cancel || erase_event == EraseEvent::End {
                        break;
                    }
                }
            }
        });

        // gotta set this to true every frame, else you can't pan and zoom
        *eraser_ctx.allow_viewport_changes = !self.is_building;

        if let Some(pos) = ui.input(|r| r.pointer.hover_pos()) {
            self.draw_elevated_eraser_cursor(ui, eraser_ctx.painter, pos);
        }
    }

    pub fn map_ui_event(
        &mut self, event: &egui::Event, eraser_ctx: &mut ToolContext, is_multi_touch: bool,
    ) -> Option<EraseEvent> {
        match *event {
            egui::Event::PointerMoved(pos) => {
                if self.is_building && !eraser_ctx.is_touch_frame {
                    return Some(EraseEvent::Build((pos, None)));
                }
            }
            egui::Event::PointerButton { pos, button, pressed, modifiers: _ } => {
                if button != egui::PointerButton::Primary {
                    return None;
                }

                if eraser_ctx.is_touch_frame {
                    return None;
                }

                return if pressed {
                    Some(EraseEvent::Build((pos, None)))
                } else {
                    Some(EraseEvent::End)
                };
            }
            egui::Event::Touch { device_id: _, id, phase, pos, force } => {
                if phase == egui::TouchPhase::Cancel {
                    return Some(EraseEvent::Cancel);
                }

                match phase {
                    egui::TouchPhase::Start | egui::TouchPhase::Move => {
                        if let Some(first_build) = self.first_build_frame {
                            if is_multi_touch
                                && force.is_none()
                                && !eraser_ctx.settings.pencil_only_drawing
                                && Instant::now() - first_build < Duration::from_millis(500)
                            {
                                return Some(EraseEvent::Cancel);
                            }
                        }

                        if eraser_ctx.settings.pencil_only_drawing && force.is_some() {
                            return Some(EraseEvent::Build((pos, Some(id))));
                        }
                        if !eraser_ctx.settings.pencil_only_drawing && !is_multi_touch {
                            return Some(EraseEvent::Build((pos, Some(id))));
                        }
                    }
                    egui::TouchPhase::End => {
                        if let Some(touch_id) = self.build_touch_id {
                            if touch_id == id {
                                return Some(EraseEvent::End);
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

    fn handle_erase_event(&mut self, event: &EraseEvent, eraser_ctx: &mut ToolContext<'_>) {
        match *event {
            EraseEvent::Build((pos, maybe_touch_id)) => {
                if !eraser_ctx.painter.clip_rect().contains(pos) {
                    return;
                }
                self.build_touch_id = maybe_touch_id;
                self.is_building = true;

                if self.first_build_frame.is_none() {
                    self.first_build_frame = Some(Instant::now());
                }

                eraser_ctx
                    .buffer
                    .elements
                    .iter()
                    .filter(|(_, el)| !el.deleted())
                    .for_each(|(id, el)| {
                        if self.delete_candidates.contains_key(id) {
                            return;
                        }
                        if pointer_intersects_element(el, pos, self.last_pos, self.radius as f64) {
                            self.delete_candidates.insert(*id, el.opacity());
                        }
                    });

                self.delete_candidates.iter().for_each(|(id, &opacity)| {
                    if let Some(Element::Path(p)) = eraser_ctx.buffer.elements.get_mut(id) {
                        if opacity == p.opacity {
                            p.opacity *= 0.3;
                            p.diff_state.opacity_changed = true
                        }
                    };
                });

                self.last_pos = Some(pos);
            }
            EraseEvent::End => {
                self.is_building = false;

                if self.delete_candidates.is_empty() {
                    return;
                }

                self.delete_candidates.iter().for_each(|(id, &opacity)| {
                    if let Some(Element::Path(p)) = eraser_ctx.buffer.elements.get_mut(id) {
                        p.opacity = opacity;
                        p.deleted = true;
                        p.diff_state.delete_changed = true;
                    };
                });
                let event = super::Event::Delete(
                    self.delete_candidates
                        .keys()
                        .map(|id| DeleteElement { id: *id })
                        .collect(),
                );

                eraser_ctx.history.save(event);

                self.first_build_frame = None;
                self.delete_candidates.clear();
                self.last_pos = None;
            }
            EraseEvent::Cancel => {
                self.delete_candidates.iter().for_each(|(id, &opacity)| {
                    if let Some(Element::Path(p)) = eraser_ctx.buffer.elements.get_mut(id) {
                        p.opacity = opacity;
                        p.diff_state.opacity_changed = true;
                    };
                });

                self.first_build_frame = None;
                self.delete_candidates.clear();
                self.last_pos = None;
                self.is_building = false;
            }
        }
    }

    fn draw_elevated_eraser_cursor(
        &mut self, ui: &mut egui::Ui, painter: &mut egui::Painter, pos: egui::Pos2,
    ) {
        let old_layer = painter.layer_id();
        painter.set_layer_id(egui::LayerId {
            order: egui::Order::PanelResizeLine,
            id: "eraser_overlay".into(),
        });

        self.draw_eraser_cursor(ui, painter, pos);

        painter.set_layer_id(old_layer);
    }

    pub fn draw_eraser_cursor(
        &mut self, ui: &mut egui::Ui, painter: &egui::Painter, cursor_pos: egui::Pos2,
    ) {
        let stroke = egui::Stroke { width: 1.0, color: ui.visuals().text_color() };
        painter.circle_stroke(cursor_pos, self.radius, stroke);

        // todo: apple integration doesn't support this correctly.
        // ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::None);
    }
}
