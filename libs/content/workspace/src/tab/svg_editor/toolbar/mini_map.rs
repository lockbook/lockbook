use resvg::usvg::Transform;

use crate::tab::svg_editor::gesture_handler::transform_canvas;
use crate::tab::svg_editor::renderer::RenderOptions;
use crate::tab::svg_editor::toolbar::{MINI_MAP_WIDTH, Toolbar, ToolbarContext};
use crate::tab::svg_editor::util::{get_pan, transform_rect};
use crate::tab::svg_editor::{CanvasSettings, ViewportSettings};
const SCROLLBAR_WIDTH: f32 = 15.0;

impl Toolbar {
    pub fn should_show_mini_map(
        hide_overlay: bool, settings: &CanvasSettings, viewport_settings: &ViewportSettings,
    ) -> bool {
        !hide_overlay
            && settings.show_mini_map
            && viewport_settings.is_scroll_mode()
            && viewport_settings.bounded_rect.is_some()
    }

    pub fn show_mini_map(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> (bool, Option<egui::Response>) {
        if !Self::should_show_mini_map(
            self.hide_overlay,
            tlbr_ctx.settings,
            tlbr_ctx.viewport_settings,
        ) {
            return (false, None);
        }
        let bounded_rect = tlbr_ctx.viewport_settings.bounded_rect.unwrap();

        let mini_map_size =
            egui::vec2(MINI_MAP_WIDTH, tlbr_ctx.viewport_settings.container_rect.height());

        let mini_map_rect = egui::Rect::from_min_size(
            tlbr_ctx.viewport_settings.container_rect.right_top()
                - egui::vec2(mini_map_size.x, 0.0),
            mini_map_size,
        );

        self.layout.mini_map = Some(mini_map_rect);

        let shadow: egui::Shape = egui::Shadow {
            offset: egui::vec2(0.0, 0.0),
            blur: 40.0,
            spread: 0.0,
            color: ui.visuals().window_shadow.color,
        }
        .as_shape(mini_map_rect, 0.0)
        .into();

        let mini_map_line_sep = egui::Shape::line_segment(
            [mini_map_rect.left_top(), mini_map_rect.left_bottom()],
            egui::Stroke { width: 1., color: ui.visuals().window_stroke.color },
        );
        let scroll_bar_line_sep = egui::Shape::line_segment(
            [mini_map_rect.right_top(), mini_map_rect.right_bottom()],
            egui::Stroke { width: 0.5, color: ui.visuals().window_stroke.color },
        );

        let mut painter = ui.painter().clone();
        painter.set_clip_rect(mini_map_rect);

        let invert_master_transform = tlbr_ctx
            .viewport_settings
            .master_transform
            .invert()
            .unwrap();

        let bounded_rect = transform_rect(bounded_rect, invert_master_transform);
        // make sure the min of bounded_rect starts at (0,0)
        let bounded_rect_offset = egui::vec2(-bounded_rect.left(), -bounded_rect.top());

        let bounded_rect = bounded_rect.translate(bounded_rect_offset);
        let container_rect =
            transform_rect(tlbr_ctx.viewport_settings.container_rect, invert_master_transform)
                .translate(bounded_rect_offset);

        let s = mini_map_rect.width() / bounded_rect.width();

        let mini_map_full_height = bounded_rect.height() * s;
        let mini_map_off_screen_height = (mini_map_full_height - mini_map_rect.height()).max(0.0);
        let scroll_percentage =
            container_rect.top() / (bounded_rect.height() - container_rect.height());
        let offset = mini_map_off_screen_height * scroll_percentage;

        let viewport_transform = Transform::identity().post_scale(s, s).post_translate(
            mini_map_rect.left() - s * bounded_rect.left() + bounded_rect_offset.x * s,
            mini_map_rect.top() - s * bounded_rect.top() + bounded_rect_offset.y * s
                - offset.max(0.0),
        );

        painter.rect_filled(painter.clip_rect(), 0.0, ui.visuals().extreme_bg_color);

        let out = self.renderer.render_svg(
            ui,
            tlbr_ctx.buffer,
            &mut painter,
            RenderOptions { viewport_transform: Some(viewport_transform) },
            tlbr_ctx.viewport_settings.master_transform,
        );

        let viewport_rect =
            transform_rect(tlbr_ctx.viewport_settings.container_rect, out.absolute_transform);

        let blue = ui.visuals().widgets.active.bg_fill;
        painter.rect(
            viewport_rect,
            0.0,
            blue.linear_multiply(0.2),
            egui::Stroke { width: 0.5, color: blue },
        );

        self.show_scroll_bar(ui, tlbr_ctx, mini_map_rect);

        let res = ui.interact(
            egui::Rect::from_min_size(
                mini_map_rect.min,
                egui::vec2(MINI_MAP_WIDTH - SCROLLBAR_WIDTH, mini_map_rect.height()),
            ),
            egui::Id::from("scroll_mini_map"),
            egui::Sense::click_and_drag(),
        );

        let mut has_transformed_canvas = false;

        if let Some(click_pos) = ui.input(|r| r.pointer.interact_pos()) {
            let maybe_delta = if (res.clicked() || res.drag_started())
                && !viewport_rect.contains(click_pos)
            {
                Some((viewport_rect.center() - click_pos) / out.absolute_transform.sx)
            } else if res.dragged() {
                let delta_factor = if mini_map_full_height > mini_map_rect.height() {
                    mini_map_rect.height() / mini_map_full_height
                } else {
                    1.0
                };

                Some(
                    -get_pan(ui, tlbr_ctx.settings.pencil_only_drawing).unwrap_or(res.drag_delta())
                        / out.absolute_transform.sx
                        / delta_factor,
                )
            } else {
                None
            };

            let transform =
                maybe_delta.map(|delta| Transform::default().post_translate(delta.x, delta.y));

            if let Some(transform) = transform {
                transform_canvas(tlbr_ctx.buffer, tlbr_ctx.viewport_settings, transform);
                has_transformed_canvas = true;
            }
        }

        ui.painter()
            .extend([shadow, scroll_bar_line_sep, mini_map_line_sep]);

        (has_transformed_canvas, Some(res))
    }

    fn show_scroll_bar(
        &self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext, mini_map_rect: egui::Rect,
    ) {
        let bounded_rect = match tlbr_ctx.viewport_settings.bounded_rect {
            Some(rect) => rect,
            None => return,
        };

        let invert_master_transform = tlbr_ctx
            .viewport_settings
            .master_transform
            .invert()
            .unwrap();

        let bounded_rect = transform_rect(bounded_rect, invert_master_transform);
        // make sure the min of bounded_rect starts at (0,0)
        let bounded_rect_offset = egui::vec2(-bounded_rect.left(), -bounded_rect.top());
        let bounded_rect = bounded_rect.translate(bounded_rect_offset);

        let container_rect =
            transform_rect(tlbr_ctx.viewport_settings.container_rect, invert_master_transform)
                .translate(bounded_rect_offset);

        let scrollarea_size = egui::vec2(SCROLLBAR_WIDTH, mini_map_rect.height());

        let scrollarea_rect = egui::Rect::from_min_size(
            mini_map_rect.right_top() - egui::vec2(SCROLLBAR_WIDTH, 0.0),
            scrollarea_size,
        );

        let mut painter = ui.painter().clone();
        painter.set_clip_rect(scrollarea_rect);

        let scale_down_factor = scrollarea_rect.height() / bounded_rect.height();
        let scrollbar_rect = egui::Rect::from_center_size(
            egui::pos2(
                scrollarea_rect.center().x,
                container_rect.center().y * scale_down_factor + mini_map_rect.top(),
            ),
            egui::vec2(scrollarea_size.x / 3.0, container_rect.height() * scale_down_factor),
        );
        let blue = ui.visuals().widgets.active.bg_fill;
        painter.rect_filled(scrollbar_rect, ui.visuals().window_rounding, blue);

        let scrollarea_res = ui.interact(
            scrollarea_rect,
            egui::Id::from("mini_map_scroll_area"),
            egui::Sense::click_and_drag(),
        );

        let mut delta = egui::vec2(0.0, 0.0);
        if scrollarea_res.dragged() {
            delta = -scrollarea_res.drag_delta();
        }
        if scrollarea_res.clicked() {
            if let Some(hover_pos) = scrollarea_res.hover_pos() {
                if !scrollbar_rect.contains(hover_pos) {
                    delta = scrollbar_rect.center() - hover_pos;
                }
            }
        }
        delta = delta / scale_down_factor * tlbr_ctx.viewport_settings.master_transform.sx;

        if scrollarea_res.hover_pos().is_some() {
            let smooth_scroll_delta = ui.input(|r| r.smooth_scroll_delta);
            if smooth_scroll_delta.y != 0.0 {
                delta = smooth_scroll_delta;
            }
        }
        let transform = Transform::default().post_translate(0.0, delta.y);
        if delta.y != 0.0 {
            transform_canvas(tlbr_ctx.buffer, tlbr_ctx.viewport_settings, transform);
        }
    }
}
