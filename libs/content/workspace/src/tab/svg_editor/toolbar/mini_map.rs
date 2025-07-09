use resvg::usvg::Transform;

use crate::tab::svg_editor::{
    gesture_handler::transform_canvas,
    renderer::RenderOptions,
    toolbar::{Toolbar, ToolbarContext},
    util::transform_rect,
};

pub const MINI_MAP_WIDTH: f32 = 100.0;

impl Toolbar {
    pub fn show_mini_map(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> Option<egui::Response> {
        if !tlbr_ctx.settings.show_mini_map || !tlbr_ctx.viewport_settings.is_scroll_mode() {
            return None;
        }
        let mini_map_size =
            egui::vec2(MINI_MAP_WIDTH, tlbr_ctx.viewport_settings.container_rect.height());

        let mini_map_rect = egui::Rect::from_min_size(
            tlbr_ctx.viewport_settings.container_rect.right_top()
                - egui::vec2(mini_map_size.x, 0.0),
            mini_map_size,
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
        let mut painter = ui.painter().clone();
        painter.set_clip_rect(mini_map_rect);

        let bounded_rect = transform_rect(
            tlbr_ctx.viewport_settings.bounded_rect.unwrap(),
            tlbr_ctx
                .viewport_settings
                .master_transform
                .invert()
                .unwrap(),
        );
        let s = mini_map_rect.width() / bounded_rect.width();

        let viewport_transform = Transform::identity().post_scale(s, s).post_translate(
            mini_map_rect.center().x - s * bounded_rect.center().x,
            mini_map_rect.top() - s * bounded_rect.top(),
        );

        painter.rect_filled(painter.clip_rect(), 0.0, ui.visuals().extreme_bg_color);

        let out = self.renderer.render_svg(
            ui,
            tlbr_ctx.buffer,
            &mut painter,
            RenderOptions { viewport_transform: Some(viewport_transform) },
            tlbr_ctx.viewport_settings.master_transform,
        );

        let viewport_rect = transform_rect(
            tlbr_ctx.viewport_settings.container_rect,
            tlbr_ctx
                .viewport_settings
                .master_transform
                .invert()
                .unwrap()
                .post_concat(viewport_transform),
        );

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
                let delta = delta / out.absolute_transform.sx;
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
}
