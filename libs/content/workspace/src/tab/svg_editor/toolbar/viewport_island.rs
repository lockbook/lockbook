use egui::Response;
use resvg::usvg::Transform;

use crate::{
    tab::svg_editor::{
        gesture_handler::{
            calc_elements_bounds, get_rect_identity_transform, get_zoom_fit_transform,
            transform_canvas, zoom_percentage_to_transform, MIN_ZOOM_LEVEL,
        },
        renderer::{RenderOptions, RendererOutput},
        util::{draw_dashed_line, transform_rect},
    },
    theme::icons::Icon,
    widgets::{switch, Button},
};

use super::{Toolbar, ToolbarContext, ViewportMode};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ViewportPopover {
    More,
    ZoomStops,
}

enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Toolbar {
    pub fn show_viewport_controls(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> Option<egui::Response> {
        let history_island = self.layout.history_island?;
        let viewport_island_x_start = history_island.right() + 15.0;
        let viewport_island_y_start = history_island.top();

        let viewport_rect = self.layout.viewport_island.unwrap_or(egui::Rect {
            min: egui::pos2(viewport_island_x_start, viewport_island_y_start),
            max: egui::Pos2 { x: viewport_island_x_start, y: history_island.bottom() },
        });

        let mut island_res = ui
            .allocate_ui_at_rect(viewport_rect, |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::symmetric(7.5, 3.5))
                    .show(ui, |ui| self.show_inner_island(ui, tlbr_ctx))
            })
            .inner
            .response;

        // the viewport island will expand and shrink depending on the zoom level it's
        // width determines the width of the more viewport popover which should
        // be consistent. Bref, this avoids flicker in the viewport popover
        if self.layout.viewport_island.is_none() {
            self.layout.viewport_island = Some(island_res.rect);
        }
        let viewport_island_rect = self.layout.viewport_island.unwrap();

        if let Some(res) = self.show_popovers(ui, tlbr_ctx, viewport_island_rect) {
            island_res = island_res.union(res);
        }

        if let Some(res) = show_bring_back_btn(ui, tlbr_ctx, viewport_island_rect) {
            island_res = island_res.union(res);
        }

        Some(island_res)
    }

    fn show_inner_island(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext<'_>,
    ) -> egui::InnerResponse<()> {
        ui.horizontal(|ui| {
            let zoom_percentage = (tlbr_ctx.buffer.master_transform.sx * 100.0).round();

            let mut transform = None;
            let zoom_step = 10.0;

            if ui
                .add_enabled_ui(zoom_percentage > zoom_step, |ui| {
                    Button::default().icon(&Icon::ZOOM_OUT).show(ui)
                })
                .inner
                .clicked()
            {
                let target_zoom_percentage =
                    ((zoom_percentage / zoom_step).floor() - 1.0) * zoom_step;

                transform =
                    Some(zoom_percentage_to_transform(target_zoom_percentage, tlbr_ctx.buffer, ui));
            }

            let zoom_percentage_label = if tlbr_ctx.buffer.master_transform.sx <= MIN_ZOOM_LEVEL {
                "MAX"
            } else {
                &format!("{}%", zoom_percentage as i32)
            };

            let zoom_pct_btn = Button::default().text(zoom_percentage_label).show(ui);

            self.layout.zoom_pct_btn = Some(zoom_pct_btn.rect);

            if zoom_pct_btn.clicked() || zoom_pct_btn.drag_started() {
                self.toggle_popover(Some(ViewportPopover::ZoomStops));
            }

            if Button::default().icon(&Icon::ZOOM_IN).show(ui).clicked() {
                let target_zoom_percentage =
                    ((zoom_percentage / zoom_step).floor() + 1.0) * zoom_step;

                transform =
                    Some(zoom_percentage_to_transform(target_zoom_percentage, tlbr_ctx.buffer, ui));
            };

            if let Some(t) = transform {
                transform_canvas(tlbr_ctx.buffer, tlbr_ctx.inner_rect, t);
            };

            ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));

            let icon = if let Some(ViewportPopover::More) = self.viewport_popover {
                Icon::ARROW_UP
            } else {
                Icon::ARROW_DOWN
            };

            let more_btn = Button::default().icon(&icon).show(ui);
            if more_btn.clicked() || more_btn.drag_started() {
                self.toggle_popover(Some(ViewportPopover::More))
            }
        })
    }

    fn show_popovers(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext<'_>,
        viewport_island_rect: egui::Rect,
    ) -> Option<Response> {
        let opacity = if self.viewport_popover.is_none() || self.hide_overlay { 0.0 } else { 1.0 };

        ui.scope(|ui| {
            ui.set_opacity(opacity);
            if let Some(popover) = self.viewport_popover {
                let popover_rect = match popover {
                    ViewportPopover::More => {
                        let parent_container = self
                            .layout
                            .history_island
                            .unwrap_or(viewport_island_rect)
                            .union(viewport_island_rect);

                        let min =
                            egui::pos2(parent_container.left(), parent_container.bottom() + 10.0);

                        egui::Rect { min, max: min + egui::vec2(parent_container.width(), 0.0) }
                    }
                    ViewportPopover::ZoomStops => {
                        let parent_container = self.layout.zoom_pct_btn.unwrap_or(egui::Rect::ZERO);
                        ui.visuals_mut().window_rounding /= 2.0;
                        let min = egui::pos2(
                            parent_container.center().x
                                - self
                                    .layout
                                    .zoom_stops_popover
                                    .unwrap_or(egui::Rect::ZERO)
                                    .width()
                                    / 2.0,
                            parent_container.bottom() + 10.0,
                        );

                        let zoom_stop_length = if let Some(rect) = self.layout.zoom_stops_popover {
                            rect.width()
                        } else {
                            70.0 // just an approximation to attempt avoiding layout flashes
                        };

                        // todo: avoid layout flashes, something fishy is happening here
                        egui::Rect { min, max: min + egui::vec2(zoom_stop_length, 0.0) }
                    }
                };

                let popover_res = ui
                    .allocate_ui_at_rect(popover_rect, |ui| {
                        egui::Frame::window(ui.style()).show(ui, |ui| {
                            ui.set_min_width(
                                popover_rect.width()
                                    - ui.style().spacing.window_margin.left
                                    - ui.style().spacing.window_margin.right,
                            );

                            match popover {
                                ViewportPopover::More => self.show_more_popover(ui, tlbr_ctx),
                                ViewportPopover::ZoomStops => {
                                    self.show_zoom_stops_popover(ui, tlbr_ctx)
                                }
                            }
                        })
                    })
                    .inner
                    .response;

                match popover {
                    ViewportPopover::More => self.layout.viewport_popover = Some(popover_res.rect),
                    ViewportPopover::ZoomStops => {
                        self.layout.zoom_stops_popover = Some(popover_res.rect)
                    }
                }
                Some(popover_res)
            } else {
                None
            }
        })
        .inner
    }

    fn show_zoom_stops_popover(&self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext) {
        if Button::default().text("FIT").show(ui).clicked() {
            let transform = get_zoom_fit_transform(tlbr_ctx.buffer, tlbr_ctx.container_rect, false)
                .unwrap_or_default();

            transform_canvas(tlbr_ctx.buffer, tlbr_ctx.inner_rect, transform);
        }

        for zoom_percentage in [120.0, 100.0, 80.0, 60.0] {
            if Button::default()
                .text(format!("{}%", (zoom_percentage as i32)))
                .show(ui)
                .clicked()
            {
                let transform = zoom_percentage_to_transform(zoom_percentage, tlbr_ctx.buffer, ui);
                transform_canvas(tlbr_ctx.buffer, tlbr_ctx.inner_rect, transform);
            }
        }
    }

    fn show_more_popover(&mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext) {
        // decreases the height of radio and toggle buttons
        ui.spacing_mut().interact_size.y /= 1.5;

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(5.0);
            ui.label(egui::RichText::new("Choose your layout:").size(13.0));
        });

        let mini_map_res = egui::Frame::default()
            .fill(ui.visuals().code_bg_color)
            .inner_margin(egui::Margin::same(30.0))
            .outer_margin(egui::Margin::symmetric(0.0, 10.0))
            .rounding(ui.visuals().window_rounding / 2.0)
            .show(ui, |ui| self.show_mini_map(ui, tlbr_ctx))
            .inner;

        ui.horizontal(|ui| {
            for mode in ViewportMode::variants() {
                let res = ui.radio(mode.is_active(tlbr_ctx), mode.label());
                if res.clicked() || res.drag_started() {
                    mode.set_active(tlbr_ctx);
                    if let Some(bounds) = mini_map_res {
                        if !tlbr_ctx.inner_rect.is_infinite_mode() {
                            tlbr_ctx.inner_rect.bounded_rect = bounds.0;
                            tlbr_ctx.inner_rect.viewport_transform = Some(bounds.1);
                        }
                    }
                }
                ui.add_space(8.0);
            }
        });

        ui.add_space(15.0);
        ui.add(egui::Separator::default().shrink(ui.available_width() * 0.45));
        ui.add_space(15.0);

        ui.horizontal(|ui| {
            ui.label("Show dot grid");
            ui.add_space(10.0);
            switch(ui, &mut tlbr_ctx.settings.show_dot_grid);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(5.0);
                switch(ui, &mut self.gesture_handler.is_zoom_locked);
                ui.add_space(10.0);
                ui.label("Zoom lock");
            });
        });

        ui.add_space(10.0);
    }

    fn show_mini_map(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext<'_>,
    ) -> Option<(egui::Rect, Transform)> {
        // let's take the full width
        ui.set_width(ui.available_width());

        let preview_size = egui::vec2(100.0, 140.0);
        let preview_rect = egui::Rect::from_min_size(
            egui::pos2(
                ui.cursor().left() + (ui.available_width() - preview_size.x) / 2.0,
                ui.cursor().top(),
            ),
            preview_size,
        );

        let mut preview_painter = ui.painter_at(preview_rect);
        let res =
            ui.interact(preview_rect, egui::Id::new("vp_preview"), egui::Sense::click_and_drag());

        preview_painter.rect_filled(preview_rect, 0.0, ui.style().visuals.extreme_bg_color);

        if tlbr_ctx.inner_rect.is_infinite_mode() {
            tlbr_ctx.inner_rect.viewport_transform = None;
        }

        // todo: calc the right fit transform here, to ensure that
        // it is not less than a minimum transform. solves
        // the empty canvas issue.

        let out = self.renderer.render_svg(
            ui,
            tlbr_ctx.buffer,
            &mut preview_painter,
            RenderOptions {
                tight_fit_mode: tlbr_ctx.inner_rect.viewport_transform.is_none(),
                viewport_transform: tlbr_ctx.inner_rect.viewport_transform,
            },
        );

        if let Some(t) = out.maybe_tight_fit_transform {
            let clipped_rect = transform_rect(tlbr_ctx.container_rect, t);
            let bounded_rect = transform_rect(preview_rect, t.invert().unwrap_or_default());

            let blue = ui.visuals().widgets.active.bg_fill;
            preview_painter.rect(
                clipped_rect,
                0.0,
                blue.linear_multiply(0.2),
                egui::Stroke { width: 0.5, color: blue },
            );

            handle_mini_map_transforms(ui, tlbr_ctx, preview_painter, res, out, clipped_rect);

            ui.advance_cursor_after_rect(preview_rect);

            // Draw the shadows
            let shadow = egui::Shadow {
                offset: egui::vec2(0.0, 0.0),
                blur: 40.0,
                spread: 10.0,
                color: ui.visuals().extreme_bg_color,
            }
            .as_shape(preview_rect, 0.0);

            let left_bound_rect = egui::Rect::from_two_pos(
                preview_rect.min,
                preview_rect.min + egui::vec2(-100.0, preview_rect.height()),
            );
            let right_bound_rect = egui::Rect::from_two_pos(
                preview_rect.max,
                preview_rect.max + egui::vec2(100.0, -preview_rect.height()),
            );

            let top_bound_rect = egui::Rect::from_two_pos(
                preview_rect.min,
                preview_rect.min + egui::vec2(preview_rect.width(), -100.0),
            );

            let bottom_bound_rect = egui::Rect::from_two_pos(
                preview_rect.max,
                preview_rect.max + egui::vec2(-preview_rect.width(), 100.0),
            );

            let res =
                show_side_controls(ui, Side::Left, left_bound_rect, shadow.into(), tlbr_ctx, t)
                    .union(show_side_controls(
                        ui,
                        Side::Right,
                        right_bound_rect,
                        shadow.into(),
                        tlbr_ctx,
                        t,
                    ))
                    .union(show_side_controls(
                        ui,
                        Side::Top,
                        top_bound_rect,
                        shadow.into(),
                        tlbr_ctx,
                        t,
                    ))
                    .union(show_side_controls(
                        ui,
                        Side::Bottom,
                        bottom_bound_rect,
                        shadow.into(),
                        tlbr_ctx,
                        t,
                    ));

            if res.clicked() || res.drag_started() && !tlbr_ctx.inner_rect.is_infinite_mode() {
                tlbr_ctx.inner_rect.bounded_rect = bounded_rect
            }

            Some((bounded_rect, t.pre_concat(tlbr_ctx.buffer.master_transform)))
        } else {
            None
        }
    }

    pub fn toggle_popover(&mut self, new_popover: Option<ViewportPopover>) {
        if self.viewport_popover == new_popover {
            self.viewport_popover = None;
        } else {
            self.viewport_popover = new_popover;
        }

        if let Some(ViewportPopover::More) = self.viewport_popover {
            // without this, content since the last more-popover open
            // would not show - unless you pan to trigger the tess
            self.renderer.request_rerender = true;
        }
    }
}

