use std::ops::RangeInclusive;

use bezier_rs::{Cap, Subpath};
use egui::{InnerResponse, Response, RichText};
use egui_animation::{animate_eased, easing};
use glam::DVec2;
use lb_rs::model::svg::{
    buffer::{get_highlighter_colors, get_pen_colors},
    element::{DynamicColor, ManipulatorGroupId},
};
use lyon::tessellation::{BuffersBuilder, FillOptions, FillTessellator, VertexBuffers};

use crate::{
    set_tool,
    tab::svg_editor::{
        eraser::DEFAULT_ERASER_RADIUS,
        gesture_handler::get_rect_identity_transform,
        pen::{PenSettings, DEFAULT_HIGHLIGHTER_STROKE_WIDTH, DEFAULT_PEN_STROKE_WIDTH},
        renderer::VertexConstructor,
        util::{bb_to_rect, devc_to_point},
        CanvasSettings, Pen, Tool,
    },
    theme::{icons::Icon, palette::ThemePalette},
    widgets::{switch, Button},
    workspace::WsPersistentStore,
};

use super::{
    Toolbar, ToolbarContext, COLOR_SWATCH_BTN_RADIUS, SCREEN_PADDING, THICKNESS_BTN_WIDTH,
};

impl Toolbar {
    pub fn show_tools_island(
        &mut self, ui: &mut egui::Ui,
    ) -> InnerResponse<InnerResponse<InnerResponse<()>>> {
        let tools_island_size = self.layout.tools_island.unwrap_or(egui::Rect::ZERO).size();

        let tools_island_x_start = ui.available_rect_before_wrap().left()
            + (ui.available_width() - tools_island_size.x) / 2.0;
        let tools_island_y_start =
            ui.available_rect_before_wrap().bottom() - SCREEN_PADDING - tools_island_size.y;

        let tools_island_rect = egui::Rect {
            min: egui::pos2(tools_island_x_start, tools_island_y_start),
            max: egui::pos2(
                tools_island_x_start + tools_island_size.x,
                tools_island_y_start + tools_island_size.y,
            ),
        };

        let res = ui.allocate_ui_at_rect(tools_island_rect, |ui| {
            egui::Frame::window(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let tool_icon_size = 25.0;

                    let selection_btn = Button::default()
                        .icon(&Icon::HAND.size(tool_icon_size))
                        .show(ui);
                    if selection_btn.clicked() || selection_btn.drag_started() {
                        set_tool!(self, Tool::Selection);
                    }

                    let pen_btn = Button::default()
                        .icon(&Icon::BRUSH.size(tool_icon_size))
                        .show(ui);
                    if pen_btn.clicked() || pen_btn.drag_started() {
                        set_tool!(self, Tool::Pen);
                    }

                    let highlighter_btn = Button::default()
                        .icon(&Icon::HIGHLIGHTER.size(tool_icon_size))
                        .show(ui);
                    if highlighter_btn.clicked() || highlighter_btn.drag_started() {
                        set_tool!(self, Tool::Highlighter);
                    }

                    let eraser_btn = Button::default()
                        .icon(&Icon::ERASER.size(tool_icon_size))
                        .show(ui);
                    if eraser_btn.clicked() || eraser_btn.drag_started() {
                        set_tool!(self, Tool::Eraser);
                    }

                    let active_rect = match self.active_tool {
                        Tool::Pen => pen_btn.rect,
                        Tool::Eraser => eraser_btn.rect,
                        Tool::Selection => selection_btn.rect,
                        Tool::Highlighter => highlighter_btn.rect,
                    };

                    let min_x = animate_eased(
                        ui.ctx(),
                        "min",
                        active_rect.left() + 3.0,
                        0.5,
                        easing::cubic_in_out,
                    );

                    let max_x = animate_eased(
                        ui.ctx(),
                        "max",
                        active_rect.right() - 3.0,
                        0.5,
                        easing::cubic_in_out,
                    );
                    ui.style_mut().animation_time = 2.0;

                    let color = if self.active_tool == Tool::Pen {
                        ThemePalette::resolve_dynamic_color(
                            self.pen.active_color,
                            ui.visuals().dark_mode,
                        )
                        .linear_multiply(self.pen.active_opacity)
                    } else if self.active_tool == Tool::Highlighter {
                        ThemePalette::resolve_dynamic_color(
                            self.highlighter.active_color,
                            ui.visuals().dark_mode,
                        )
                        .linear_multiply(self.highlighter.active_opacity)
                    } else {
                        ui.visuals().text_color().linear_multiply(0.2)
                    };

                    ui.painter().line_segment(
                        [
                            egui::pos2(min_x, active_rect.bottom() + 6.0),
                            egui::pos2(max_x, active_rect.bottom() + 6.0),
                        ],
                        egui::Stroke { width: 3.0, color },
                    );
                })
            })
        });
        self.layout.tools_island = Some(res.response.rect);
        res
    }

    pub fn show_tool_popovers(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> Option<Response> {
        if self.active_tool == Tool::Selection {
            return None;
        }

        let tools_island_rect = self.layout.tools_island?;

        let opacity = animate_eased(
            ui.ctx(),
            "opacity",
            if self.layout.tool_popover.is_none() || self.hide_overlay { 0.0 } else { 1.0 },
            0.2,
            easing::cubic_in_out,
        );
        ui.set_opacity(opacity);
        let tool_popovers_size = self.layout.tool_popover.unwrap_or(egui::Rect::ZERO).size();

        let tool_popover_x_start = ui.available_rect_before_wrap().left()
            + (ui.available_width() - tool_popovers_size.x) / 2.0;
        let tool_popover_y_start = tools_island_rect.top() - tool_popovers_size.y - 10.0;
        let tool_popover_rect = egui::Rect {
            min: egui::pos2(tool_popover_x_start, tool_popover_y_start),
            max: egui::pos2(tool_popover_x_start + tool_popovers_size.x, tool_popover_y_start),
        };

        ui.allocate_rect(tool_popover_rect, egui::Sense::click());
        if self.show_tool_popover {
            let tool_popover = ui.allocate_ui_at_rect(tool_popover_rect, |ui| {
                egui::Frame::window(ui.style()).show(ui, |ui| {
                    match self.active_tool {
                        Tool::Pen => show_pen_popover(ui, &mut self.pen, tlbr_ctx),
                        Tool::Eraser => self.show_eraser_popover(ui),
                        Tool::Highlighter => {
                            show_highlighter_popover(ui, &mut self.highlighter, tlbr_ctx)
                        }
                        Tool::Selection => {}
                    };
                })
            });

            self.layout.tool_popover = Some(tool_popover.response.rect);
            Some(tool_popover.response)
        } else {
            None
        }
    }

    pub fn hide_tool_popover(
        &mut self, canvas_settings: &mut CanvasSettings, cfg: &mut WsPersistentStore,
    ) {
        self.show_tool_popover = false;

        let color = self.pen.active_color.light;
        canvas_settings.pen = PenSettings {
            color: egui::Color32::from_rgb(color.red, color.green, color.blue),
            width: self.pen.active_stroke_width,
            opacity: self.pen.active_opacity,
            pressure_alpha: self.pen.pressure_alpha,
            has_inf_thick: self.pen.has_inf_thick,
        };

        cfg.set_canvas_settings(*canvas_settings);
    }
    fn show_eraser_popover(&mut self, ui: &mut egui::Ui) {
        let width = 200.0;
        ui.style_mut().spacing.slider_width = width;
        ui.set_width(width);

        let (_, preview_rect) = ui.allocate_space(egui::vec2(ui.available_width(), 100.0));
        let mut painter = ui.painter().to_owned();
        painter.set_clip_rect(preview_rect);

        self.eraser
            .draw_eraser_cursor(ui, &painter, preview_rect.center());

        ui.add_space(20.0);
        show_thickness_slider(
            ui,
            &mut self.eraser.radius,
            DEFAULT_ERASER_RADIUS..=DEFAULT_ERASER_RADIUS * 20.0,
        );
        ui.add_space(10.0);
    }
}
fn show_pen_popover(ui: &mut egui::Ui, pen: &mut Pen, tlbr_ctx: &mut ToolbarContext) {
    let width = 220.0;
    ui.style_mut().spacing.slider_width = width;
    ui.set_width(width);

    show_stroke_preview(ui, pen, tlbr_ctx);
    // a bit hacky but without this there will be collision with
    // thickness hints.
    ui.add_space(20.0);

    show_thickness_slider(ui, &mut pen.active_stroke_width, DEFAULT_PEN_STROKE_WIDTH..=30.0);

    if cfg!(target_os = "ios") {
        ui.add_space(10.0);

        show_pressure_alpha_slider(ui, pen);
    }

    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("Fixed zoom thickness: ");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            switch(ui, &mut pen.has_inf_thick);
        });
    });

    ui.add_space(30.0);

    ui.horizontal_wrapped(|ui| {
        show_color_swatches(ui, get_pen_colors(), pen);
    });

    ui.add_space(10.0);

    show_opacity_slider(ui, pen);

    ui.add_space(10.0);
}

