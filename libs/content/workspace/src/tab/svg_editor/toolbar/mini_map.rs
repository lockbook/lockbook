use resvg::usvg::Transform;

use crate::tab::svg_editor::{
    gesture_handler::transform_canvas,
    renderer::RenderOptions,
    toolbar::{Toolbar, ToolbarContext, MINI_MAP_WIDTH},
    util::transform_rect,
};
const SCROLLBAR_WIDTH: f32 = 12.0;

impl Toolbar {
    pub fn show_mini_map(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> Option<egui::Response> {
        if !tlbr_ctx.settings.show_mini_map
            || !tlbr_ctx.viewport_settings.is_scroll_mode()
            || tlbr_ctx.viewport_settings.bounded_rect.is_none()
        {
            return None;
        }
        let bounded_rect = tlbr_ctx.viewport_settings.bounded_rect.unwrap();

        let mini_map_size =
            egui::vec2(MINI_MAP_WIDTH, tlbr_ctx.viewport_settings.container_rect.height());

        let mini_map_rect = egui::Rect::from_min_size(
            tlbr_ctx.viewport_settings.container_rect.right_top()
                - egui::vec2(mini_map_size.x, 0.0),
            mini_map_size - egui::vec2(SCROLLBAR_WIDTH, 0.0),
        );
        let shadow: egui::Shape = egui::Shadow {
            offset: egui::vec2(0.0, 0.0),
            blur: 40.0,
            spread: 0.0,
            color: ui.visuals().window_shadow.color,
        }
        .as_shape(mini_map_rect, 0.0)
        .into();
        let line_sep = egui::Shape::line_segment(
            [mini_map_rect.left_top(), mini_map_rect.left_bottom()],
            egui::Stroke { width: 1.5, color: ui.visuals().window_stroke.color },
        );
        ui.painter().extend([shadow, line_sep]);

        self.show_scroll_bar(ui, tlbr_ctx, mini_map_rect);

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
        let scroll_percentage = container_rect.top() / bounded_rect.height();
        let offset = mini_map_off_screen_height * scroll_percentage;

        // let no_scroll_diff = (bounded_rect.height() / s) - absolute_mini_map_rect.height();
        // let mini_map_no_scroll_height = (bounded_rect.height() / s) / scale_down_factor;

        // let y_offset =
        //     (bounded_rect.top() - (absolute_mini_map_rect.top())) * s * scale_down_factor;

        // let scroll_ratio = container_rect.top() / bounded_rect.height();
        // let small_scroll_position = scroll_ratio * absolute_mini_map_rect.height();

        let viewport_transform = Transform::identity().post_scale(s, s).post_translate(
            mini_map_rect.center().x - s * bounded_rect.center().x,
            mini_map_rect.top() - s * bounded_rect.top() - offset,
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

        let extended_viewport_rect = egui::Rect::from_two_pos(
            egui::pos2(mini_map_rect.left(), viewport_rect.top()),
            egui::pos2(mini_map_rect.right(), viewport_rect.bottom()),
        );

        let blue = ui.visuals().widgets.active.bg_fill;
        painter.rect(
            extended_viewport_rect,
            0.0,
            blue.linear_multiply(0.2),
            egui::Stroke { width: 0.5, color: blue },
        );

        let res = ui.interact(
            mini_map_rect,
            egui::Id::from("scroll_mini_map"),
            egui::Sense::click_and_drag(),
        );

        if let Some(click_pos) = ui.input(|r| r.pointer.interact_pos()) {
            let maybe_delta = if (res.clicked() || res.drag_started())
                && !extended_viewport_rect.contains(click_pos)
            {
                Some(extended_viewport_rect.center() - click_pos)
            } else if res.dragged() {
                Some(-res.drag_delta())
            } else {
                None
            };

            let transform = if let Some(delta) = maybe_delta {
                let delta = delta / out.absolute_transform.sx
                    * tlbr_ctx.viewport_settings.master_transform.sx;
                Some(Transform::default().post_translate(0.0, delta.y))
            } else {
                None
            };

            if let Some(transform) = transform {
                transform_canvas(tlbr_ctx.buffer, tlbr_ctx.viewport_settings, transform);
            }
        }

        None
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

        let scrollarea_rect = egui::Rect::from_min_size(mini_map_rect.right_top(), scrollarea_size);

        ui.painter()
            .rect_filled(scrollarea_rect, 0.0, ui.visuals().extreme_bg_color);

        let scale_down_factor = scrollarea_rect.height() / bounded_rect.height();
        let scrollbar_rect = egui::Rect::from_min_size(
            egui::pos2(
                scrollarea_rect.left(),
                container_rect.top() * scale_down_factor + mini_map_rect.top(),
            ),
            egui::vec2(scrollarea_size.x, container_rect.height() * scale_down_factor),
        );
        let blue = ui.visuals().widgets.active.bg_fill;
        ui.painter().rect(
            scrollbar_rect,
            0.0,
            blue.linear_multiply(0.2),
            egui::Stroke { width: 0.5, color: blue },
        );

        let res = ui.interact(
            scrollbar_rect,
            egui::Id::from("mini_map_scroll_bar"),
            egui::Sense::click_and_drag(),
        );
        if res.dragged() {
            let delta = -res.drag_delta() / scale_down_factor
                * tlbr_ctx.viewport_settings.master_transform.sx;
            let transform = Transform::default().post_translate(0.0, delta.y);

            transform_canvas(tlbr_ctx.buffer, tlbr_ctx.viewport_settings, transform);
        }
    }
}