fn handle_mini_map_transforms(
    ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext<'_>, preview_painter: egui::Painter,
    res: Response, out: RendererOutput, clipped_rect: egui::Rect,
) {
    if let Some(click_pos) = ui.input(|r| r.pointer.interact_pos()) {
        let maybe_delta =
            if (res.clicked() || res.drag_started()) && !clipped_rect.contains(click_pos) {
                Some(clipped_rect.center() - click_pos)
            } else if res.dragged() {
                Some(-res.drag_delta())
            } else {
                None
            };

        let is_outside_bounds = !preview_painter.clip_rect().intersects(clipped_rect)
            && !preview_painter.clip_rect().contains_rect(clipped_rect);

        let transform = if is_outside_bounds && res.clicked() {
            get_zoom_fit_transform(tlbr_ctx.buffer, tlbr_ctx.painter.clip_rect(), false)
        } else if let Some(delta) = maybe_delta {
            let delta = delta / out.maybe_tight_fit_transform.unwrap_or_default().sx;
            Some(Transform::default().post_translate(delta.x, delta.y))
        } else {
            None
        };

        if let Some(transform) = transform {
            transform_canvas(tlbr_ctx.buffer, tlbr_ctx.inner_rect, transform);
        }
    }
}

fn show_bring_back_btn(
    ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext<'_>, viewport_island_rect: egui::Rect,
) -> Option<Response> {
    let elements_bound = match calc_elements_bounds(tlbr_ctx.buffer) {
        Some(rect) => transform_rect(rect, tlbr_ctx.buffer.master_transform),
        None => return None,
    };

    if tlbr_ctx
        .buffer
        .elements
        .iter()
        .filter(|(_, e)| !e.deleted())
        .count()
        != 0
        && !tlbr_ctx.container_rect.contains_rect(elements_bound)
        && !tlbr_ctx.container_rect.intersects(elements_bound)
    {
        let bring_home_x_start = viewport_island_rect.right() + 15.0;
        let bring_home_y_start = viewport_island_rect.top();

        let bring_home_rect = egui::Rect {
            min: egui::pos2(bring_home_x_start, bring_home_y_start),
            max: egui::Pos2 { x: bring_home_x_start, y: viewport_island_rect.bottom() },
        };

        let res = ui.allocate_ui_at_rect(bring_home_rect, |ui| {
            egui::Frame::window(ui.style())
                .inner_margin(egui::Margin::symmetric(7.5, 3.5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let text_stroke = egui::Stroke {
                            color: ui.visuals().widgets.active.bg_fill,
                            ..Default::default()
                        };
                        ui.visuals_mut().widgets.inactive.fg_stroke = text_stroke;
                        ui.visuals_mut().widgets.active.fg_stroke = text_stroke;
                        ui.visuals_mut().widgets.hovered.fg_stroke = text_stroke;

                        if Button::default()
                            .text("Focus back to content")
                            .show(ui)
                            .clicked()
                        {
                            let transform = get_rect_identity_transform(
                                tlbr_ctx.container_rect,
                                elements_bound,
                                0.7,
                                tlbr_ctx.container_rect.center(),
                            )
                            .unwrap_or_default();

                            transform_canvas(tlbr_ctx.buffer, tlbr_ctx.inner_rect, transform);
                        }
                    })
                })
        });
        Some(res.inner.response)
    } else {
        None
    }
}