fn show_opacity_slider(ui: &mut egui::Ui, pen: &mut Pen) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Opacity").size(13.0));
        ui.add_space(20.0);
        let slider_color =
            ThemePalette::resolve_dynamic_color(pen.active_color, ui.visuals().dark_mode)
                .linear_multiply(pen.active_opacity);
        ui.visuals_mut().widgets.inactive.bg_fill = slider_color;
        ui.visuals_mut().widgets.inactive.fg_stroke =
            egui::Stroke { width: 1.0, color: slider_color };
        ui.visuals_mut().widgets.hovered.bg_fill = slider_color;
        ui.visuals_mut().widgets.hovered.fg_stroke =
            egui::Stroke { width: 2.0, color: slider_color };
        ui.visuals_mut().widgets.active.bg_fill = slider_color;
        ui.visuals_mut().widgets.active.fg_stroke =
            egui::Stroke { width: 2.5, color: slider_color };
        ui.spacing_mut().slider_width = ui.available_width();
        ui.spacing_mut().slider_rail_height = 2.0;
        ui.add(egui::Slider::new(&mut pen.active_opacity, 0.01..=1.0).show_value(false));
    });
}

fn show_pressure_alpha_slider(ui: &mut egui::Ui, pen: &mut Pen) {
    ui.label(RichText::new("Pressure Sensitivity").size(13.0));
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut pen.pressure_alpha, 0.0..=1.0).show_value(false));
    });
}

