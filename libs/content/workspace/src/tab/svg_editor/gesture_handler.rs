use std::collections::HashMap;

use resvg::usvg::Transform;

use crate::tab::ExtendedInput as _;
use crate::tab::svg_editor::toolbar::{MINI_MAP_WIDTH, Toolbar};
use crate::tab::svg_editor::util::get_pan;

use super::element::BoundedElement;
use super::util::transform_rect;
use super::{SVGEditor, ViewportSettings};
use lb_rs::model::svg::buffer::u_transform_to_bezier;
use lb_rs::model::svg::element::Element;

use super::Buffer;
use super::toolbar::ToolContext;
pub const MIN_ZOOM_LEVEL: f32 = 0.1;

#[derive(Default)]
pub struct GestureHandler {
    pub is_zoom_locked: bool,
    pub is_pan_x_locked: bool,
    pub is_pan_y_locked: bool,
}

impl GestureHandler {
    pub fn handle_input(
        &mut self, ui: &mut egui::Ui, gesture_ctx: &mut ToolContext, hide_overlay: bool,
    ) {
        for e in ui.ctx().read_events() {
            match e {
                crate::Event::Undo => gesture_ctx.history.undo(gesture_ctx.buffer),
                crate::Event::Redo => gesture_ctx.history.redo(gesture_ctx.buffer),
                _ => {}
            };
        }
        self.change_viewport(ui, gesture_ctx, hide_overlay);
    }

    fn change_viewport(
        &mut self, ui: &mut egui::Ui, gesture_ctx: &mut ToolContext<'_>, hide_overlay: bool,
    ) {
        let zoom_delta = ui.input(|r| r.zoom_delta());
        let is_zooming = zoom_delta != 1.0;
        let pan: Option<egui::Vec2> = get_pan(ui, gesture_ctx.settings.pencil_only_drawing);

        let touch_positions = SVGEditor::get_touch_positions(ui);
        let pos_cardinality = touch_positions.len();
        let mut sum_pos = egui::Pos2::default();
        for pos in SVGEditor::get_touch_positions(ui).values() {
            sum_pos.x += pos.x;
            sum_pos.y += pos.y;
        }

        let maybe_pos = if pos_cardinality != 0 {
            Some(sum_pos / pos_cardinality as f32)
        } else {
            ui.ctx().pointer_hover_pos()
        };

        let container_rect_with_mini_map = if Toolbar::should_show_mini_map(
            hide_overlay,
            gesture_ctx.settings,
            gesture_ctx.viewport_settings,
        ) {
            egui::Rect::from_min_size(
                gesture_ctx.viewport_settings.container_rect.min,
                egui::vec2(
                    gesture_ctx.viewport_settings.container_rect.width() - MINI_MAP_WIDTH,
                    gesture_ctx.viewport_settings.container_rect.height(),
                ),
            )
        } else {
            gesture_ctx.viewport_settings.container_rect
        };

        if maybe_pos.is_some() && !container_rect_with_mini_map.contains(maybe_pos.unwrap()) {
            return;
        }

        let mut t = Transform::identity();
        if let Some(p) = pan {
            t = t.post_translate(
                if !self.is_pan_x_locked { p.x } else { 0.0 },
                if !self.is_pan_y_locked { p.y } else { 0.0 },
            );
        }
        if is_zooming && !self.is_zoom_locked {
            // apply zoom
            t = t.post_scale(zoom_delta, zoom_delta);

            // correct the zoom to center
            if let Some(pos) = maybe_pos {
                t = t.post_translate((1.0 - zoom_delta) * pos.x, (1.0 - zoom_delta) * pos.y);
            }
        }

        if pan.is_some() || is_zooming {
            transform_canvas(gesture_ctx.buffer, gesture_ctx.viewport_settings, t);
        }
    }
}

