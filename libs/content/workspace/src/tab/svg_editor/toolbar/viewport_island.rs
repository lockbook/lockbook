use std::sync::Arc;

use egui::Response;
use lb_rs::model::svg::buffer::get_background_colors;
use resvg::usvg::Transform;

use crate::tab::svg_editor::background::{show_dot_grid, show_lines_background};
use crate::tab::svg_editor::gesture_handler::{
    MIN_ZOOM_LEVEL, get_rect_identity_transform, get_zoom_fit_transform, transform_canvas,
    zoom_percentage_to_transform,
};
use crate::tab::svg_editor::toolbar::get_non_additive;
use crate::tab::svg_editor::util::draw_dashed_line;
use crate::tab::svg_editor::{BackgroundOverlay, get_secondary_color};
use crate::theme::icons::Icon;
use crate::theme::palette::ThemePalette;
use crate::widgets::{Button, switch};

use super::{Toolbar, ToolbarContext};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ViewportPopover {
    More,
    ZoomStops,
}

pub enum ViewportMode {
    Scroll,
    Infinite,
}

impl ViewportMode {
    pub fn variants() -> [ViewportMode; 2] {
        [ViewportMode::Scroll, ViewportMode::Infinite]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ViewportMode::Scroll => "Notebook",
            ViewportMode::Infinite => "Infinite",
        }
    }

    pub fn is_active(&self, tlbr_ctx: &ToolbarContext) -> bool {
        match self {
            ViewportMode::Scroll => tlbr_ctx.viewport_settings.is_scroll_mode(),
            ViewportMode::Infinite => tlbr_ctx.viewport_settings.is_infinite_mode(),
        }
    }

    pub fn set_active(&self, tlbr_ctx: &mut ToolbarContext) {
        match self {
            ViewportMode::Scroll => tlbr_ctx.viewport_settings.set_scroll_mode(),
            ViewportMode::Infinite => tlbr_ctx.viewport_settings.set_infinite_mode(),
        }
    }
}

impl Toolbar {
    pub fn show_viewport_controls(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> Option<egui::Response> {
        let history_island = self.layout.history_island?;
        let viewport_island_x_start = history_island.right() + 15.0;
        let viewport_island_y_start = history_island.top();

        let viewport_rect = egui::Rect {
            min: egui::pos2(viewport_island_x_start, viewport_island_y_start),
            max: egui::Pos2 { x: viewport_island_x_start, y: history_island.bottom() },
        };

        let mut island_res = ui
            .allocate_ui_at_rect(viewport_rect, |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::symmetric(7.5, 3.5))
                    .show(ui, |ui| self.show_inner_island(ui, tlbr_ctx))
            })
            .inner
            .response;

        self.layout.viewport_island = Some(island_res.rect);

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
            let zoom_percentage = (tlbr_ctx.viewport_settings.master_transform.sx * 100.0).round();

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