fn show_highlighter_popover(ui: &mut egui::Ui, pen: &mut Pen, tlbr_ctx: &mut ToolbarContext) {
    let width = 200.0;
    ui.style_mut().spacing.slider_width = width;
    ui.set_width(width);

    show_stroke_preview(ui, pen, tlbr_ctx);

    // a bit hacky but without this there will be collision with
    // thickness hints.
    ui.add_space(20.0);

    show_thickness_slider(
        ui,
        &mut pen.active_stroke_width,
        DEFAULT_HIGHLIGHTER_STROKE_WIDTH..=40.0,
    );

    ui.add_space(10.0);

    ui.horizontal_wrapped(|ui| {
        show_color_swatches(ui, get_highlighter_colors(), pen);
    });

    ui.add_space(10.0);
}

fn show_color_swatches(ui: &mut egui::Ui, colors: Vec<DynamicColor>, pen: &mut Pen) {
    colors.iter().for_each(|c| {
        let color = ThemePalette::resolve_dynamic_color(*c, ui.visuals().dark_mode);
        let active_color =
            ThemePalette::resolve_dynamic_color(pen.active_color, ui.visuals().dark_mode);
        let color_btn = show_color_btn(ui, color, active_color);
        if color_btn.clicked() || color_btn.drag_started() {
            pen.active_color = *c;
        }
    });
}

fn show_color_btn(
    ui: &mut egui::Ui, color: egui::Color32, active_color: egui::Color32,
) -> egui::Response {
    let circle_diameter = COLOR_SWATCH_BTN_RADIUS * 2.0;
    let margin = 6.0;
    let (id, rect) =
        ui.allocate_space(egui::vec2(circle_diameter + margin, circle_diameter + margin));

    ui.painter()
        .circle_filled(rect.center(), COLOR_SWATCH_BTN_RADIUS, color);

    if get_non_additive(&active_color).eq(&color) {
        ui.painter().circle_stroke(
            rect.center(),
            COLOR_SWATCH_BTN_RADIUS - 3.0,
            egui::Stroke { width: 1.5, color: ui.visuals().extreme_bg_color },
        );
    }
    ui.interact(rect, id, egui::Sense::click_and_drag())
}

