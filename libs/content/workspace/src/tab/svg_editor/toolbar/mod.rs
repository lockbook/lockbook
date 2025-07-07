mod history_island;
mod tools_island;
mod viewport_island;

use crate::{
    tab::svg_editor::{
        gesture_handler::transform_canvas, renderer::RenderOptions, util::transform_rect,
        InputContext,
    },
    theme::icons::Icon,
    widgets::Button,
    workspace::WsPersistentStore,
};
use lb_rs::model::svg::buffer::Buffer;
use resvg::usvg::Transform;
use viewport_island::ViewportPopover;

use super::{
    gesture_handler::GestureHandler, history::History, pen::PenSettings, renderer::Renderer,
    selection::Selection, CanvasSettings, Eraser, Pen, ViewportSettings,
};

const COLOR_SWATCH_BTN_RADIUS: f32 = 11.0;
const THICKNESS_BTN_WIDTH: f32 = 25.0;
const SCREEN_PADDING: f32 = 20.0;
const MINI_MAP_WIDTH: f32 = 100.0;

pub struct Toolbar {
    pub active_tool: Tool,
    pub pen: Pen,
    pub highlighter: Pen,
    pub eraser: Eraser,
    pub selection: Selection,
    pub previous_tool: Option<Tool>,
    pub gesture_handler: GestureHandler,

    hide_overlay: bool,
    pub show_tool_popover: bool,
    pub show_at_cursor_tool_popover: Option<Option<egui::Pos2>>,
    layout: ToolbarLayout,
    pub viewport_popover: Option<ViewportPopover>,
    renderer: Renderer,
}

#[derive(Default)]
struct ToolbarLayout {
    tools_island: Option<egui::Rect>,
    history_island: Option<egui::Rect>,
    viewport_island: Option<egui::Rect>,
    viewport_popover: Option<egui::Rect>,
    tool_popover: Option<egui::Rect>,
    zoom_pct_btn: Option<egui::Rect>,
    zoom_stops_popover: Option<egui::Rect>,
    overlay_toggle: Option<egui::Rect>,
}
#[derive(PartialEq, Eq, Copy, Clone, Debug, Default)]
pub enum Tool {
    #[default]
    Pen,
    Eraser,
    Selection,
    Highlighter,
}

pub struct ToolContext<'a> {
    pub painter: &'a mut egui::Painter,
    pub buffer: &'a mut Buffer,
    pub history: &'a mut History,
    pub allow_viewport_changes: &'a mut bool,
    pub is_touch_frame: bool,
    pub settings: &'a mut CanvasSettings,
    pub is_locked_vw_pen_only: bool,
    pub viewport_settings: &'a mut ViewportSettings,
}

pub struct ToolbarContext<'a> {
    pub painter: &'a mut egui::Painter,
    pub buffer: &'a mut Buffer,
    pub history: &'a mut History,
    pub settings: &'a mut CanvasSettings,
    pub viewport_settings: &'a mut ViewportSettings,
    pub cfg: &'a mut WsPersistentStore,
    pub input_ctx: &'a InputContext,
}

pub enum ViewportMode {
    Page,
    Scroll,
    Timeline,
    Infinite,
}

impl ViewportMode {
    pub fn variants() -> [ViewportMode; 4] {
        [ViewportMode::Page, ViewportMode::Scroll, ViewportMode::Timeline, ViewportMode::Infinite]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ViewportMode::Page => "Page",
            ViewportMode::Scroll => "Scroll",
            ViewportMode::Timeline => "Timeline",
            ViewportMode::Infinite => "Infinite",
        }
    }

    pub fn is_active(&self, tlbr_ctx: &ToolbarContext) -> bool {
        match self {
            ViewportMode::Page => tlbr_ctx.viewport_settings.is_page_mode(),
            ViewportMode::Scroll => tlbr_ctx.viewport_settings.is_scroll_mode(),
            ViewportMode::Timeline => tlbr_ctx.viewport_settings.is_timeline_mode(),
            ViewportMode::Infinite => tlbr_ctx.viewport_settings.is_infinite_mode(),
        }
    }

    pub fn set_active(&self, tlbr_ctx: &mut ToolbarContext) {
        match self {
            ViewportMode::Page => tlbr_ctx.viewport_settings.set_page_mode(),
            ViewportMode::Scroll => tlbr_ctx.viewport_settings.set_scroll_mode(),
            ViewportMode::Timeline => tlbr_ctx.viewport_settings.set_timeline_mode(),
            ViewportMode::Infinite => tlbr_ctx.viewport_settings.set_infinite_mode(),
        }
    }
}

