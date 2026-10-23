use std::mem;
use std::sync::Arc;
use web_time::{Duration, Instant};

use comrak::Arena;
use comrak::nodes::{AstNode, ListType, NodeHeading, NodeList, NodeValue, TableAlignment};
use egui::scroll_area::{ScrollBarVisibility, ScrollSource};
use egui::{
    Color32, FontId, Frame, Label, Layout, Margin, Pos2, Rect, Response, RichText, ScrollArea,
    Separator, Stroke, StrokeKind, Ui, UiBuilder, Vec2, Widget,
};
use lb_rs::model::text::offset_types::{IntoRangeExt, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;
use serde::{Deserialize, Serialize};

use crate::style::{
    CHROME_BAND_GLYPH, CHROME_BAND_H, STROKE_HAIRLINE, ThemeExt, control_height, icon_button_glyph,
    icon_button_mark, phosphor, phosphor_font_id, place_at, tip_text,
};
use crate::tab::markdown_editor::table::TableAction;
use crate::tab::markdown_editor::widget::utils::NodeValueExt;
use crate::tab::markdown_editor::{MdRender, TouchTarget};
use crate::tab::{ExtendedInput as _, ExtendedOutput as _};

use crate::tab::markdown_editor::{self, Editor};
use markdown_editor::Event;
use markdown_editor::input::Region;

pub const MOBILE_TOOL_BAR_SIZE: f32 = CHROME_BAND_H;
pub const MENU_SPACE: f32 = 20.; // space used for separators between menu sections
pub const MENU_MARGIN: f32 = 20.; // space on left and right side

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum MarkdownToolbarAction {
    Undo,
    Redo,
    Heading,
    Bold,
    Italic,
    Code,
    Strikethrough,
    Highlight,
    Underline,
    Spoiler,
    Subscript,
    Superscript,
    NumberedList,
    BulletedList,
    TaskList,
    Quote,
    Link,
    Image,
    Indent,
    Outdent,
    /// Inserts a table.
    Table,
    // the table set, for the table the selection ends in
    PreviousCell,
    NextCell,
    NextRow,
    RowAbove,
    RowBelow,
    ColumnLeft,
    ColumnRight,
    DeleteRow,
    DeleteColumn,
    MoveRowUp,
    MoveRowDown,
    MoveColumnLeft,
    MoveColumnRight,
    AlignLeft,
    AlignCenter,
    AlignRight,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownToolbarState {
    pub active: Vec<MarkdownToolbarAction>,
    pub heading_level: Option<u8>,
    /// Whether the selection ends in a table: the table set stands in for
    /// the block styles.
    pub in_table: bool,
    /// Tools of the table set that would do nothing where the selection is.
    pub disabled: Vec<MarkdownToolbarAction>,
    /// Whether the selection is in the header, where Delete row deletes the table.
    pub on_header: bool,
}

/// A tool of the table set, as it stands where the selection ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableTool {
    pub action: MarkdownToolbarAction,
    /// Marked as the current choice, as a column's alignment is.
    pub applied: bool,
    /// False where the tool would do nothing.
    pub enabled: bool,
    /// What it does where the selection is.
    pub tip: &'static str,
}

fn toolbar_icon(
    ui: &mut Ui, t: &crate::style::Theme, icon: &'static str, applied: bool, menu_open: bool,
    tip: &str,
) -> bool {
    let r = icon_button_glyph(
        ui,
        t,
        icon,
        applied && !menu_open,
        t.neutral_bg(),
        control_height(),
        CHROME_BAND_GLYPH,
    );
    tip_text(ui.ctx(), &r, tip);
    !menu_open && r.clicked()
}

/// What a table tool's button shows: a Phosphor glyph, or one of the two
/// marks Phosphor lacks.
#[derive(Clone, Copy)]
enum Mark {
    Glyph(&'static str),
    RemoveRow,
    RemoveColumn,
}

impl Mark {
    fn paint(self, painter: &egui::Painter, rect: Rect, color: Color32) {
        let rows = match self {
            Mark::Glyph(icon) => {
                let font = phosphor_font_id(CHROME_BAND_GLYPH);
                let g = painter.layout_no_wrap(icon.into(), font, Color32::PLACEHOLDER);
                painter.galley(rect.center() - g.size() / 2.0, g, color);
                return;
            }
            Mark::RemoveRow => true,
            Mark::RemoveColumn => false,
        };

        // Phosphor's rows-plus-bottom and columns-plus-right, on its
        // 256-unit grid, with a minus where the plus is
        let unit = CHROME_BAND_GLYPH / 256.0;
        let origin = rect.center() - Vec2::splat(CHROME_BAND_GLYPH / 2.0);
        for bar in [40.0, 112.0] {
            let (min, size) = (egui::vec2(32.0, bar), egui::vec2(192.0, 56.0));
            let (min, size) = if rows { (min, size) } else { (min.yx(), size.yx()) };
            let bar = Rect::from_min_size(origin + min * unit, size * unit);
            let stroke = Stroke::new(16.0 * unit, color);
            painter.rect_stroke(bar, 16.0 * unit, stroke, StrokeKind::Inside);
        }
        let minus = if rows { egui::vec2(96.0, 208.0) } else { egui::vec2(184.0, 120.0) };
        let minus = Rect::from_min_size(origin + minus * unit, egui::vec2(64.0, 16.0) * unit);
        painter.rect_filled(minus, 8.0 * unit, color);
    }
}

/// The table set in toolbar groups: each tool's mark and tooltip. The first
/// group stands in for keys a touch keyboard lacks.
const TABLE_TOOLS: &[&[(MarkdownToolbarAction, Mark, &str)]] = {
    use Mark::Glyph;
    use MarkdownToolbarAction::*;
    &[
        &[
            (PreviousCell, Glyph(phosphor::ARROW_LINE_LEFT), "Previous cell"),
            (NextCell, Glyph(phosphor::ARROW_LINE_RIGHT), "Next cell"),
            (NextRow, Glyph(phosphor::KEY_RETURN), "Next row"),
        ],
        &[
            (RowAbove, Glyph(phosphor::ROWS_PLUS_TOP), "Insert row above"),
            (RowBelow, Glyph(phosphor::ROWS_PLUS_BOTTOM), "Insert row below"),
            (ColumnLeft, Glyph(phosphor::COLUMNS_PLUS_LEFT), "Insert column left"),
            (ColumnRight, Glyph(phosphor::COLUMNS_PLUS_RIGHT), "Insert column right"),
        ],
        &[
            (DeleteRow, Mark::RemoveRow, "Delete row"),
            (DeleteColumn, Mark::RemoveColumn, "Delete column"),
        ],
        &[
            (AlignLeft, Glyph(phosphor::TEXT_ALIGN_LEFT), "Align left"),
            (AlignCenter, Glyph(phosphor::TEXT_ALIGN_CENTER), "Align center"),
            (AlignRight, Glyph(phosphor::TEXT_ALIGN_RIGHT), "Align right"),
        ],
    ]
};

/// A table tool's button. One that would do nothing shows muted.
fn toolbar_table_tool(
    ui: &mut Ui, t: &crate::style::Theme, tool: TableTool, menu_open: bool,
) -> bool {
    let mark = TABLE_TOOLS
        .iter()
        .flat_map(|tools| tools.iter())
        .find(|(action, ..)| *action == tool.action)
        .map(|&(_, mark, _)| mark)
        .expect("a table tool");
    let tip = tool.tip;
    let hit = control_height();
    if !tool.enabled {
        let (rect, r) = ui.allocate_exact_size(Vec2::splat(hit), egui::Sense::hover());
        mark.paint(ui.painter(), rect, t.neutral());
        tip_text(ui.ctx(), &r, tip);
        return false;
    }
    let applied = tool.applied && !menu_open;
    let r = icon_button_mark(ui, t, applied, t.neutral_bg(), hit, |painter, rect, color| {
        mark.paint(painter, rect, color)
    });
    tip_text(ui.ctx(), &r, tip);
    !menu_open && r.clicked()
}

pub struct Toolbar {
    pub menu_open: bool,
    heading_last_click_at: Instant,
    /// Whether the table set was shown last frame: entering or leaving a
    /// table scrolls the toolbar back to its start.
    in_table: bool,
    /// The size picker under the insert-table button, while open.
    pub table_picker_open: bool,
    /// The picker's grid as last painted, for a test to click in.
    pub table_picker_grid: Option<Rect>,
}

impl Default for Toolbar {
    fn default() -> Self {
        Self {
            menu_open: false,
            heading_last_click_at: Instant::now(),
            in_table: false,
            table_picker_open: false,
            table_picker_grid: None,
        }
    }
}

/// The insert-table picker's grid, in cells.
pub const TABLE_PICKER: (usize, usize) = (10, 8);

/// The size picker by the insert-table button — under it, or on a `phone`
/// over it, where the keyboard is under the toolbar, and sized for a
/// finger: the block of cells up to the one under the pointer lights up,
/// a caption says its size, and a click or a lift inserts a table of it.
/// Any other click, or Escape, puts it away.
fn show_table_picker(
    ui: &mut Ui, t: &crate::style::Theme, anchor: Rect, phone: bool,
) -> (Option<Event>, Option<Rect>, Rect, bool) {
    use crate::style::Space;
    let (columns, rows) = TABLE_PICKER;
    let (cell_side, gap, caption_size, padding) =
        if phone { (22.0, 5.0, 15.0, Space::Sm) } else { (14.0, 4.0, 12.0, Space::Xs) };
    let above = phone;
    let step = cell_side + gap;
    let grid_size = Vec2::new(columns as f32 * step - gap, rows as f32 * step - gap);
    let mut chosen = None;
    let mut grid_rect = None;
    let area = egui::Area::new(egui::Id::unique("md_table_picker"))
        .order(egui::Order::Foreground)
        .pivot(if above { egui::Align2::LEFT_BOTTOM } else { egui::Align2::LEFT_TOP })
        .fixed_pos(if above {
            anchor.left_top() - Vec2::new(0.0, 4.0)
        } else {
            anchor.left_bottom() + Vec2::new(0.0, 4.0)
        })
        .constrain(true)
        .show(ui.ctx(), |ui| {
            crate::style::chrome::canvas_overlay_frame(t, padding).show(ui, |ui| {
                let (rect, resp) = ui.allocate_exact_size(grid_size, egui::Sense::click());
                grid_rect = Some(rect);
                // a lifted finger has no hover position in the frame it clicks
                let pos = resp.interact_pointer_pos().or(resp.hover_pos());
                let picked = pos.map(|pos| {
                    let cell = ((pos - rect.min) / step).floor();
                    (
                        cell.x.clamp(0.0, columns as f32 - 1.0) as usize + 1,
                        cell.y.clamp(0.0, rows as f32 - 1.0) as usize + 1,
                    )
                });
                let painter = ui.painter();
                for row in 0..rows {
                    for column in 0..columns {
                        let min = rect.min + Vec2::new(column as f32 * step, row as f32 * step);
                        let cell = Rect::from_min_size(min, Vec2::splat(cell_side));
                        let lit = picked.is_some_and(|(c, r)| column < c && row < r);
                        let fill = if lit {
                            t.accent().gamma_multiply(0.25)
                        } else {
                            t.neutral_bg_secondary()
                        };
                        let edge = if lit { t.accent() } else { t.neutral_bg_tertiary() };
                        painter.rect_filled(cell, 2.0, fill);
                        painter.rect_stroke(
                            cell,
                            2.0,
                            Stroke::new(1.0_f32, edge),
                            StrokeKind::Inside,
                        );
                    }
                }
                let caption = match picked {
                    Some((c, r)) => format!("{c} × {r}"),
                    None => "Table size".to_string(),
                };
                ui.add_space(gap);
                ui.label(
                    RichText::new(caption)
                        .color(t.neutral_fg_secondary())
                        .size(caption_size),
                );
                // a finger slid to its size and lifted is no click to egui
                let lifted = ui.input(|i| {
                    i.pointer.any_released() && pos.is_some_and(|pos| rect.contains(pos))
                });
                if resp.clicked() || lifted {
                    if let Some((c, r)) = picked {
                        chosen =
                            Some(Event::Table(TableAction::Insert { columns: c, rows: r - 1 }));
                    }
                }
            });
        });
    let outside = ui.input(|i| {
        let clicked = i.pointer.any_click();
        let pos = i.pointer.interact_pos();
        clicked && pos.is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p))
    });
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
    (chosen, grid_rect, area.response.rect, outside || escape)
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolbarPersistence {
    undo: bool,
    redo: bool,
    heading: bool,
    bold: bool,
    emph: bool,
    code: bool,
    strikethrough: bool,
    highlight: bool,
    underline: bool,
    spoiler: bool,
    subscript: bool,
    superscript: bool,
    ordered_list: bool,
    unordered_list: bool,
    task_list: bool,
    #[serde(default)]
    quote: bool,
    link: bool,
    image: bool,
    indent: bool,
    deindent: bool,
    search: bool,
    #[serde(default)]
    table: bool,
}

impl<'ast> Editor {
    pub fn android_toolbar_state(&mut self) -> MarkdownToolbarState {
        let arena = Arena::new();
        let root = self.edit.renderer.reparse(&arena);
        let selection = self.edit.renderer.buffer.current.selection;
        let mut state = MarkdownToolbarState::default();
        let styles = [
            (MarkdownToolbarAction::Bold, NodeValue::Strong),
            (MarkdownToolbarAction::Italic, NodeValue::Emph),
            (MarkdownToolbarAction::Code, NodeValue::Code(Default::default())),
            (MarkdownToolbarAction::Strikethrough, NodeValue::Strikethrough),
            (MarkdownToolbarAction::Highlight, NodeValue::Highlight),
            (MarkdownToolbarAction::Underline, NodeValue::Underline),
            (MarkdownToolbarAction::Spoiler, NodeValue::SpoileredText),
            (MarkdownToolbarAction::Subscript, NodeValue::Subscript),
            (MarkdownToolbarAction::Superscript, NodeValue::Superscript),
            (MarkdownToolbarAction::Link, NodeValue::Link(Default::default())),
            (MarkdownToolbarAction::Image, NodeValue::Image(Default::default())),
        ];
        for (action, style) in styles {
            if self.edit.inline_styled(root, selection, &style) {
                state.active.push(action);
            }
        }
        for (action, style) in [
            (
                MarkdownToolbarAction::NumberedList,
                NodeValue::List(NodeList { list_type: ListType::Ordered, ..Default::default() }),
            ),
            (
                MarkdownToolbarAction::BulletedList,
                NodeValue::List(NodeList { list_type: ListType::Bullet, ..Default::default() }),
            ),
            (
                MarkdownToolbarAction::TaskList,
                NodeValue::List(NodeList {
                    list_type: ListType::Bullet,
                    is_task_list: true,
                    ..Default::default()
                }),
            ),
            (MarkdownToolbarAction::Quote, NodeValue::BlockQuote),
        ] {
            if self.edit.unapply_block(root, &style) {
                state.active.push(action);
            }
        }
        for node in root.descendants() {
            if let NodeValue::Heading(heading) = &node.data.borrow().value {
                if self
                    .edit
                    .renderer
                    .node_range(node)
                    .contains_range(&selection, true, true)
                {
                    state.active.push(MarkdownToolbarAction::Heading);
                    state.heading_level = Some(heading.level);
                    break;
                }
            }
        }
        for tool in self.table_tools().into_iter().flatten().flatten() {
            state.in_table = true;
            if tool.applied {
                state.active.push(tool.action);
            }
            if !tool.enabled {
                state.disabled.push(tool.action);
            }
        }
        let position = self.edit.renderer.table_position();
        state.on_header = position.is_some_and(|(_, row, _)| row == 0);
        state
    }

    /// The event a toolbar action stands for.
    pub fn toolbar_action(&mut self, action: MarkdownToolbarAction) -> Event {
        use TableAction::*;

        let style = match action {
            MarkdownToolbarAction::Undo => return Event::Undo,
            MarkdownToolbarAction::Redo => return Event::Redo,
            MarkdownToolbarAction::Heading => {
                let level = if self.toolbar.heading_last_click_at.elapsed() > Duration::from_secs(1)
                {
                    1
                } else {
                    let arena = Arena::new();
                    let root = self.edit.renderer.reparse(&arena);
                    let selection = self.edit.renderer.buffer.current.selection;
                    let current = root
                        .descendants()
                        .find_map(|node| {
                            if let NodeValue::Heading(heading) = &node.data.borrow().value {
                                self.edit
                                    .renderer
                                    .node_range(node)
                                    .contains_range(&selection, true, true)
                                    .then_some(heading.level)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    current.min(5) + 1
                };
                self.toolbar.heading_last_click_at = Instant::now();
                NodeValue::Heading(NodeHeading { level, ..Default::default() })
            }
            MarkdownToolbarAction::Bold => NodeValue::Strong,
            MarkdownToolbarAction::Italic => NodeValue::Emph,
            MarkdownToolbarAction::Code => NodeValue::Code(Default::default()),
            MarkdownToolbarAction::Strikethrough => NodeValue::Strikethrough,
            MarkdownToolbarAction::Highlight => NodeValue::Highlight,
            MarkdownToolbarAction::Underline => NodeValue::Underline,
            MarkdownToolbarAction::Spoiler => NodeValue::SpoileredText,
            MarkdownToolbarAction::Subscript => NodeValue::Subscript,
            MarkdownToolbarAction::Superscript => NodeValue::Superscript,
            MarkdownToolbarAction::NumberedList => {
                NodeValue::List(NodeList { list_type: ListType::Ordered, ..Default::default() })
            }
            MarkdownToolbarAction::BulletedList => {
                NodeValue::List(NodeList { list_type: ListType::Bullet, ..Default::default() })
            }
            MarkdownToolbarAction::TaskList => NodeValue::List(NodeList {
                list_type: ListType::Bullet,
                is_task_list: true,
                ..Default::default()
            }),
            MarkdownToolbarAction::Quote => NodeValue::BlockQuote,
            MarkdownToolbarAction::Link => NodeValue::Link(Default::default()),
            MarkdownToolbarAction::Image => NodeValue::Image(Default::default()),
            MarkdownToolbarAction::Indent | MarkdownToolbarAction::NextCell => {
                return Event::Indent { deindent: false };
            }
            MarkdownToolbarAction::Outdent | MarkdownToolbarAction::PreviousCell => {
                return Event::Indent { deindent: true };
            }
            MarkdownToolbarAction::NextRow => return Event::Newline { shift: false },
            MarkdownToolbarAction::Table => return Event::Table(Insert { columns: 3, rows: 1 }),
            MarkdownToolbarAction::RowAbove => return Event::Table(InsertRow { below: false }),
            MarkdownToolbarAction::RowBelow => return Event::Table(InsertRow { below: true }),
            MarkdownToolbarAction::ColumnLeft => {
                return Event::Table(InsertColumn { right: false });
            }
            MarkdownToolbarAction::ColumnRight => {
                return Event::Table(InsertColumn { right: true });
            }
            MarkdownToolbarAction::DeleteRow => return Event::Table(DeleteRow),
            MarkdownToolbarAction::DeleteColumn => return Event::Table(DeleteColumn),
            MarkdownToolbarAction::MoveRowUp => return Event::Table(MoveRow { down: false }),
            MarkdownToolbarAction::MoveRowDown => return Event::Table(MoveRow { down: true }),
            MarkdownToolbarAction::MoveColumnLeft => {
                return Event::Table(MoveColumn { right: false });
            }
            MarkdownToolbarAction::MoveColumnRight => {
                return Event::Table(MoveColumn { right: true });
            }
            MarkdownToolbarAction::AlignLeft => {
                return self.table_align_event(TableAlignment::Left);
            }
            MarkdownToolbarAction::AlignCenter => {
                return self.table_align_event(TableAlignment::Center);
            }
            MarkdownToolbarAction::AlignRight => {
                return self.table_align_event(TableAlignment::Right);
            }
        };
        Event::ToggleStyle { region: Region::Selection, style }
    }
    pub fn show_toolbar(&mut self, root: &'ast AstNode<'ast>, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let w = ui.available_width();
        let (band, _) = ui.allocate_exact_size(egui::vec2(w, CHROME_BAND_H), egui::Sense::hover());
        let t = ui.ctx().get_lb_theme();
        ui.painter().rect_filled(band, 0.0, t.neutral_bg());
        place_at(ui, band, Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            self.show_toolbar_inner(root, ui);
        });
    }

    /// Computes the toolbar's content width without drawing it.
    pub fn toolbar_width(&self) -> f32 {
        let btn = control_height();
        let sep = 20.; // separator with .spacing(20.)
        let gap = 5.; // explicit add_space(5.) between buttons
        let margin = 10.; // padding on each side

        let persistence = self.persistence.get_markdown().toolbar;
        let is_default = persistence == Default::default();
        let is_ios = cfg!(target_os = "ios");
        let supports_attachments = cfg!(any(target_os = "ios", target_os = "android"));

        // width of a group of n buttons with intra-group spacing + trailing separator
        let group = |n: usize| -> f32 {
            if n > 0 { btn * n as f32 + gap * (n - 1) as f32 + sep } else { 0. }
        };
        let count =
            |flags: &[bool]| -> usize { flags.iter().filter(|&&on| on || is_default).count() };
        // a group with nothing after it
        let last = |n: usize| -> f32 { (group(n) - sep).max(0.) };

        let table_tools = self.table_tools();
        // a table has no place for the block styles
        let block =
            |flags: &[bool]| -> usize { if table_tools.is_some() { 0 } else { count(flags) } };

        let mut w = 2. * margin;

        if is_ios && (persistence.search || is_default) {
            w += group(1);
        }

        w += group(count(&[persistence.undo, persistence.redo]));
        for tools in table_tools.iter().flatten() {
            w += group(tools.len());
        }
        w += group(
            block(&[persistence.heading])
                + count(&[
                    persistence.bold,
                    persistence.emph,
                    persistence.code,
                    persistence.strikethrough,
                    persistence.highlight,
                    persistence.underline,
                    persistence.spoiler,
                    persistence.subscript,
                    persistence.superscript,
                ]),
        );
        w += group(block(&[
            persistence.ordered_list,
            persistence.unordered_list,
            persistence.task_list,
            persistence.quote,
            persistence.table,
        ]));

        let mut media = count(&[persistence.link]);
        if (persistence.image || is_default) && supports_attachments {
            media += 1;
        }
        let indents = block(&[persistence.indent, persistence.deindent]);
        w += if indents > 0 { group(media) + last(indents) } else { last(media) };

        w
    }

    /// The toolbar's table set, in groups: what it offers in place of the
    /// block styles while the selection ends in a table. `None` off one.
    pub fn table_tools(&self) -> Option<Vec<Vec<TableTool>>> {
        use MarkdownToolbarAction::*;

        let r = &self.edit.renderer;
        let (table, row, column) = r.table_position()?;
        let alignment = table.delimiter.alignments[column];
        let tool = |action, tip| TableTool {
            action,
            applied: match action {
                AlignLeft => alignment == TableAlignment::Left,
                AlignCenter => alignment == TableAlignment::Center,
                AlignRight => alignment == TableAlignment::Right,
                _ => false,
            },
            enabled: true,
            // the header is the table
            tip: if action == DeleteRow && row == 0 { "Delete table" } else { tip },
        };
        let groups = TABLE_TOOLS.iter().skip(if r.touch_mode { 0 } else { 1 });
        Some(
            groups
                .map(|tools| {
                    tools
                        .iter()
                        .map(|&(action, _, tip)| tool(action, tip))
                        .collect()
                })
                .collect(),
        )
    }

    /// The event for an alignment tool: it sets the alignment of the
    /// selection's column, or takes it away if the column has it.
    fn table_align_event(&self, to: TableAlignment) -> Event {
        let position = self.edit.renderer.table_position();
        let current = position.map(|(table, _, column)| table.delimiter.alignments[column]);
        let to = if current == Some(to) { TableAlignment::None } else { to };
        Event::Table(TableAction::Align(to))
    }

    #[allow(clippy::option_map_unit_fn)] // use of .map() reduces line wrapping, improving readability
    pub fn show_toolbar_inner(&mut self, root: &'ast AstNode<'ast>, ui: &mut Ui) {
        // center the toolbar content horizontally
        let toolbar_w = self.toolbar_width();
        let available = ui.available_width();
        let offset = ((available - toolbar_w) / 2.).max(0.);

        let in_table = self.table_tools().is_some();
        let swapped = mem::replace(&mut self.toolbar.in_table, in_table) != in_table;
        let mut scroll = ScrollArea::horizontal()
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .max_height(control_height());
        if swapped {
            scroll = scroll.horizontal_scroll_offset(0.0);
        }
        scroll.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                let t = ui.ctx().get_lb_theme();
                let menu_open = self.toolbar.menu_open;
                let is_ios = cfg!(target_os = "ios");
                let supports_attachments = cfg!(any(target_os = "ios", target_os = "android"));

                let toolbar_margin = 10.;
                // offset centers the full toolbar_w (including margins);
                // add margin so buttons start after the leading margin
                ui.add_space(offset + toolbar_margin);

                let persistence = self.persistence.get_markdown().toolbar;
                let toolbar_is_default = persistence == Default::default();

                let table_tools = self.table_tools();
                // a table has no place for the block styles
                let in_table = table_tools.is_some();
                let block = |on: bool| (on || toolbar_is_default) && !in_table;
                let indents = block(persistence.indent) || block(persistence.deindent);

                let mut events = Vec::new();

                if is_ios && (persistence.search || toolbar_is_default) {
                    let find_open = self.find.term.is_some();
                    if toolbar_icon(ui, &t, phosphor::SEARCH, find_open, menu_open, "Search") {
                        if find_open {
                            self.find.term = None;
                            self.find.matches.clear();
                            self.find.current_match = None;
                        } else {
                            self.find.open_requested = true;
                        }
                    }
                    add_seperator(ui);
                }

                let mut any_undo_redo = false;
                if persistence.undo || toolbar_is_default {
                    if toolbar_icon(
                        ui,
                        &t,
                        phosphor::ARROW_COUNTER_CLOCKWISE,
                        false,
                        menu_open,
                        "Undo",
                    ) {
                        events.push(Event::Undo);
                    }
                    any_undo_redo = true;
                }
                if persistence.redo || toolbar_is_default {
                    if any_undo_redo {
                        ui.add_space(5.);
                    }
                    if toolbar_icon(ui, &t, phosphor::ARROW_CLOCKWISE, false, menu_open, "Redo") {
                        events.push(Event::Redo);
                    }
                    any_undo_redo = true;
                }
                if any_undo_redo {
                    add_seperator(ui);
                }

                for tools in table_tools.into_iter().flatten() {
                    for (i, tool) in tools.into_iter().enumerate() {
                        if i > 0 {
                            ui.add_space(5.);
                        }
                        if toolbar_table_tool(ui, &t, tool, menu_open) {
                            events.push(self.toolbar_action(tool.action));
                        }
                    }
                    add_seperator(ui);
                }

                let mut any_style = false;
                if block(persistence.heading) {
                    self.heading_button(root, ui).map(|e| events.push(e));
                    any_style = true;
                }
                if persistence.bold || toolbar_is_default {
                    if any_style {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::TEXT_B, NodeValue::Strong, root, ui)
                        .map(|e| events.push(e));
                    any_style = true;
                }
                if persistence.emph || toolbar_is_default {
                    if any_style {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::TEXT_ITALIC, NodeValue::Emph, root, ui)
                        .map(|e| events.push(e));
                    any_style = true;
                }
                if persistence.code || toolbar_is_default {
                    if any_style {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::CODE, NodeValue::Code(Default::default()), root, ui)
                        .map(|e| events.push(e));
                    any_style = true;
                }
                if persistence.strikethrough || toolbar_is_default {
                    if any_style {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::TEXT_STRIKETHROUGH, NodeValue::Strikethrough, root, ui)
                        .map(|e| events.push(e));
                    any_style = true;
                }
                if persistence.highlight || toolbar_is_default {
                    if any_style {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::HIGHLIGHTER, NodeValue::Highlight, root, ui)
                        .map(|e| events.push(e));
                    any_style = true;
                }
                if persistence.underline || toolbar_is_default {
                    if any_style {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::TEXT_UNDERLINE, NodeValue::Underline, root, ui)
                        .map(|e| events.push(e));
                    any_style = true;
                }
                if persistence.spoiler || toolbar_is_default {
                    if any_style {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::EYE_SLASH, NodeValue::SpoileredText, root, ui)
                        .map(|e| events.push(e));
                    any_style = true;
                }
                if persistence.subscript || toolbar_is_default {
                    if any_style {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::TEXT_SUBSCRIPT, NodeValue::Subscript, root, ui)
                        .map(|e| events.push(e));
                    any_style = true;
                }
                if persistence.superscript || toolbar_is_default {
                    if any_style {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::TEXT_SUPERSCRIPT, NodeValue::Superscript, root, ui)
                        .map(|e| events.push(e));
                    any_style = true;
                }
                if any_style {
                    add_seperator(ui);
                }

                let mut any_list = false;
                if block(persistence.ordered_list) {
                    self.style(
                        phosphor::LIST_NUMBERS,
                        NodeValue::List(NodeList {
                            list_type: ListType::Ordered,
                            ..Default::default()
                        }),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));
                    any_list = true;
                }
                if block(persistence.unordered_list) {
                    if any_list {
                        ui.add_space(5.);
                    }
                    self.style(
                        phosphor::LIST_BULLETS,
                        NodeValue::List(NodeList {
                            list_type: ListType::Bullet,
                            ..Default::default()
                        }),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));
                    any_list = true;
                }
                if block(persistence.task_list) {
                    if any_list {
                        ui.add_space(5.);
                    }
                    self.style(
                        phosphor::CHECK_SQUARE,
                        NodeValue::List(NodeList {
                            list_type: ListType::Bullet,
                            is_task_list: true,
                            ..Default::default()
                        }),
                        root,
                        ui,
                    )
                    .map(|e| events.push(e));
                    any_list = true;
                }
                if block(persistence.quote) {
                    if any_list {
                        ui.add_space(5.);
                    }
                    self.style(phosphor::QUOTES, NodeValue::BlockQuote, root, ui)
                        .map(|e| events.push(e));
                    any_list = true;
                }
                if block(persistence.table) {
                    if any_list {
                        ui.add_space(5.);
                    }
                    let open = self.toolbar.table_picker_open;
                    let r = icon_button_glyph(
                        ui,
                        &t,
                        phosphor::TABLE,
                        open && !menu_open,
                        t.neutral_bg(),
                        control_height(),
                        CHROME_BAND_GLYPH,
                    );
                    tip_text(ui.ctx(), &r, "Table");
                    if !menu_open && r.clicked() {
                        self.toolbar.table_picker_open = !open;
                    }
                    if self.toolbar.table_picker_open && !menu_open {
                        let phone = self.edit.phone_mode;
                        let (chosen, grid, rect, close) = show_table_picker(ui, &t, r.rect, phone);
                        self.toolbar.table_picker_grid = grid;
                        // its touches stay with egui, not the iOS text page
                        self.edit
                            .renderer
                            .touch_targets
                            .push((rect, TouchTarget::Popup));
                        let caret = self.edit.renderer.buffer.current.selection.1;
                        let caret = self.edit.cursor_line(caret);
                        self.next_resp.caret_covered = caret.is_some_and(|[top, bottom]| {
                            rect.intersects(Rect::from_two_pos(top, bottom))
                        });
                        if chosen.is_some() || close {
                            self.toolbar.table_picker_open = false;
                        }
                        events.extend(chosen);
                    } else {
                        self.toolbar.table_picker_grid = None;
                    }
                    any_list = true;
                }
                if any_list {
                    add_seperator(ui);
                }

                let mut any_media = false;
                if persistence.link || toolbar_is_default {
                    self.style(phosphor::LINK, NodeValue::Link(Default::default()), root, ui)
                        .map(|e| events.push(e));
                    ui.add_space(5.);
                    self.style(phosphor::IMAGE, NodeValue::Image(Default::default()), root, ui)
                        .map(|e| events.push(e));
                    any_media = true;
                }
                if (persistence.image || toolbar_is_default) && supports_attachments {
                    if any_media {
                        ui.add_space(5.);
                    }
                    // Accent only when the cursor is inside an image, matching link.
                    let applied = self.edit.inline_styled(
                        root,
                        self.edit.renderer.buffer.current.selection,
                        &NodeValue::Image(Default::default()),
                    );
                    let tooltip =
                        if cfg!(target_os = "android") { "Insert photo" } else { "Camera" };
                    if toolbar_icon(ui, &t, phosphor::CAMERA, applied, menu_open, tooltip) {
                        events.push(Event::Camera);
                    }
                    any_media = true;
                }
                if any_media && (indents || self.edit.phone_mode) {
                    add_seperator(ui);
                }

                let mut any_indent = false;
                if block(persistence.indent) {
                    if toolbar_icon(ui, &t, phosphor::TEXT_INDENT, false, menu_open, "Indent") {
                        events.push(Event::Indent { deindent: false });
                    }
                    any_indent = true;
                }
                if block(persistence.deindent) {
                    if any_indent {
                        ui.add_space(5.);
                    }
                    if toolbar_icon(ui, &t, phosphor::TEXT_OUTDENT, false, menu_open, "De-indent") {
                        events.push(Event::Indent { deindent: true });
                    }
                    any_indent = true;
                }

                if self.edit.phone_mode {
                    if any_indent {
                        add_seperator(ui);
                    }

                    // fill remaining space
                    const MENU_TOGGLE_SPACE: f32 = 40.;
                    if ui.available_width() > MENU_TOGGLE_SPACE {
                        ui.add_space(ui.available_width() - MENU_TOGGLE_SPACE);
                    }

                    let chevron = if self.toolbar.menu_open {
                        phosphor::CARET_DOWN
                    } else {
                        phosphor::CARET_UP
                    };
                    if toolbar_icon(
                        ui,
                        &t,
                        chevron,
                        self.toolbar.menu_open,
                        false,
                        "Toolbar Settings",
                    ) {
                        self.toolbar.menu_open = !self.toolbar.menu_open;
                        ui.ctx().set_virtual_keyboard_shown(false);
                    }
                    ui.add_space(5.);
                }

                ui.add_space(toolbar_margin);
                ui.add_space(ui.available_width());

                for event in events {
                    ui.ctx().push_markdown_event(event);
                    if self.edit.phone_mode {
                        // bottom toolbar painted after editor events processed
                        ui.ctx().request_repaint();
                    }
                }
            })
        });
    }

    fn heading_button(&mut self, root: &'ast AstNode<'ast>, ui: &mut Ui) -> Option<Event> {
        let mut current_heading_level = 0;
        let mut applied = false;

        for node in root.descendants() {
            if let NodeValue::Heading(NodeHeading { level, .. }) = &node.data.borrow().value {
                if self.edit.renderer.node_range(node).contains_range(
                    &self.edit.renderer.buffer.current.selection,
                    true,
                    true,
                ) {
                    current_heading_level = *level;
                    applied = true;
                    break;
                }
            }
        }

        let level = if self.toolbar.heading_last_click_at.elapsed() > Duration::from_secs(1) {
            1
        } else {
            current_heading_level.min(5) + 1
        };
        let style = NodeValue::Heading(NodeHeading { level, ..Default::default() });

        let t = ui.ctx().get_lb_theme();
        if toolbar_icon(ui, &t, phosphor::TEXT_H_ONE, applied, self.toolbar.menu_open, style.name())
        {
            self.toolbar.heading_last_click_at = Instant::now();
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }

    fn style(
        &self, icon: &'static str, style: NodeValue, root: &'ast AstNode<'ast>, ui: &mut Ui,
    ) -> Option<Event> {
        let applied = if style.is_inline() {
            self.edit
                .inline_styled(root, self.edit.renderer.buffer.current.selection, &style)
        } else {
            self.edit.unapply_block(root, &style)
        };

        self.button(icon, style, applied, ui)
    }

    fn button(
        &self, icon: &'static str, style: NodeValue, applied: bool, ui: &mut Ui,
    ) -> Option<Event> {
        let t = ui.ctx().get_lb_theme();
        if toolbar_icon(ui, &t, icon, applied, self.toolbar.menu_open, style.name()) {
            Some(Event::ToggleStyle { region: Region::Selection, style })
        } else {
            None
        }
    }

    pub fn show_toolbar_menu(&mut self, ui: &mut Ui) {
        let margin: Margin =
            if cfg!(target_os = "android") { Margin::symmetric(0, 60) } else { Margin::ZERO };
        ScrollArea::vertical()
            .scroll_source(ScrollSource::ALL)
            .id_salt("toolbar_settings")
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    Frame::canvas(ui.style())
                        .inner_margin(margin)
                        .stroke(Stroke::NONE)
                        .fill(self.edit.renderer.ctx.get_lb_theme().neutral_bg())
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                            let is_android = cfg!(target_os = "android");
                            let is_ios = cfg!(target_os = "ios");
                            let supports_attachments = is_ios || is_android;

                            let persistence = self.persistence.get_markdown().toolbar;

                            let scroll_view_height = ui.max_rect().height();
                            ui.allocate_space(Vec2 { x: ui.available_width(), y: 0. });
                            let padding = (ui.available_width() - self.edit.renderer.width) / 2.;

                            let mut top_left =
                                ui.max_rect().min + (padding + MENU_MARGIN) * Vec2::X;
                            let md_width = self.edit.renderer.width - 2. * MENU_MARGIN;

                            // store values
                            let source_lines =
                                mem::take(&mut self.edit.renderer.bounds.source_lines);
                            let buffer = mem::take(&mut self.edit.renderer.buffer);
                            let inline_paragraphs =
                                mem::take(&mut self.edit.renderer.bounds.inline_paragraphs);

                            let fragments = mem::take(&mut self.edit.renderer.fragments);
                            let wrap_lines = mem::take(&mut self.edit.renderer.bounds.wrap_lines);
                            let touch_targets = mem::take(&mut self.edit.renderer.touch_targets);

                            // menu labels: force blue links + plain image-link text
                            let link_resolver =
                                mem::replace(&mut self.edit.renderer.link_resolver, Box::new(()));
                            self.edit.renderer.disable_images = true;

                            // labels are static exemplars — never reveal syntax
                            // at the editor's (unrelated) selection offsets
                            let reveal_selection =
                                mem::take(&mut self.edit.renderer.reveal_selection);

                            self.edit.renderer.layout_cache.clear();

                            // page title
                            ui.add_space(MENU_SPACE);
                            top_left.y += MENU_SPACE;

                            ui.vertical_centered_justified(|ui| {
                                let font =
                                    FontId::new(16.0, egui::FontFamily::Name(Arc::from("Bold")));
                                Label::new(RichText::from("Show / Hide Toolbar Buttons").font(font))
                                    .ui(ui)
                            });
                            top_left.y += ui.text_style_height(&egui::TextStyle::Heading)
                                + ui.spacing().item_spacing.y;

                            ui.add_space(MENU_SPACE);
                            top_left.y += MENU_SPACE;

                            if !is_android {
                                // search
                                if self
                                    .menu_toggle(
                                        ui,
                                        top_left,
                                        md_width,
                                        "Search",
                                        phosphor::SEARCH,
                                        persistence.search,
                                    )
                                    .clicked()
                                {
                                    let mut persistence = self.persistence.data.write().unwrap();
                                    let persistence = &mut persistence.markdown.toolbar;
                                    persistence.search ^= true;
                                    self.persistence.write_to_file();
                                }
                                top_left.y += self.menu_toggle_height("Search");

                                Separator::default().spacing(MENU_SPACE).ui(ui);
                                top_left.y += MENU_SPACE;
                            }

                            // undo / redo
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "Undo",
                                    phosphor::ARROW_COUNTER_CLOCKWISE,
                                    persistence.undo,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.undo ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("Undo");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "Redo",
                                    phosphor::ARROW_CLOCKWISE,
                                    persistence.redo,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.redo ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("Redo");

                            Separator::default().spacing(MENU_SPACE).ui(ui);
                            top_left.y += MENU_SPACE;

                            // styles
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "### Heading",
                                    phosphor::TEXT_H_ONE,
                                    persistence.heading,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.heading ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("### Heading");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "**Bold**",
                                    phosphor::TEXT_B,
                                    persistence.bold,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.bold ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("**Bold**");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "*Italic*",
                                    phosphor::TEXT_ITALIC,
                                    persistence.emph,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.emph ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("*Italic*");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "`Code`",
                                    phosphor::CODE,
                                    persistence.code,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.code ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("`Code`");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "~~Strikethrough~~",
                                    phosphor::TEXT_STRIKETHROUGH,
                                    persistence.strikethrough,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.strikethrough ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("~~Strikethrough~~");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "==Highlight==",
                                    phosphor::HIGHLIGHTER,
                                    persistence.highlight,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.highlight ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("==Highlight==");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "__Underline__",
                                    phosphor::TEXT_UNDERLINE,
                                    persistence.underline,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.underline ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("__Underline__");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "||Spoiler||",
                                    phosphor::EYE_SLASH,
                                    persistence.spoiler,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.spoiler ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("||Spoiler||");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "~Subscript~",
                                    phosphor::TEXT_SUBSCRIPT,
                                    persistence.subscript,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.subscript ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("~Subscript~");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "^Superscript^",
                                    phosphor::TEXT_SUPERSCRIPT,
                                    persistence.superscript,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.superscript ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("^Superscript^");

                            Separator::default().spacing(MENU_SPACE).ui(ui);
                            top_left.y += MENU_SPACE;

                            // lists
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "1. Ordered List",
                                    phosphor::LIST_NUMBERS,
                                    persistence.ordered_list,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.ordered_list ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("1. Ordered List");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "- Unordered List",
                                    phosphor::LIST_BULLETS,
                                    persistence.unordered_list,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.unordered_list ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("- Unordered List");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "- [ ] Task List",
                                    phosphor::CHECK_SQUARE,
                                    persistence.task_list,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.task_list ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("- [ ] Task List");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "> Quote",
                                    phosphor::QUOTES,
                                    persistence.quote,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.quote ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("> Quote");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "Table",
                                    phosphor::TABLE,
                                    persistence.table,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.table ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("Table");

                            Separator::default().spacing(MENU_SPACE).ui(ui);
                            top_left.y += MENU_SPACE;

                            // media
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "[Link](url)",
                                    phosphor::LINK,
                                    persistence.link,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.link ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("[Link](url)");

                            if supports_attachments {
                                if self
                                    .menu_toggle(
                                        ui,
                                        top_left,
                                        md_width,
                                        "![Image](url)",
                                        phosphor::CAMERA,
                                        persistence.image,
                                    )
                                    .clicked()
                                {
                                    let mut persistence = self.persistence.data.write().unwrap();
                                    let persistence = &mut persistence.markdown.toolbar;
                                    persistence.image ^= true;
                                    self.persistence.write_to_file();
                                }
                                top_left.y += self.menu_toggle_height("![Image](url)");
                            }

                            Separator::default().spacing(MENU_SPACE).ui(ui);
                            top_left.y += MENU_SPACE;

                            // indent
                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "Indent",
                                    phosphor::TEXT_INDENT,
                                    persistence.indent,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.indent ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("Indent");

                            if self
                                .menu_toggle(
                                    ui,
                                    top_left,
                                    md_width,
                                    "De-indent",
                                    phosphor::TEXT_OUTDENT,
                                    persistence.deindent,
                                )
                                .clicked()
                            {
                                let mut persistence = self.persistence.data.write().unwrap();
                                let persistence = &mut persistence.markdown.toolbar;
                                persistence.deindent ^= true;
                                self.persistence.write_to_file();
                            }
                            top_left.y += self.menu_toggle_height("De-indent");

                            // fill remaining space
                            let cumulative_height = top_left.y - ui.max_rect().min.y;
                            let height = if cumulative_height < scroll_view_height {
                                scroll_view_height - cumulative_height
                            } else {
                                0.
                            };
                            let rect = Rect::from_min_size(
                                top_left,
                                Vec2::new(self.edit.renderer.width, height),
                            );

                            ui.advance_cursor_after_rect(rect);

                            // submit shaped text — `MdEdit::show` (which
                            // normally drains text_areas) doesn't run while
                            // the menu is open
                            let text_areas = mem::take(&mut self.edit.renderer.text_areas);
                            if !text_areas.is_empty() {
                                ui.painter().add(
                                    egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                                        ui.clip_rect(),
                                        crate::GlyphonRendererCallback::new(text_areas),
                                    ),
                                );
                            }

                            // restore stored values
                            self.edit.renderer.buffer = buffer;
                            self.edit.renderer.bounds.source_lines = source_lines;
                            self.edit.renderer.bounds.inline_paragraphs = inline_paragraphs;
                            self.edit.renderer.calc_words();

                            self.edit.renderer.fragments = fragments;
                            self.edit.renderer.bounds.wrap_lines = wrap_lines;
                            self.edit.renderer.touch_targets = touch_targets;

                            self.edit.renderer.link_resolver = link_resolver;
                            self.edit.renderer.disable_images = false;
                            self.edit.renderer.reveal_selection = reveal_selection;
                        });
                });
            });
    }

    pub fn menu_toggle_height(&mut self, md: &str) -> f32 {
        let md_height = self.markdown_label_height(md);
        md_height.max(40.)
    }

    pub fn menu_toggle(
        &mut self, ui: &mut Ui, top_left: Pos2, width: f32, md: &str, icon: &'static str, on: bool,
    ) -> Response {
        let md_height = self.markdown_label_height(md);
        let height = md_height.max(40.);

        let margin = (height - md_height) / 2.;
        let md_top_left = top_left + margin * Vec2::Y;
        self.markdown_label(ui, md_top_left, width, md);

        let padding = (ui.max_rect().width() - width) / 2.;
        let t = ui.ctx().get_lb_theme();
        let resp = ui.allocate_ui_with_layout(
            Vec2::new(width, height),
            Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.add_space(padding);
                let r = icon_button_glyph(
                    ui,
                    &t,
                    icon,
                    on,
                    t.neutral_bg(),
                    control_height(),
                    CHROME_BAND_GLYPH,
                );
                tip_text(ui.ctx(), &r, md);
                r
            },
        );

        resp.inner
    }

    pub fn markdown_label_height(&mut self, md: &str) -> f32 {
        self.edit.renderer.buffer = md.into();

        // place cursor (affects capture)
        self.edit.renderer.buffer.queue(vec![Operation::Select(
            self.edit
                .renderer
                .buffer
                .current
                .segs
                .last_cursor_position()
                .into_range(),
        )]);
        self.edit.renderer.buffer.update();

        // parse
        let arena = Arena::new();
        let options = MdRender::comrak_options();
        let text_with_newline = self.edit.renderer.buffer.current.text.to_string() + "\n";
        let root = comrak::parse_document(&arena, &text_with_newline, &options);

        // pre-render work
        self.edit.renderer.calc_source_lines();
        self.edit.renderer.calc_fold_bounds(root);
        self.edit.renderer.calc_atom_bounds(root);
        self.edit.renderer.populate_hidden_by_fold(root);
        self.edit.renderer.compute_bounds(root);
        self.edit.renderer.bounds.inline_paragraphs.sort();
        self.edit.renderer.calc_words();

        let height = self.edit.renderer.height(root);

        self.edit.renderer.layout_cache.clear();

        height
    }

    pub fn markdown_label(&mut self, ui: &mut Ui, top_left: Pos2, width: f32, md: &str) {
        self.edit.renderer.buffer = md.into();

        // place cursor (affects capture)
        self.edit.renderer.buffer.queue(vec![Operation::Select(
            self.edit
                .renderer
                .buffer
                .current
                .segs
                .last_cursor_position()
                .into_range(),
        )]);
        self.edit.renderer.buffer.update();

        // parse
        let arena = Arena::new();
        let options = MdRender::comrak_options();
        let text_with_newline = self.edit.renderer.buffer.current.text.to_string() + "\n";
        let root = comrak::parse_document(&arena, &text_with_newline, &options);

        // pre-render work
        self.edit.renderer.calc_source_lines();
        self.edit.renderer.calc_fold_bounds(root);
        self.edit.renderer.calc_atom_bounds(root);
        self.edit.renderer.populate_hidden_by_fold(root);
        self.edit.renderer.compute_bounds(root);
        self.edit.renderer.bounds.inline_paragraphs.sort();
        self.edit.renderer.calc_words();

        let height = self.edit.renderer.height(root);
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));

        self.edit.renderer.show_block(
            &mut ui.new_child(UiBuilder::new().max_rect(rect).layout(*ui.layout())),
            root,
            top_left,
        );

        self.edit.renderer.layout_cache.clear();
    }
}