fn show_stroke_preview(ui: &mut egui::Ui, pen: &mut Pen, tlbr_ctx: &mut ToolbarContext) {
    let (res, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), 100.0),
        egui::Sense::focusable_noninteractive(),
    );
    let preview_rect = res.rect;

    let mut bez =
        bezier_rs::Bezier::from_cubic_coordinates(146., 162., 272.0, 239., 215., 68., 329., 148.);
    let path_rect = bb_to_rect(bez.bounding_box());
    if let Some(t) =
        get_rect_identity_transform(preview_rect, path_rect, 0.7, preview_rect.center())
    {
        bez = bez.apply_transformation(|p| DVec2 {
            x: t.sx as f64 * p.x + t.tx as f64,
            y: t.sy as f64 * p.y + t.ty as f64,
        });
    }

    let mut fill_tess = FillTessellator::new();

    let mut builder = lyon::path::Builder::new();
    let stroke_color =
        ThemePalette::resolve_dynamic_color(pen.active_color, ui.visuals().dark_mode)
            .linear_multiply(pen.active_opacity);

    let mut thickness = pen.active_stroke_width;
    if !pen.has_inf_thick {
        thickness *= tlbr_ctx.viewport_settings.master_transform.sx;
    }

    let subapth: Subpath<ManipulatorGroupId> = bez.graduated_outline(
        (thickness + thickness * pen.pressure_alpha) as f64,
        (thickness - thickness * pen.pressure_alpha) as f64,
        Cap::Round,
    );

    let mut i = 0;
    let mut first = None;
    while let Some(seg) = subapth.get_segment(i) {
        let start = devc_to_point(seg.start());
        let end = devc_to_point(seg.end());
        if first.is_none() {
            first = Some(start);
            builder.begin(start);
        }
        if seg.handle_end().is_some() && seg.handle_start().is_some() {
            let handle_start = devc_to_point(seg.handle_start().unwrap());
            let handle_end = devc_to_point(seg.handle_end().unwrap());

            builder.cubic_bezier_to(handle_start, handle_end, end);
        } else if seg.handle_end().is_none() && seg.handle_start().is_none() {
            builder.line_to(end);
        }
        i += 1;
    }
    if first.is_some() {
        builder.end(true);
    }

    let path = builder.build();

    let mut mesh: VertexBuffers<_, u32> = VertexBuffers::new();

    let _ = fill_tess.tessellate_path(
        &path,
        &FillOptions::DEFAULT,
        &mut BuffersBuilder::new(&mut mesh, VertexConstructor { color: stroke_color }),
    );

    let mesh = egui::epaint::Mesh {
        indices: mesh.indices.clone(),
        vertices: mesh.vertices.clone(),
        texture_id: Default::default(),
    };

    painter.add(egui::Shape::Mesh(mesh));
}

fn show_thickness_slider(ui: &mut egui::Ui, value: &mut f32, value_range: RangeInclusive<f32>) {
    let width = ui.available_width();
    let slider_rect = ui
        .add(
            egui::Slider::new(value, value_range.clone())
                .show_value(false)
                .step_by(1.0)
                .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.5 }),
        )
        .rect;

    let middle_range = value_range.start() + (value_range.end() - value_range.start()).abs() / 2.0;
    let ticks = [value_range.start(), &middle_range, value_range.end()];

    for (i, t) in ticks.iter().enumerate() {
        let margin = egui::vec2(2.0, 10.0);
        let end_y = slider_rect.top() - margin.y + (i as f32 * 3.0 + 1.0);

        let total_spacing = width - (THICKNESS_BTN_WIDTH * ticks.len() as f32);
        let spacing_between = total_spacing / (ticks.len() as f32 + 1.0);

        let rect_start_x = slider_rect.left()
            + spacing_between
            + i as f32 * (THICKNESS_BTN_WIDTH + spacing_between);

        let rect = match i {
            0 => egui::Rect {
                min: egui::pos2(slider_rect.left() + margin.x, slider_rect.top() - margin.y),
                max: egui::pos2(slider_rect.left() + margin.x + THICKNESS_BTN_WIDTH, end_y),
            },
            1 => egui::Rect {
                min: egui::pos2(rect_start_x, slider_rect.top() - margin.y),
                max: egui::pos2(rect_start_x + THICKNESS_BTN_WIDTH, end_y),
            },
            2 => egui::Rect {
                min: egui::pos2(
                    slider_rect.right() - margin.x - THICKNESS_BTN_WIDTH,
                    slider_rect.top() - margin.y,
                ),
                max: egui::pos2(slider_rect.right() - margin.x, end_y),
            },
            _ => break,
        };

        let response = ui.allocate_rect(rect.expand2(egui::vec2(0.0, 5.0)), egui::Sense::click());

        if t.eq(&value) {
            ui.painter().rect_filled(
                rect.expand(5.0),
                egui::Rounding::same(8.0),
                egui::Color32::GRAY.linear_multiply(0.1),
            );
        }

        ui.painter().rect_filled(
            rect,
            egui::Rounding::same(2.0),
            ui.visuals().text_color().linear_multiply(0.8),
        );

        if response.clicked() {
            *value = **t;
        }
    }
    ui.advance_cursor_after_rect(slider_rect);
}

fn get_non_additive(color: &egui::Color32) -> egui::Color32 {
    egui::Color32::from_rgb(color.r(), color.g(), color.b())
}