                transform = Some(zoom_percentage_to_transform(
                    target_zoom_percentage,
                    tlbr_ctx.viewport_settings,
                    ui,
                ));
            }

            let zoom_percentage_label =
                if tlbr_ctx.viewport_settings.master_transform.sx <= MIN_ZOOM_LEVEL {
                    "MAX"
                } else {
                    &format!("{}%", zoom_percentage as i32)
                };

            let zoom_pct_btn = Button::default().text(zoom_percentage_label).show(ui);
            self.layout.zoom_pct_btn = Some(zoom_pct_btn.rect);

            if zoom_pct_btn.clicked() || zoom_pct_btn.drag_started() {
                self.toggle_viewport_popover(Some(ViewportPopover::ZoomStops));
            }

            if Button::default().icon(&Icon::ZOOM_IN).show(ui).clicked() {
                let target_zoom_percentage =
                    ((zoom_percentage / zoom_step).floor() + 1.0) * zoom_step;

                transform = Some(zoom_percentage_to_transform(
                    target_zoom_percentage,
                    tlbr_ctx.viewport_settings,
                    ui,
                ));
            };

            if let Some(t) = transform {
                transform_canvas(tlbr_ctx.buffer, tlbr_ctx.viewport_settings, t);
            };

            // fixes the jitter
            ui.add_space((50.0 - zoom_pct_btn.rect.width()).max(0.0));

            ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));

            let icon = if let Some(ViewportPopover::More) = self.viewport_popover {
                Icon::ARROW_UP
            } else {
                Icon::ARROW_DOWN
            };

            let more_btn = Button::default().icon(&icon).show(ui);
            if more_btn.clicked() || more_btn.drag_started() {
                self.toggle_viewport_popover(Some(ViewportPopover::More))
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
            let transform = get_zoom_fit_transform(tlbr_ctx.viewport_settings).unwrap_or_default();

            transform_canvas(tlbr_ctx.buffer, tlbr_ctx.viewport_settings, transform);
        }

        for zoom_percentage in [120.0, 100.0, 80.0] {
            if Button::default()
                .text(format!("{}%", (zoom_percentage as i32)))
                .show(ui)
                .clicked()
            {
                let transform =
                    zoom_percentage_to_transform(zoom_percentage, tlbr_ctx.viewport_settings, ui);
                transform_canvas(tlbr_ctx.buffer, tlbr_ctx.viewport_settings, transform);
            }
        }
    }

    fn show_more_popover(&mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext) {
        // decreases the height of radio and toggle buttons
        ui.spacing_mut().interact_size.y /= 1.5;

        ui.add_space(10.0);

        ui.scope(|ui| {
            show_background_selector(ui, tlbr_ctx);
            ui.add_space(10.0);
            show_background_color_selector(ui, tlbr_ctx);
        });

        ui.add_space(20.0);
        ui.label(
            egui::RichText::new("Layout".to_uppercase())
                .font(egui::FontId::new(12.0, egui::FontFamily::Name(Arc::from("Bold"))))
                .color(egui::Color32::GRAY),
        );
        egui::Frame::default()
            .fill(ThemePalette::resolve_dynamic_color(
                tlbr_ctx.settings.background_color,
                ui.visuals().dark_mode,
            ))
            .inner_margin(egui::Margin::same(30.0))
            .outer_margin(egui::Margin::symmetric(0.0, 5.0))
            .rounding(ui.visuals().window_rounding / 2.0)
            .show(ui, |ui| {
                // let's take the full width
                ui.set_width(ui.available_width());

                let max_preview = egui::vec2(100.0, 100.0);
                let scale_down_factor = if tlbr_ctx.viewport_settings.container_rect.height()
                    > tlbr_ctx.viewport_settings.container_rect.width()
                {
                    max_preview.y / tlbr_ctx.viewport_settings.container_rect.height()
                } else {
                    max_preview.x / tlbr_ctx.viewport_settings.container_rect.width()
                };

                let preview_size = egui::vec2(
                    tlbr_ctx.viewport_settings.container_rect.width() * scale_down_factor,
                    tlbr_ctx.viewport_settings.container_rect.height() * scale_down_factor,
                );

                let preview_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        ui.cursor().left() + (ui.available_width() - preview_size.x) / 2.0,
                        ui.cursor().top(),
                    ),
                    preview_size,
                );

                let preview_painter = ui.painter_at(preview_rect);

                preview_painter.rect_filled(
                    preview_rect,
                    0.0,
                    ThemePalette::resolve_dynamic_color(
                        tlbr_ctx.settings.background_color,
                        ui.visuals().dark_mode,
                    ),
                );
                ui.advance_cursor_after_rect(preview_rect);
                // Draw the shadows

                let shadow = egui::Shadow {
                    offset: egui::vec2(0.0, 0.0),
                    blur: 40.0,
                    spread: 0.5,
                    color: tlbr_ctx.settings.get_secondary_background_color(ui),
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

                show_side_controls(ui, Side::Left, left_bound_rect, shadow.into(), tlbr_ctx);
                show_side_controls(ui, Side::Right, right_bound_rect, shadow.into(), tlbr_ctx);
                show_side_controls(ui, Side::Top, top_bound_rect, shadow.into(), tlbr_ctx);
                show_side_controls(ui, Side::Bottom, bottom_bound_rect, shadow.into(), tlbr_ctx);
            });

        ui.add_space(5.0);
        ui.vertical(|ui| {
            for mode in ViewportMode::variants() {
                let res = ui.radio(mode.is_active(tlbr_ctx), mode.label());

                if res.clicked() || res.drag_started() {
                    mode.set_active(tlbr_ctx);

                    tlbr_ctx.viewport_settings.bounded_rect = Some(tlbr_ctx.painter.clip_rect());

                    tlbr_ctx
                        .settings
                        .update_viewport_settings(tlbr_ctx.viewport_settings);

                    tlbr_ctx.cfg.set_canvas_settings(*tlbr_ctx.settings);
                }

                ui.add_space(8.0);
            }
        });

        ui.add_space(5.0);

        ui.add(egui::Separator::default().shrink(ui.available_width()));

        ui.add_space(5.0);

        ui.horizontal(|ui| {
            ui.label("Show mini map");
            ui.add_space(10.0);
            if switch(ui, &mut tlbr_ctx.settings.show_mini_map).changed() {
                tlbr_ctx.cfg.set_canvas_settings(*tlbr_ctx.settings);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    switch(ui, &mut self.gesture_handler.is_zoom_locked);
                });
                ui.add_space(10.0);

                ui.label("Zoom lock");
            });
        });
        ui.add_space(10.0);
    }

    pub fn toggle_viewport_popover(&mut self, new_popover: Option<ViewportPopover>) {
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

fn show_background_color_selector(ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext<'_>) {
    let colors = get_background_colors();
    ui.horizontal_wrapped(|ui| {
        let active_color = ThemePalette::resolve_dynamic_color(
            tlbr_ctx.settings.background_color,
            ui.visuals().dark_mode,
        );
        for dyn_color in colors {
            let color = ThemePalette::resolve_dynamic_color(dyn_color, ui.visuals().dark_mode);

            let (id, rect) = ui.allocate_space(egui::vec2(40.0, 20.0));
            let res = ui.interact(rect, id, egui::Sense::click_and_drag());
            if res.clicked() || res.drag_started() {
                tlbr_ctx.settings.background_color = dyn_color;
                tlbr_ctx.cfg.set_canvas_settings(*tlbr_ctx.settings);
            }

            let is_active = get_non_additive(&active_color).eq(&color);
            let stroke_color = get_secondary_color(color);

            ui.painter().rect(
                rect,
                10.0,
                color,
                if is_active {
                    egui::Stroke { width: 2.0, color: stroke_color }
                } else {
                    egui::Stroke { width: 0.5, color: stroke_color }
                },
            );
            ui.add_space(10.0);
        }
    });
}

fn show_background_selector(ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext<'_>) {
    ui.label(
        egui::RichText::new("Background".to_uppercase())
            .font(egui::FontId::new(12.0, egui::FontFamily::Name(Arc::from("Bold"))))
            .color(egui::Color32::GRAY),
    );
    ui.add_space(5.0);

    let x_padding = 15.0;
    let width = ui.available_width();
    let selector_count_per_row = 4;
    let bg_selector_dim = egui::vec2(
        (width - x_padding * (selector_count_per_row - 1) as f32) / selector_count_per_row as f32,
        40.0,
    );

    let background_color = ThemePalette::resolve_dynamic_color(
        tlbr_ctx.settings.background_color,
        ui.visuals().dark_mode,
    );
    let active_stroke_color = tlbr_ctx.settings.get_secondary_background_color(ui);

    let transform = Transform::identity()
        .post_scale(0.5, 0.5)
        .post_translate(-1.0, 3.0);

    ui.visuals_mut().widgets.active.fg_stroke.color = active_stroke_color;
    ui.visuals_mut().widgets.inactive.fg_stroke.color = active_stroke_color;
    ui.visuals_mut().widgets.hovered.fg_stroke.color = active_stroke_color;

    ui.visuals_mut().widgets.active.fg_stroke.width =
        ui.visuals_mut().widgets.hovered.fg_stroke.width;

    ui.horizontal(|ui| {
        let dot_selector_rect = egui::Rect::from_min_size(ui.cursor().min, bg_selector_dim);
        let dot_res = ui.allocate_rect(dot_selector_rect, egui::Sense::click_and_drag());

        let is_active = tlbr_ctx.settings.background_type == BackgroundOverlay::Dots;
        let mut stroke = ui.style().interact(&dot_res).fg_stroke;
        if is_active {
            stroke.width = 2.0
        }
        ui.painter()
            .rect(dot_res.rect, 3.0, background_color, stroke);

        show_dot_grid(dot_res.rect, transform, &ui.painter_at(dot_res.rect), Some(1.5));

        if dot_res.clicked() {
            tlbr_ctx.settings.background_type = BackgroundOverlay::Dots;
            tlbr_ctx.cfg.set_canvas_settings(*tlbr_ctx.settings);
        }

        ui.add_space(x_padding);
        let notebook_selector_rect = egui::Rect::from_min_size(ui.cursor().min, bg_selector_dim);

        let notebook_res = ui.allocate_rect(notebook_selector_rect, egui::Sense::click_and_drag());

        let is_active = tlbr_ctx.settings.background_type == BackgroundOverlay::Lines;

        let mut stroke = ui.style().interact(&notebook_res).fg_stroke;
        if is_active {
            stroke.width = 2.0
        };

        ui.painter()
            .rect(notebook_res.rect, 3.0, background_color, stroke);

        show_lines_background(
            false,
            notebook_res.rect,
            transform,
            &ui.painter_at(notebook_res.rect),
            Some(1.0),
        );

        if notebook_res.clicked() {
            tlbr_ctx.settings.background_type = BackgroundOverlay::Lines;
            tlbr_ctx.cfg.set_canvas_settings(*tlbr_ctx.settings);
        }

        ui.add_space(x_padding);

        let grid_selector_rect = egui::Rect::from_min_size(ui.cursor().min, bg_selector_dim);

        let grid_res = ui.allocate_rect(grid_selector_rect, egui::Sense::click_and_drag());

        let is_active = tlbr_ctx.settings.background_type == BackgroundOverlay::Grid;

        let mut stroke = ui.style().interact(&grid_res).fg_stroke;
        if is_active {
            stroke.width = 2.0
        };

        ui.painter()
            .rect(grid_res.rect, 3.0, background_color, stroke);

        show_lines_background(
            true,
            grid_res.rect,
            transform,
            &ui.painter_at(grid_res.rect),
            Some(1.0),
        );

        if grid_res.clicked() {
            tlbr_ctx.settings.background_type = BackgroundOverlay::Grid;
            tlbr_ctx.cfg.set_canvas_settings(*tlbr_ctx.settings);
        }

        ui.add_space(x_padding);

        let blank_selector_rect = egui::Rect::from_min_size(ui.cursor().min, bg_selector_dim);

        let blank_res = ui.allocate_rect(blank_selector_rect, egui::Sense::click_and_drag());

        let is_active = tlbr_ctx.settings.background_type == BackgroundOverlay::Blank;

        let mut stroke = ui.style().interact(&blank_res).fg_stroke;
        if is_active {
            stroke.width = 2.0
        };

        ui.painter()
            .rect(blank_res.rect, 3.0, background_color, stroke);

        if blank_res.clicked() {
            tlbr_ctx.settings.background_type = BackgroundOverlay::Blank;
            tlbr_ctx.cfg.set_canvas_settings(*tlbr_ctx.settings);
        }
    });
}

fn show_bring_back_btn(
    ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext<'_>, viewport_island_rect: egui::Rect,
) -> Option<Response> {
    let elements_bound = tlbr_ctx.viewport_settings.bounded_rect?;

    if tlbr_ctx
        .buffer
        .elements
        .iter()
        .filter(|(_, e)| !e.deleted())
        .count()
        != 0
        && !tlbr_ctx
            .viewport_settings
            .container_rect
            .contains_rect(elements_bound)
        && !tlbr_ctx
            .viewport_settings
            .container_rect
            .intersects(elements_bound)
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
                                tlbr_ctx.viewport_settings.container_rect,
                                elements_bound,
                                0.7,
                                tlbr_ctx.viewport_settings.container_rect.center(),
                            )
                            .unwrap_or_default();

                            transform_canvas(
                                tlbr_ctx.buffer,
                                tlbr_ctx.viewport_settings,
                                transform,
                            );
                        }
                    })
                })
        });
        Some(res.inner.response)
    } else {
        None
    }
}

enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

fn show_side_controls(
    ui: &mut egui::Ui, side: Side, rect: egui::Rect, shadow: egui::Shape,
    tlbr_ctx: &mut ToolbarContext,
) -> Response {
    let line_extension = 5.0;

    let (layout, segment_edges, is_locked) = match side {
        Side::Left => {
            let edges = [
                rect.right_top() - egui::vec2(0.0, line_extension),
                rect.right_bottom() + egui::vec2(0.0, line_extension),
            ];

            let layout = egui::Layout::right_to_left(egui::Align::Center);

            (layout, edges, &mut tlbr_ctx.viewport_settings.left_locked)
        }

        Side::Right => {
            let layout = egui::Layout::left_to_right(egui::Align::Center);

            let edges = [
                rect.left_top() - egui::vec2(0.0, line_extension),
                rect.left_bottom() + egui::vec2(0.0, line_extension),
            ];

            (layout, edges, &mut tlbr_ctx.viewport_settings.right_locked)
        }

        Side::Top => {
            let layout = egui::Layout::bottom_up(egui::Align::Center);

            let edges = [
                rect.left_bottom() - egui::vec2(line_extension, 0.0),
                rect.right_bottom() + egui::vec2(line_extension, 0.0),
            ];

            (layout, edges, &mut tlbr_ctx.viewport_settings.top_locked)
        }

        Side::Bottom => {
            let layout = egui::Layout::top_down(egui::Align::Center);

            let edges = [
                rect.left_top() - egui::vec2(line_extension, 0.0),
                rect.right_top() + egui::vec2(line_extension, 0.0),
            ];

            (layout, edges, &mut tlbr_ctx.viewport_settings.bottom_locked)
        }
    };

    let unlocked_stroke =
        egui::Stroke { width: 1.0, color: tlbr_ctx.settings.get_secondary_background_color(ui) };

    let mut locked_stroke = unlocked_stroke;
    locked_stroke.width = 1.4;

    let child_ui = &mut ui.child_ui(rect, layout, None);

    if !*is_locked {
        child_ui.set_clip_rect(rect);

        child_ui.painter().add(shadow);
    }

    let opacity = if *is_locked { 1.0 } else { 0.3 };

    child_ui.set_opacity(opacity);

    let icon = if *is_locked { Icon::LOCK_CLOSED } else { Icon::LOCK_OPEN };

    let res = Button::default().icon(&icon.size(13.0)).show(child_ui);

    if *is_locked {
        ui.painter().line_segment(segment_edges, locked_stroke);
    } else {
        draw_dashed_line(ui.painter(), &segment_edges, 5.0, 3.0, unlocked_stroke);
    }

    res
}