pub fn transform_canvas(
    buffer: &mut Buffer, viewport_settings: &mut ViewportSettings, t: Transform,
) {
    let new_transform = viewport_settings.master_transform.post_concat(t);

    // max allowed zoom level is 10%
    if viewport_settings.master_transform.sx < MIN_ZOOM_LEVEL
        && new_transform.sx < viewport_settings.master_transform.sx
    {
        return;
    }
    if new_transform.sx == 0.0 || new_transform.sy == 0.0 {
        return;
    }
    viewport_settings.master_transform = new_transform;
    buffer.master_transform_changed = true;

    for el in buffer.elements.values_mut() {
        match el {
            Element::Path(path) => {
                path.diff_state.transformed = Some(t);
                path.data.apply_transform(u_transform_to_bezier(&t));
            }
            Element::Image(image) => {
                if let Some(new_vbox) = image.view_box.transform(t) {
                    image.view_box = new_vbox;
                }
                image.diff_state.transformed = Some(t);
            }
            Element::Text(_) => todo!(),
        }
    }
    viewport_settings.bounded_rect = viewport_settings
        .bounded_rect
        .map(|rect| transform_rect(rect, t));
}

/// returns the fit transform in the non master transform plane
pub fn get_zoom_fit_transform(viewport_settings: &ViewportSettings) -> Option<Transform> {
    let elements_bound = viewport_settings.bounded_rect?;

    get_rect_identity_transform(
        viewport_settings.container_rect,
        elements_bound,
        0.7,
        viewport_settings.container_rect.center(),
    )
}

/// given two rects how to transform them such that they're both equal
pub fn get_rect_identity_transform(
    origin: egui::Rect, source: egui::Rect, padding_coeff: f32, anchor: egui::Pos2,
) -> Option<Transform> {
    let is_width_smaller = source.width() < source.height();
    let zoom_delta = if is_width_smaller {
        origin.height() * padding_coeff / source.height()
    } else {
        origin.width() * padding_coeff / source.width()
    };
    let center_x = anchor.x - zoom_delta * (source.left() + source.width() / 2.0);
    let center_y = anchor.y - zoom_delta * (source.top() + source.height() / 2.0);
    Some(
        Transform::identity()
            .post_scale(zoom_delta, zoom_delta)
            .post_translate(center_x, center_y),
    )
}

/// result is in absolute plane
pub fn calc_elements_bounds(buffer: &Buffer) -> Option<egui::Rect> {
    let mut elements_bound =
        egui::Rect { min: egui::pos2(f32::MAX, f32::MAX), max: egui::pos2(f32::MIN, f32::MIN) };
    let mut dirty_bound = false;
    for (_, el) in buffer.elements.iter() {
        if el.deleted() {
            continue;
        }

        let el_rect = el.bounding_box();
        dirty_bound = true;

        elements_bound.min.x = elements_bound.min.x.min(el_rect.min.x);
        elements_bound.min.y = elements_bound.min.y.min(el_rect.min.y);

        elements_bound.max.x = elements_bound.max.x.max(el_rect.max.x);
        elements_bound.max.y = elements_bound.max.y.max(el_rect.max.y);
    }
    if !dirty_bound { None } else { Some(elements_bound) }
}

pub fn zoom_percentage_to_transform(
    zoom_percentage: f32, viewport_settings: &ViewportSettings, ui: &mut egui::Ui,
) -> Transform {
    let zoom_delta = (zoom_percentage) / (viewport_settings.master_transform.sx * 100.0);
    Transform::identity()
        .post_scale(zoom_delta, zoom_delta)
        .post_translate(
            (1.0 - zoom_delta) * ui.ctx().screen_rect().center().x,
            (1.0 - zoom_delta) * ui.ctx().screen_rect().center().y,
        )
}

impl SVGEditor {
    pub fn get_touch_positions(ui: &mut egui::Ui) -> HashMap<u64, egui::Pos2> {
        ui.input(|r| {
            let mut touch_positions = HashMap::new();
            for e in r.events.iter() {
                if let egui::Event::Touch { device_id: _, id, phase, pos, force: _ } = *e {
                    if phase != egui::TouchPhase::Cancel {
                        touch_positions.insert(id.0, pos);
                    }
                }
            }

            touch_positions
        })
    }
}