impl ViewportSettings {
    pub fn update_working_rect(&mut self, settings: CanvasSettings) {
        let new_working_rect = if let Some(bounded_rect) = self.bounded_rect {
            let min_x = if self.left_locked {
                bounded_rect.left().max(self.container_rect.left())
            } else {
                self.container_rect.left()
            };

            let min_y = if self.top_locked {
                bounded_rect.top().max(self.container_rect.top())
            } else {
                self.container_rect.top()
            };

            let mini_map_width = if settings.show_mini_map { MINI_MAP_WIDTH } else { 0.0 };

            let max_x = if self.right_locked {
                bounded_rect
                    .right()
                    .min(self.container_rect.right() - mini_map_width)
            } else {
                self.container_rect.right()
            };

            let max_y = if self.bottom_locked {
                bounded_rect.bottom().min(self.container_rect.bottom())
            } else {
                self.container_rect.bottom()
            };

            egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
        } else {
            self.container_rect
        };

        self.working_rect = new_working_rect;
    }
    pub fn is_page_mode(&self) -> bool {
        self.bottom_locked && self.left_locked && self.right_locked && self.top_locked
    }

    pub fn set_page_mode(&mut self) {
        self.bottom_locked = true;
        self.left_locked = true;
        self.right_locked = true;
        self.top_locked = true;
    }

    pub fn is_timeline_mode(&self) -> bool {
        self.top_locked && self.bottom_locked && self.left_locked && !self.right_locked
    }

    pub fn set_timeline_mode(&mut self) {
        self.top_locked = true;
        self.bottom_locked = true;
        self.left_locked = true;
        self.right_locked = false;
    }

    pub fn is_scroll_mode(&self) -> bool {
        self.top_locked && self.right_locked && !self.bottom_locked && self.left_locked
    }

    pub fn set_scroll_mode(&mut self) {
        self.top_locked = true;
        self.right_locked = true;
        self.left_locked = true;
        self.bottom_locked = false;
    }

    pub fn is_infinite_mode(&self) -> bool {
        !self.top_locked && !self.right_locked && !self.bottom_locked && !self.left_locked
    }

    pub fn set_infinite_mode(&mut self) {
        self.top_locked = false;
        self.right_locked = false;
        self.left_locked = false;
        self.bottom_locked = false;
    }
}
#[macro_export]
macro_rules! set_tool {
    ($obj:expr, $new_tool:expr) => {
        if $obj.active_tool != $new_tool {
            $obj.show_tool_popover = false;
            $obj.layout.tool_popover = None;

            if (matches!($new_tool, Tool::Selection)) {
                $obj.selection = $crate::tab::svg_editor::selection::Selection::default();
            }
            $obj.previous_tool = Some($obj.active_tool);
            $obj.active_tool = $new_tool;
        } else {
            if $obj.show_tool_popover == true {
                $obj.show_tool_popover = false;
            } else {
                $obj.show_tool_popover = true;
            }
        }
    };
}

impl Toolbar {
    pub fn set_tool(
        &mut self, new_tool: Tool, settings: &mut CanvasSettings, cfg: &mut WsPersistentStore,
    ) {
        set_tool!(self, new_tool);
        if !self.show_tool_popover {
            self.hide_tool_popover(settings, cfg);
        }
    }

    pub fn toggle_tool_between_eraser(
        &mut self, settings: &mut CanvasSettings, cfg: &mut WsPersistentStore,
    ) {
        let new_tool = if self.active_tool == Tool::Eraser {
            self.previous_tool.unwrap_or(Tool::Pen)
        } else {
            Tool::Eraser
        };

        self.set_tool(new_tool, settings, cfg);
    }

    pub fn new(elements_count: usize, settings: &CanvasSettings) -> Self {
        Toolbar {
            pen: Pen::new(settings.pen),
            highlighter: Pen::new(PenSettings::default_highlighter()),
            renderer: Renderer::new(elements_count),
            active_tool: Default::default(),
            eraser: Default::default(),
            selection: Default::default(),
            previous_tool: Default::default(),
            gesture_handler: Default::default(),
            hide_overlay: Default::default(),
            show_tool_popover: Default::default(),
            layout: Default::default(),
            viewport_popover: Default::default(),
            show_at_cursor_tool_popover: None,
        }
    }