fn show_side_controls(
    ui: &mut egui::Ui, side: Side, rect: egui::Rect, shadow: egui::Shape,
    tlbr_ctx: &mut ToolbarContext, transform: Transform,
) -> Response {
    let line_extension = 5.0;

    let (layout, segment_edges, is_locked) = match side {
        Side::Left => {
            let edges = [
                rect.right_top() - egui::vec2(0.0, line_extension),
                rect.right_bottom() + egui::vec2(0.0, line_extension),
            ];
            let layout = egui::Layout::right_to_left(egui::Align::Center);
            (layout, edges, &mut tlbr_ctx.inner_rect.left_locked)
        }
        Side::Right => {
            let layout = egui::Layout::left_to_right(egui::Align::Center);
            let edges = [
                rect.left_top() - egui::vec2(0.0, line_extension),
                rect.left_bottom() + egui::vec2(0.0, line_extension),
            ];
            (layout, edges, &mut tlbr_ctx.inner_rect.right_locked)
        }
        Side::Top => {
            let layout = egui::Layout::bottom_up(egui::Align::Center);
            let edges = [
                rect.left_bottom() - egui::vec2(line_extension, 0.0),
                rect.right_bottom() + egui::vec2(line_extension, 0.0),
            ];
            (layout, edges, &mut tlbr_ctx.inner_rect.top_locked)
        }
        Side::Bottom => {
            let layout = egui::Layout::top_down(egui::Align::Center);
            let edges = [
                rect.left_top() - egui::vec2(line_extension, 0.0),
                rect.right_top() + egui::vec2(line_extension, 0.0),
            ];
            (layout, edges, &mut tlbr_ctx.inner_rect.bottom_locked)
        }
    };

    let unlocked_stroke =
        egui::Stroke { width: 1.0, color: egui::Color32::GRAY.linear_multiply(0.8) };
    let locked_stroke = egui::Stroke { width: 1.4, color: egui::Color32::GRAY };

    let child_ui = &mut ui.child_ui(rect, layout, None);

    if !*is_locked {
        child_ui.set_clip_rect(rect);
        child_ui.painter().add(shadow);
    }

    let opacity = if *is_locked { 1.0 } else { 0.3 };
    child_ui.set_opacity(opacity);
    let icon = if *is_locked { Icon::LOCK_CLOSED } else { Icon::LOCK_OPEN };
    let res = Button::default().icon(&icon.size(13.0)).show(child_ui);

    if res.clicked() {
        *is_locked = !*is_locked;
        if *is_locked {
            tlbr_ctx.inner_rect.viewport_transform =
                Some(transform.pre_concat(tlbr_ctx.buffer.master_transform));
        }
    }

    if *is_locked {
        ui.painter().line_segment(segment_edges, locked_stroke);
    } else {
        draw_dashed_line(ui.painter(), &segment_edges, 5.0, 3.0, unlocked_stroke);
    }
    res
}