fn add_seperator(ui: &mut Ui) {
    let t = ui.ctx().get_lb_theme();
    // Fixed to the icon row. Stock Separator in a horizontal layout uses
    // available_height (the leftover editor), which stretched the strip.
    let hit = control_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, hit), egui::Sense::hover());
    let h = hit * 0.7;
    ui.painter().vline(
        rect.center().x,
        rect.center().y - h / 2.0..=rect.center().y + h / 2.0,
        Stroke::new(STROKE_HAIRLINE, t.neutral()),
    );
}

trait Name {
    fn name(&self) -> &'static str;
}

impl Name for NodeValue {
    fn name(&self) -> &'static str {
        match self {
            NodeValue::Document => "",
            NodeValue::FrontMatter(_) => "",
            NodeValue::BlockQuote => "Quote",
            NodeValue::List(NodeList {
                list_type: ListType::Bullet, is_task_list: false, ..
            }) => "Bulleted List",
            NodeValue::List(NodeList { list_type: ListType::Ordered, .. }) => "Numbered List",
            NodeValue::List(NodeList {
                list_type: ListType::Bullet, is_task_list: true, ..
            }) => "Task List",
            NodeValue::Item(_) => "Item",
            NodeValue::DescriptionList => "",
            NodeValue::DescriptionItem(_) => "",
            NodeValue::DescriptionTerm => "",
            NodeValue::DescriptionDetails => "",
            NodeValue::CodeBlock(_) => "",
            NodeValue::HtmlBlock(_) => "",
            NodeValue::Paragraph => "",
            NodeValue::Heading(_) => "Heading",
            NodeValue::ThematicBreak => "",
            NodeValue::FootnoteDefinition(_) => "",
            NodeValue::Table(_) => "",
            NodeValue::TableRow(_) => "",
            NodeValue::TableCell => "",
            NodeValue::Text(_) => "",
            NodeValue::TaskItem(_) => "",
            NodeValue::SoftBreak => "",
            NodeValue::LineBreak => "",
            NodeValue::Code(_) => "Code",
            NodeValue::HtmlInline(_) => "",
            NodeValue::Raw(_) => "",
            NodeValue::Emph => "Italic",
            NodeValue::Strong => "Bold",
            NodeValue::Strikethrough => "Strikethrough",
            NodeValue::Highlight => "Highlight",
            NodeValue::Superscript => "Superscript",
            NodeValue::Link(_) => "Link",
            NodeValue::Image(_) => "Image",
            NodeValue::FootnoteReference(_) => "",
            NodeValue::ShortCode(_) => "",
            NodeValue::Math(_) => "",
            NodeValue::BlockDirective(_) => "",
            NodeValue::MultilineComment(_) => "",
            NodeValue::MultilineBlockQuote(_) => "",
            NodeValue::Escaped => "",
            NodeValue::WikiLink(_) => "",
            NodeValue::Underline => "Underline",
            NodeValue::Subscript => "Subscript",
            NodeValue::SpoileredText => "Spoiler",
            NodeValue::EscapedTag(_) => "",
            NodeValue::Alert(_) => "",
            NodeValue::Insert => "",
            NodeValue::CommentInline(_) => "",
            NodeValue::Subtext => "",
        }
    }
}
