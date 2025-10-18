mod history_island;
mod mini_map;
mod tools_island;
mod viewport_island;

use crate::tab::svg_editor::gesture_handler::calc_elements_bounds;
use crate::tab::svg_editor::shapes::ShapesTool;
use crate::tab::svg_editor::{InputContext, SVGEditor};
use crate::theme::icons::Icon;
use crate::widgets::Button;
use crate::workspace::WsPersistentStore;
use lb_rs::model::svg::buffer::Buffer;
use lb_rs::model::svg::diff::DiffState;
use viewport_island::ViewportPopover;

use super::gesture_handler::GestureHandler;
use super::history::History;
use super::pen::PenSettings;
use super::renderer::Renderer;
use super::selection::Selection;
use super::{CanvasSettings, Eraser, Pen, ViewportSettings};
pub const MINI_MAP_WIDTH: f32 = 100.0;

const COLOR_SWATCH_BTN_RADIUS: f32 = 11.0;
const THICKNESS_BTN_WIDTH: f32 = 25.0;
const SCREEN_PADDING: f32 = 20.0;

pub struct Toolbar {
    pub active_tool: Tool,
    pub pen: Pen,
    pub highlighter: Pen,
    pub eraser: Eraser,
    pub selection: Selection,
    pub shapes_tool: ShapesTool,
    pub previous_tool: Option<Tool>,
    pub gesture_handler: GestureHandler,

    pub hide_overlay: bool,
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
    mini_map: Option<egui::Rect>,
}
#[derive(PartialEq, Eq, Copy, Clone, Debug, Default)]
pub enum Tool {
    #[default]
    Pen,
    Eraser,
    Selection,
    Highlighter,
    Shapes,
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

impl ViewportSettings {
    pub fn update_working_rect(
        &mut self, settings: CanvasSettings, buffer: &Buffer, diff_state: &DiffState,
        hide_overlay: bool,
    ) {
        let is_scroll_mode = self.is_scroll_mode();
        let new_working_rect = if let Some(bounded_rect) = &mut self.bounded_rect {
            if diff_state.is_dirty() && diff_state.transformed.is_none() {
                if let Some(elements_bounds) = calc_elements_bounds(buffer) {
                    if is_scroll_mode {
                        bounded_rect.max.y = elements_bounds.max.y
                    } else {
                        *bounded_rect = elements_bounds;
                    }
                }
            }

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

            let mini_map_width = if settings.show_mini_map && is_scroll_mode && !hide_overlay {
                MINI_MAP_WIDTH
            } else {
                0.0
            };

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
            shapes_tool: Default::default(),
        }
    }

    pub fn show(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
        has_islands_interaction: &mut bool,
    ) -> bool {
        let mut res = self.handle_keyboard_shortcuts(ui, tlbr_ctx);

        let tool_popover_at_cursor = self.show_tool_popovers_at_cursor(ui, tlbr_ctx);

        let opacity = if self.hide_overlay { 0.0 } else { 1.0 };

        ui.set_opacity(opacity);

        let (history_island, history_dirty) = self.show_history_island(ui, tlbr_ctx);
        if history_dirty {
            res = true;
        }

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
                *has_islands_interaction = true;
            }
            return res;
        }

        let (mini_map_dirty, mini_map_res) = self.show_mini_map(ui, tlbr_ctx);

        // shows the viewport island + popovers + bring home button
        let viewport_controls = self.show_viewport_controls(ui, tlbr_ctx);

        let tools_island = self.show_tools_island(ui);
        let tool_controls_res = self.show_tool_popovers(ui, tlbr_ctx);

        if is_pointer_over_res(ui, &history_island) {
            *has_islands_interaction = true;
        }

        if let Some(res) = tool_popover_at_cursor {
            if is_pointer_over_res(ui, &res) {
                *has_islands_interaction = true;
            }
        }
        if let Some(res) = mini_map_res {
            if is_pointer_over_res(ui, &res) {
                *has_islands_interaction = true;
            }
        }

        if mini_map_dirty {
            res = true;
            *has_islands_interaction = true;
        }

        if let Some(res) = tool_controls_res {
            if is_pointer_over_res(ui, &res) {
                *has_islands_interaction = true;
            }
        }
        if let Some(res) = viewport_controls {
            if is_pointer_over_res(ui, &res) {
                *has_islands_interaction = true;
            }
        }

        if is_pointer_over_res(ui, &tools_island.inner.response) {
            *has_islands_interaction = true;
        }

        if is_pointer_over_res(ui, &overlay_toggle_res) {
            *has_islands_interaction = true;
        }
        res
    }

    fn handle_keyboard_shortcuts(
        &mut self, ui: &mut egui::Ui, tlbr_ctx: &mut ToolbarContext,
    ) -> bool {
        let mut res = false;

        if ui.input_mut(|r| {
            r.consume_key(egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT), egui::Key::Z)
        }) {
            tlbr_ctx.history.redo(tlbr_ctx.buffer);
            res = true;
        }

        if ui.input_mut(|r| r.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            tlbr_ctx.history.undo(tlbr_ctx.buffer);
            res = true;
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
        res
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

        let mini_map_width = if tlbr_ctx.settings.show_mini_map
            && tlbr_ctx.viewport_settings.is_scroll_mode()
            && !self.hide_overlay
        {
            MINI_MAP_WIDTH
        } else {
            0.0
        };
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

impl SVGEditor {
    pub fn detect_islands_interaction(&self, pointer: egui::Pos2) -> bool {
        let islands = [
            self.toolbar.layout.history_island,
            self.toolbar.layout.overlay_toggle,
            self.toolbar.layout.tool_popover,
            self.toolbar.layout.tools_island,
            self.toolbar.layout.zoom_pct_btn,
            self.toolbar.layout.zoom_stops_popover,
            self.toolbar.layout.viewport_island,
            self.toolbar.layout.viewport_popover,
            self.toolbar.layout.mini_map,
        ];
        for island in islands.iter() {
            if island.unwrap_or(egui::Rect::ZERO).contains(pointer) {
                return true;
            }
        }

        false
    }
}

fn is_pointer_over_res(ui: &mut egui::Ui, overlay_res: &egui::Response) -> bool {
    ui.input(|r| {
        for ev in r.events.iter() {
            let temp = match ev {
                egui::Event::PointerMoved(pos2) => overlay_res.rect.contains(*pos2),
                egui::Event::PointerButton { pos, .. } => overlay_res.rect.contains(*pos),
                egui::Event::Touch { pos, .. } => overlay_res.rect.contains(*pos),
                _ => false,
            };
            if temp {
                return true;
            }
        }
        false
    })
}
