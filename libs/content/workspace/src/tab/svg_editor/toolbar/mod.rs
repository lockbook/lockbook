mod history_island;
mod tools_island;
mod viewport_island;

use crate::{theme::icons::Icon, widgets::Button};
use lb_rs::model::svg::buffer::{get_highlighter_colors, get_pen_colors, Buffer};
use resvg::usvg::Transform;
use viewport_island::ViewportPopover;

use super::{
    gesture_handler::{calc_elements_bounds, GestureHandler},
    history::History,
    pen::{DEFAULT_HIGHLIGHTER_STROKE_WIDTH, DEFAULT_PEN_STROKE_WIDTH},
    renderer::Renderer,
    selection::Selection,
    CanvasSettings, Eraser, Pen,
};

const COLOR_SWATCH_BTN_RADIUS: f32 = 11.0;
const THICKNESS_BTN_WIDTH: f32 = 25.0;
const SCREEN_PADDING: f32 = 20.0;

pub struct Toolbar {
    pub active_tool: Tool,
    pub pen: Pen,
    pub highlighter: Pen,
    pub eraser: Eraser,
    pub selection: Selection,
    pub previous_tool: Option<Tool>,
    pub gesture_handler: GestureHandler,

    hide_overlay: bool,
    pub show_tool_controls: bool,
    layout: ToolbarLayout,
    pub viewport_popover: Option<ViewportPopover>,
    viewport_transform: Option<Transform>,
    renderer: Renderer,
}

#[derive(Default)]
struct ToolbarLayout {
    tools_island: Option<egui::Rect>,
    history_island: Option<egui::Rect>,
    viewport_island: Option<egui::Rect>,
    viewport_popover: Option<egui::Rect>,
    tool_controls: Option<egui::Rect>,
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
    pub inner_rect: &'a mut BoundedRect,
    pub container_rect: egui::Rect,
}

pub struct ToolbarContext<'a> {
    pub painter: &'a mut egui::Painter,
    pub buffer: &'a mut Buffer,
    pub history: &'a mut History,
    pub settings: &'a mut CanvasSettings,
    pub inner_rect: &'a mut BoundedRect,
    pub container_rect: egui::Rect,
}

#[derive(Clone, Copy)]
pub struct BoundedRect {
    /// takes into consideration the screen size and gives the drawable area
    pub working_rect: egui::Rect,
    /// the drawable rect in absolute space, no consideration to screen size
    pub bounded_rect: egui::Rect,
    pub left_locked: bool,
    pub right_locked: bool,
    pub bottom_locked: bool,
    pub top_locked: bool,
}

impl Default for BoundedRect {
    fn default() -> Self {
        Self {
            working_rect: egui::Rect::ZERO,
            bounded_rect: egui::Rect::ZERO,
            left_locked: false,
            right_locked: false,
            bottom_locked: false,
            top_locked: false,
        }
    }
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
            ViewportMode::Page => tlbr_ctx.inner_rect.is_page_mode(),
            ViewportMode::Scroll => tlbr_ctx.inner_rect.is_scroll_mode(),
            ViewportMode::Timeline => tlbr_ctx.inner_rect.is_timeline_mode(),
            ViewportMode::Infinite => tlbr_ctx.inner_rect.is_infinite_mode(),
        }
    }

    pub fn set_active(&self, tlbr_ctx: &mut ToolbarContext) {
        match self {
            ViewportMode::Page => tlbr_ctx.inner_rect.set_page_mode(),
            ViewportMode::Scroll => tlbr_ctx.inner_rect.set_scroll_mode(),
            ViewportMode::Timeline => tlbr_ctx.inner_rect.set_timeline_mode(),
            ViewportMode::Infinite => tlbr_ctx.inner_rect.set_infinite_mode(),
        }
    }
}