    pub fn show(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext, skip_frame: &mut bool,
    ) {
        self.handle_keyboard_shortcuts(ui, tlbr_ctx);

        let tool_popover_at_cursor = self.show_tool_popovers_at_cursor(ui, tlbr_ctx);

        let opacity = if self.hide_overlay { 0.0 } else { 1.0 };

        ui.set_opacity(opacity);

        let history_island = self.show_history_island(ui, tlbr_ctx);

        let overlay_toggle_res = ui
            .scope(|ui| {
                ui.set_opacity(1.0);
                self.show_overlay_toggle(ui, tlbr_ctx)
            })
            .inner;

        if opacity == 0.0 {
            if overlay_toggle_res.hovered()
                || overlay_toggle_res.clicked()
                || overlay_toggle_res.contains_pointer()
            {
                *skip_frame = true;
            }
            return;
        }

        let mini_map_res = self.show_mini_map_v2(ui, tlbr_ctx);

        // shows the viewport island + popovers + bring home button
        let viewport_controls = self.show_viewport_controls(ui, tlbr_ctx);

        let tools_island = self.show_tools_island(ui);
        let tool_controls_res = self.show_tool_popovers(ui, tlbr_ctx);

        let mut overlay_res = history_island;
        if let Some(res) = tool_popover_at_cursor {
            overlay_res = overlay_res.union(res);
        }
        if let Some(res) = mini_map_res {
            overlay_res = overlay_res.union(res);
        }

        if let Some(res) = tool_controls_res {
            overlay_res = overlay_res.union(res);
        }
        if let Some(res) = viewport_controls {
            overlay_res = overlay_res.union(res);
        }

        overlay_res = overlay_res
            .union(tools_island.inner.response)
            .union(overlay_toggle_res);

        if overlay_res.hovered() || overlay_res.clicked() || overlay_res.contains_pointer() {
            *skip_frame = true;
        }
    }

    fn show_mini_map_v2(
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

        /**
         * okay let's figure out this transforming math.
         * so given a viewport with size x we would like to fit it into size_x_1
         * this would give us a multiplier factor
         * now for y, we would like to make it such that the top of the viewport y is the top of some other y_1
         * thus applying a correction tion translation
         */
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

        let transformed_bounded_rect = transform_rect(bounded_rect, viewport_transform);

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

        // self.renderer
        //     .render_svg(&mut ui, buffer, painter, render_options, master_transform);
        None
    }

    fn handle_keyboard_shortcuts(&mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext) {
        if ui.input_mut(|r| {
            r.consume_key(egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT), egui::Key::Z)
        }) {
            tlbr_ctx.history.redo(tlbr_ctx.buffer);
        }

        if ui.input_mut(|r| r.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            tlbr_ctx.history.undo(tlbr_ctx.buffer);
        }

        if ui.input(|r| r.key_pressed(egui::Key::E)) {
            set_tool!(self, Tool::Eraser);
        }

        if ui.input(|r| r.key_pressed(egui::Key::S)) {
            set_tool!(self, Tool::Selection);
        }

        if ui.input(|r| r.key_pressed(egui::Key::B)) {
            set_tool!(self, Tool::Pen);
        }

        if ui.input(|r| r.key_pressed(egui::Key::Tab)) {
            self.toggle_at_cursor_tool_popover();
        }
    }

    pub fn toggle_at_cursor_tool_popover(&mut self) {
        // If there's a popover then hide it. If there's no popover then show it.
        self.show_at_cursor_tool_popover =
            if self.show_at_cursor_tool_popover.is_some() { None } else { Some(None) };
    }

    fn show_overlay_toggle(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> egui::Response {
        let island_size = self
            .layout
            .overlay_toggle
            .unwrap_or(egui::Rect::from_min_size(egui::Pos2::default(), egui::vec2(10.0, 10.0)))
            .size();

        let mini_map_width = if tlbr_ctx.settings.show_mini_map { MINI_MAP_WIDTH } else { 0.0 };
        let island_rect = egui::Rect {
            min: egui::pos2(
                tlbr_ctx.viewport_settings.container_rect.right()
                    - SCREEN_PADDING
                    - island_size.x
                    - mini_map_width,
                tlbr_ctx.viewport_settings.container_rect.top() + SCREEN_PADDING,
            ),
            max: egui::pos2(
                tlbr_ctx.viewport_settings.container_rect.right() - SCREEN_PADDING - mini_map_width,
                tlbr_ctx.viewport_settings.container_rect.top() + SCREEN_PADDING + island_size.y,
            ),
        };
        let overlay_toggle = ui.allocate_ui_at_rect(island_rect, |ui| {
            egui::Frame::window(ui.style())
                .inner_margin(egui::Margin::symmetric(7.5, 3.5))
                .show(ui, |ui| {
                    let icon =
                        if self.hide_overlay { Icon::FULLSCREEN_EXIT } else { Icon::FULLSCREEN };
                    let toggle_btn = Button::default().icon(&icon).show(ui);
                    if toggle_btn.clicked() || toggle_btn.drag_started() {
                        self.hide_overlay = !self.hide_overlay;
                    }
                })
        });

        self.layout.overlay_toggle = Some(overlay_toggle.response.rect);
        overlay_toggle.response
    }
}