impl BoundedRect {
    pub fn update(&mut self, container_rect: egui::Rect, buffer: &Buffer) {
        if self.is_infinite_mode() {
            if let Some(rect) = calc_elements_bounds(buffer) {
                self.bounded_rect = rect;
            }
        }

        let min_x = if self.left_locked {
            self.bounded_rect.left().max(container_rect.left())
        } else {
            container_rect.left()
        };

        let min_y = if self.top_locked {
            self.bounded_rect.top().max(container_rect.top())
        } else {
            container_rect.top()
        };

        let max_x = if self.right_locked {
            self.bounded_rect.right().min(container_rect.right())
        } else {
            container_rect.right()
        };

        let max_y = if self.bottom_locked {
            self.bounded_rect.bottom().min(container_rect.bottom())
        } else {
            container_rect.bottom()
        };

        self.working_rect =
            egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y));
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
            $obj.show_tool_controls = false;
            $obj.layout.tool_controls = None;

            if (matches!($new_tool, Tool::Selection)) {
                $obj.selection = crate::tab::svg_editor::selection::Selection::default();
            }
            $obj.previous_tool = Some($obj.active_tool);
            $obj.active_tool = $new_tool;
        } else {
            if $obj.show_tool_controls == true {
                $obj.show_tool_controls = false;
            } else {
                $obj.show_tool_controls = true;
            }
        }
    };
}

impl Toolbar {
    pub fn set_tool(&mut self, new_tool: Tool) {
        set_tool!(self, new_tool);
    }

    pub fn toggle_tool_between_eraser(&mut self) {
        let new_tool = if self.active_tool == Tool::Eraser {
            self.previous_tool.unwrap_or(Tool::Pen)
        } else {
            Tool::Eraser
        };

        self.set_tool(new_tool);
    }

    pub fn new(elements_count: usize) -> Self {
        let mut toolbar = Toolbar {
            pen: Pen::new(get_pen_colors()[0], DEFAULT_PEN_STROKE_WIDTH),
            highlighter: Pen::new(get_highlighter_colors()[0], DEFAULT_HIGHLIGHTER_STROKE_WIDTH),
            renderer: Renderer::new(elements_count),
            active_tool: Default::default(),
            eraser: Default::default(),
            selection: Default::default(),
            previous_tool: Default::default(),
            gesture_handler: Default::default(),
            hide_overlay: Default::default(),
            show_tool_controls: Default::default(),
            layout: Default::default(),
            viewport_popover: Default::default(),
            viewport_transform: None,
        };

        toolbar.highlighter.active_opacity = 0.1;
        toolbar.pen.active_opacity = 1.0;
        toolbar
    }

    pub fn show(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext, skip_frame: &mut bool,
    ) {
        self.handle_keyboard_shortcuts(ui, tlbr_ctx.history, tlbr_ctx.buffer);

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

        let viewport_island = self.show_viewport_island(ui, tlbr_ctx);

        let tools_island = self.show_tools_island(ui);
        let tool_controls_res = self.show_tool_controls(ui, tlbr_ctx);

        let mut overlay_res = history_island;
        if let Some(res) = tool_controls_res {
            overlay_res = overlay_res.union(res);
        }
        if let Some(res) = viewport_island {
            overlay_res = overlay_res.union(res);
        }
        overlay_res = overlay_res
            .union(tools_island.inner.response)
            .union(overlay_toggle_res);

        if overlay_res.hovered() || overlay_res.clicked() || overlay_res.contains_pointer() {
            *skip_frame = true;
        }
    }

    fn handle_keyboard_shortcuts(
        &mut self, ui: &mut egui::Ui, history: &mut History, buffer: &mut Buffer,
    ) {
        if ui.input_mut(|r| {
            r.consume_key(egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT), egui::Key::Z)
        }) {
            history.redo(buffer);
        }

        if ui.input_mut(|r| r.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            history.undo(buffer);
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
    }

    fn show_overlay_toggle(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> egui::Response {
        let island_size = self
            .layout
            .overlay_toggle
            .unwrap_or(egui::Rect::from_min_size(egui::Pos2::default(), egui::vec2(10.0, 10.0)))
            .size();

        let island_rect = egui::Rect {
            min: egui::pos2(
                tlbr_ctx.container_rect.right() - SCREEN_PADDING - island_size.x,
                tlbr_ctx.container_rect.top() + SCREEN_PADDING,
            ),
            max: egui::pos2(
                tlbr_ctx.container_rect.right() - SCREEN_PADDING,
                tlbr_ctx.container_rect.top() + SCREEN_PADDING + island_size.y,
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
