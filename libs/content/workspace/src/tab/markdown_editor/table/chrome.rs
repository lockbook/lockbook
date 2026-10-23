//! A table's own chrome: a handle on the hovered row's left border and one
//! on the hovered column's top border. A handle drags its row or column to
//! reorder it, as a list item's marker does, and a click on it selects the
//! row or column and opens its actions as a menu. Touch has no handles: a
//! long press on a body row or a header cell starts the same drags.

use comrak::nodes::{AstNode, NodeValue, TableAlignment};
use egui::{
    Align2, CornerRadius, CursorIcon, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    Vec2,
};
use lb_rs::model::text::buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use super::structure::ColumnEdit;
use super::{CellBounds, TableAction, TableCellBox};
use crate::style::{context_menu, phosphor, phosphor_font_id};
use crate::tab::markdown_editor::input::Event;
use crate::tab::markdown_editor::widget::block::drag::{BlockDrag, DragUnit, DropGap};
use crate::tab::markdown_editor::{MdEdit, MdRender};
use crate::theme::palette_v2::ThemeExt as _;

/// A row's handle; a column's is as long and lies along the top border.
const HANDLE: Vec2 = Vec2::new(12.0, 20.0);

/// A painted row to put a handle on.
struct RowHandle {
    index: usize,
    line: (Grapheme, Grapheme),
    rect: Rect,
    /// How many rows the table has, the header among them.
    of: usize,
}

/// A column to put a handle on. `rect` covers the table's painted rows.
struct ColumnHandle {
    index: usize,
    header: CellBounds,
    rect: Rect,
    alignment: TableAlignment,
    /// How many columns the table has.
    of: usize,
}

/// The handle the pointer went down on, while it is still down: until the
/// press turns into a drag, only this keeps a handle the pointer has left.
fn held_handle(ui: &Ui) -> Option<Id> {
    let pressed = ui.input(|i| i.pointer.press_start_time())?;
    let (id, at): (Id, f64) = ui.data(|d| d.get_temp(Id::unique("table_handle_held")))?;
    (at == pressed).then_some(id)
}

fn row_handle_id(line: Grapheme) -> Id {
    Id::unique(("table_row_handle", line))
}

fn column_handle_id(table: Grapheme, column: usize) -> Id {
    Id::unique(("table_column_handle", table, column))
}

impl MdRender {
    /// What identifies the table that has a row on the line starting at
    /// `row`: where its header's line starts.
    pub fn table_key(&self, row: Grapheme) -> Option<Grapheme> {
        Some(self.bounds.table_at(row)?.rows[0].line.start())
    }

    /// The cells painted this frame of the table `key` identifies.
    fn table_boxes(&self, key: Grapheme) -> impl Iterator<Item = TableCellBox> + '_ {
        let boxes = self.table_cell_boxes.iter().copied();
        boxes.filter(move |cell_box| self.table_key(cell_box.row) == Some(key))
    }

    /// The extent of each column that shows, by index, over the table's
    /// painted rows.
    fn table_column_rects(&self, key: Grapheme) -> Vec<(usize, Rect)> {
        let boxes: Vec<TableCellBox> = self.table_boxes(key).collect();
        let Some(rows) = boxes
            .iter()
            .map(|cell_box| cell_box.rect)
            .reduce(Rect::union)
        else {
            return Vec::new();
        };
        let first_row = boxes.iter().filter(|cell_box| cell_box.row == boxes[0].row);
        let column = |cell_box: &TableCellBox| {
            let rect = Rect::from_x_y_ranges(cell_box.rect.x_range(), rows.y_range());
            (cell_box.column, rect)
        };
        first_row.map(column).collect()
    }

    /// What shows of `rect`, which is painted on a table's row: a table
    /// too wide for the note hides the rest. `None` if nothing does. A
    /// rect off every table is as it was.
    pub fn table_showing(&self, rect: Rect) -> Option<Rect> {
        let on_row = |cell_box: &&TableCellBox| cell_box.rect.y_range().contains(rect.center().y);
        let Some(row) = self
            .table_cell_boxes
            .iter()
            .find(on_row)
            .map(|found| found.row)
        else {
            return Some(rect);
        };
        let cells = self.table_cell_boxes.iter();
        let cells = cells.filter(|cell_box| cell_box.row == row && on_row(cell_box));
        let shows = cells.map(|cell_box| cell_box.rect).reduce(Rect::union)?;
        let x = rect.x_range().intersection(shows.x_range());
        (x.min <= x.max).then(|| Rect::from_x_y_ranges(x, rect.y_range()))
    }

    /// The table whose header's line starts at `key`, by its place among
    /// the tables, which its scroll goes by.
    pub fn table_ordinal(&self, key: Grapheme) -> Option<usize> {
        let mut tables = self.bounds.tables.iter();
        tables.position(|table| table.rows[0].line.start() == key)
    }

    /// How far that table is scrolled along its rows.
    pub fn table_scrolled(&self, key: Grapheme) -> f32 {
        let table = self.table_ordinal(key);
        let scroll = table.and_then(|table| self.table_scrolls.get(&table));
        scroll.map_or(0.0, |scroll| scroll.offset)
    }

    /// The table wider than the note that shows at `pos`, by its place
    /// among the tables.
    pub fn scrolling_table_at(&self, pos: Pos2) -> Option<usize> {
        let key = self.table_key(self.table_cell_box_at(pos)?.row)?;
        let table = self.table_ordinal(key)?;
        self.table_scrolls.contains_key(&table).then_some(table)
    }

    /// The header cell painted under `pos`, as its table and column.
    pub fn table_header_cell_at(&self, pos: Pos2) -> Option<(Grapheme, usize)> {
        let cell_box = self.table_cell_box_at(pos)?;
        let key = self.table_key(cell_box.row)?;
        (key == cell_box.row).then_some((key, cell_box.column))
    }

    /// A drag of a column by a pointer at `pos`. `None` for a table's only
    /// column.
    pub fn table_column_drag(
        &self, (key, column): (Grapheme, usize), pos: Pos2,
    ) -> Option<BlockDrag> {
        let table = self.bounds.table_at(key)?;
        let header = table.rows[0].cells.get(column)?.span;
        let columns = self.table_column_rects(key);
        let (_, rect) = columns.iter().find(|(index, _)| *index == column)?;
        (table.columns() > 1).then(|| BlockDrag {
            section_range: header,
            grabbed: header,
            parent_start: key,
            grab_offset: pos - rect.left_top(),
            unit: DragUnit::Column(column),
        })
    }

    /// Where a dragged column would drop: the gap between the other
    /// columns that its card's middle is in. `None` over its own slot.
    pub fn table_column_drop_gap(
        &self, drag: &BlockDrag, column: usize, pointer: Pos2,
    ) -> Option<DropGap> {
        let table = self.bounds.table_at(drag.parent_start)?;
        let columns = self.table_column_rects(drag.parent_start);
        let (_, dragged) = columns.iter().find(|(index, _)| *index == column)?;
        let middle = pointer.x - drag.grab_offset.x + dragged.width() / 2.0;
        let mut others = columns.iter().filter(|(index, _)| *index != column);
        // the gap is before a column, or after the last that shows
        let (gap_index, x) = match others.clone().find(|(_, other)| other.center().x >= middle) {
            Some((next, other)) => (*next, other.left()),
            None => others
                .next_back()
                .map(|(last, other)| (last + 1, other.right()))?,
        };
        if gap_index == column || gap_index == column + 1 {
            return None;
        }
        let insert_offset = match table.rows[0].cells.get(gap_index) {
            Some(cell) => cell.span.start(),
            None => self.buffer.current.segs.last_cursor_position(),
        };
        Some(DropGap { insert_offset, gap_index, at: x as i32 })
    }

    /// Paints and runs the handles of the table whose cells are
    /// `table_cell_boxes[first_cell_box..]`: one for the row and one for
    /// the column under the pointer, and any that is being dragged or has
    /// its menu open.
    pub fn show_table_handles(&mut self, ui: &mut Ui, first_cell_box: usize) {
        if self.touch_mode || self.readonly || !self.interactive || self.painting_drag_float {
            return;
        }
        let boxes = &self.table_cell_boxes[first_cell_box..];
        let Some(painted) = boxes
            .iter()
            .map(|cell_box| cell_box.rect)
            .reduce(Rect::union)
        else {
            return;
        };
        let Some(table) = self.bounds.table_at(boxes[0].row) else { return };
        let key = table.rows[0].line.start();

        // a drag or a selection under way owns the pointer
        let held = held_handle(ui);
        let busy = self.in_progress_block_drag.is_some()
            || self.in_progress_selection.is_some()
            || held.is_some();
        let reach = painted.expand2(HANDLE.yx() / 2.0);
        let hover = ui.input(|i| i.pointer.hover_pos());
        let pointer =
            hover.filter(|p| !busy && reach.contains(*p) && ui.rect_contains_pointer(reach));
        let open = context_menu::open_host(ui.ctx());
        let dragged = self
            .in_progress_block_drag
            .filter(|drag| drag.parent_start == key);

        let mut rows = Vec::new();
        for (index, row) in table.rows.iter().enumerate() {
            let cells = boxes
                .iter()
                .filter(|cell_box| cell_box.row == row.line.start());
            let Some(rect) = cells.map(|cell_box| cell_box.rect).reduce(Rect::union) else {
                continue;
            };
            let id = Some(row_handle_id(row.line.start()));
            let shown = pointer.is_some_and(|p| rect.y_range().contains(p.y))
                || open == id
                || held == id
                || dragged.is_some_and(|drag| {
                    drag.unit == DragUnit::Row && row.line.contains_inclusive(drag.section_range.0)
                });
            if shown {
                rows.push(RowHandle { index, line: row.line, rect, of: table.rows.len() });
            }
        }

        let mut columns = Vec::new();
        let first_row = boxes.iter().filter(|cell_box| cell_box.row == boxes[0].row);
        for cell_box in first_row {
            let index = cell_box.column;
            let rect = Rect::from_x_y_ranges(cell_box.rect.x_range(), painted.y_range());
            let id = Some(column_handle_id(key, index));
            let shown = pointer.is_some_and(|p| rect.x_range().contains(p.x))
                || open == id
                || held == id
                || dragged.is_some_and(|drag| drag.unit == DragUnit::Column(index));
            if let Some(&header) = table.rows[0].cells.get(index).filter(|_| shown) {
                let alignment = table.delimiter.alignments[index];
                columns.push(ColumnHandle { index, header, rect, alignment, of: table.columns() });
            }
        }

        for row in rows {
            self.show_table_row_handle(ui, key, row);
        }
        for column in columns {
            self.show_table_column_handle(ui, key, column);
        }
    }

    fn show_table_row_handle(&mut self, ui: &mut Ui, key: Grapheme, row: RowHandle) {
        use TableAction::*;

        let size = Vec2::new(HANDLE.x, HANDLE.y.min(row.rect.height() - 4.0));
        let pill = Rect::from_center_size(row.rect.left_center(), size);
        let dragging = self.in_progress_block_drag.is_some_and(|drag| {
            drag.unit == DragUnit::Row && row.line.contains_inclusive(drag.section_range.0)
        });
        let id = row_handle_id(row.line.start());
        // the header does not drag: its pill is plain
        let glyph = if row.index > 0 { phosphor::DOTS_SIX_VERTICAL } else { "" };
        let resp = self.table_handle(ui, id, pill, glyph, dragging);

        // body rows reorder among themselves
        if row.index > 0 && row.of > 2 {
            self.handle_drag_resp(ui, &resp, dragging, |_, origin| {
                Some(BlockDrag {
                    section_range: row.line,
                    grabbed: row.line,
                    parent_start: key,
                    grab_offset: origin - row.rect.left_top(),
                    unit: DragUnit::Row,
                })
            });
        }
        if resp.clicked() {
            self.render_events
                .push(Event::Select { region: row.line.into() });
        }
        let chosen = context_menu::show_toggle(&resp, &self.ctx.get_lb_theme(), |e| {
            e.item(phosphor::ROWS_PLUS_TOP, "Insert row above", InsertRow { below: false });
            e.item(phosphor::ROWS_PLUS_BOTTOM, "Insert row below", InsertRow { below: true });
            // the header stays the header; it is the table
            if row.index > 0 {
                e.separator();
                e.item(phosphor::ARROW_UP, "Move up", MoveRow { down: false });
                if row.index + 1 < row.of {
                    e.item(phosphor::ARROW_DOWN, "Move down", MoveRow { down: true });
                }
            }
            e.separator();
            let delete = if row.index > 0 { "Delete row" } else { "Delete table" };
            e.item_danger(phosphor::TRASH, delete, DeleteRow);
        });
        self.render_events.extend(chosen.map(Event::Table));
    }

    fn show_table_column_handle(&mut self, ui: &mut Ui, key: Grapheme, column: ColumnHandle) {
        use TableAction::*;

        let size = Vec2::new(HANDLE.y.min(column.rect.width() - 4.0), HANDLE.x);
        let pill = Rect::from_center_size(column.rect.center_top(), size);
        let dragging = self.in_progress_block_drag.is_some_and(|drag| {
            drag.parent_start == key && drag.unit == DragUnit::Column(column.index)
        });
        let id = column_handle_id(key, column.index);
        let theme = self.ctx.get_lb_theme();

        // a column has no selection to show it by
        if context_menu::is_open_id(ui.ctx(), id) {
            ui.painter()
                .rect_filled(column.rect, 0.0, theme.accent().gamma_multiply(0.12));
        }
        let resp = self.table_handle(ui, id, pill, phosphor::DOTS_SIX, dragging);

        if column.of > 1 {
            self.handle_drag_resp(ui, &resp, dragging, |r, origin| {
                r.table_column_drag((key, column.index), origin)
            });
        }
        if resp.clicked() {
            self.render_events
                .push(Event::Select { region: column.header.content.into() });
        }
        let chosen = context_menu::show_toggle(&resp, &theme, |e| {
            e.item(
                phosphor::COLUMNS_PLUS_LEFT,
                "Insert column left",
                InsertColumn { right: false },
            );
            e.item(
                phosphor::COLUMNS_PLUS_RIGHT,
                "Insert column right",
                InsertColumn { right: true },
            );
            e.separator();
            if column.index > 0 {
                e.item(phosphor::ARROW_LEFT, "Move left", MoveColumn { right: false });
            }
            if column.index + 1 < column.of {
                e.item(phosphor::ARROW_RIGHT, "Move right", MoveColumn { right: true });
            }
            e.separator();
            for (icon, label, alignment) in [
                (phosphor::TEXT_ALIGN_LEFT, "Align left", TableAlignment::Left),
                (phosphor::TEXT_ALIGN_CENTER, "Align center", TableAlignment::Center),
                (phosphor::TEXT_ALIGN_RIGHT, "Align right", TableAlignment::Right),
            ] {
                // the column's own is marked, and choosing it takes it away
                if column.alignment == alignment {
                    e.item(phosphor::CHECK, label, Align(TableAlignment::None));
                } else {
                    e.item(icon, label, Align(alignment));
                }
            }
            e.separator();
            e.item_danger(phosphor::TRASH, "Delete column", DeleteColumn);
        });
        self.render_events.extend(chosen.map(Event::Table));
    }

    /// A handle: a pill across a border of the table, lit while its menu
    /// is open.
    fn table_handle(
        &mut self, ui: &mut Ui, id: Id, pill: Rect, glyph: &'static str, dragging: bool,
    ) -> egui::Response {
        let resp = ui.interact(pill.expand(3.0), id, Sense::click_and_drag());
        if resp.hovered() {
            ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
        }
        let pressed = ui.input(|i| i.pointer.press_start_time());
        if let Some(pressed) = pressed.filter(|_| resp.is_pointer_button_down_on()) {
            ui.data_mut(|d| d.insert_temp(Id::unique("table_handle_held"), (id, pressed)));
        }
        // the drag's overlay stands where the handle would
        if dragging {
            return resp;
        }
        let theme = self.ctx.get_lb_theme();
        let (fill, edge, ink) = if context_menu::is_open(&resp) {
            (theme.accent(), theme.accent(), theme.neutral_bg())
        } else if resp.hovered() {
            (theme.neutral_bg(), theme.neutral(), theme.neutral_fg())
        } else {
            (theme.neutral_bg(), theme.neutral_bg_tertiary(), theme.neutral_fg_secondary())
        };
        let painter = ui.painter();
        painter.rect_filled(pill, 4.0, fill);
        painter.rect_stroke(pill, 4.0, Stroke::new(1.0_f32, edge), StrokeKind::Inside);
        painter.text(pill.center(), Align2::CENTER_CENTER, glyph, phosphor_font_id(12.0), ink);
        resp
    }
}

impl MdEdit {
    /// Gives a touch drag on the note's body to the table wider than the
    /// note it began on, if it sets out along the rows: the table pans and
    /// the page stays where it is. One that sets out down the rows is the
    /// page's, as anywhere. True while the drag is not the page's.
    pub fn pan_table(&mut self, ui: &Ui, body: &egui::Response) -> bool {
        const SLOP: f32 = 6.0;
        let (origin, pos, delta) =
            ui.input(|i| (i.pointer.press_origin(), i.pointer.interact_pos(), i.pointer.delta()));
        // a touch the platform cancels ends with no release: the pointer is
        // gone and the body is dragged still, through the next press
        let (pressed, gone) = ui.input(|i| (i.pointer.any_pressed(), !i.pointer.has_pointer()));
        if body.drag_started() || (pressed && body.dragged()) {
            let table = origin.and_then(|origin| self.renderer.scrolling_table_at(origin));
            self.table_pan = table.map(|table| (table, None));
            if let Some(scroll) =
                table.and_then(|table| self.renderer.table_scrolls.get_mut(&table))
            {
                (scroll.velocity, scroll.tracked) = (0.0, 0.0);
            }
        } else if gone || !body.dragged() || self.in_progress_block_drag.is_some() {
            // a finger lifted mid-motion leaves the table coasting
            if let Some((table, Some(true))) = self.table_pan.take() {
                if let Some(scroll) = self.renderer.table_scrolls.get_mut(&table) {
                    scroll.velocity = if gone { 0.0 } else { scroll.tracked };
                }
            }
            self.table_pan = None;
        }
        let Some((table, along)) = &mut self.table_pan else { return false };
        if along.is_none() {
            let moved = origin
                .zip(pos)
                .map_or(Vec2::ZERO, |(origin, pos)| pos - origin);
            if moved.length() < SLOP {
                return true;
            }
            *along = Some(moved.x.abs() > moved.y.abs());
        }
        let scroll = self.renderer.table_scrolls.get_mut(table);
        if let (Some(true), Some(scroll)) = (*along, scroll) {
            scroll.offset -= delta.x;
            let dt = ui.input(|i| i.stable_dt).max(0.001);
            scroll.tracked = 0.6 * scroll.tracked - 0.4 * delta.x / dt;
        }
        *along == Some(true)
    }

    /// Carries out a dropped table drag: moves the row or column `drag`
    /// holds to the gap at `insert_offset`. The selection is left where
    /// the edit puts it.
    pub fn move_table_part(
        &mut self, drag: BlockDrag, insert_offset: Grapheme,
    ) -> buffer::Response {
        let arena = comrak::Arena::new();
        self.renderer.reparse(&arena);
        let r = &self.renderer;

        let mut operations = Vec::new();
        if let Some(table) = r.bounds.table_at(drag.parent_start) {
            // the gap is before row or column `gap`, the dragged one counted
            match drag.unit {
                DragUnit::Row => {
                    let from = table.row_at(drag.section_range.start()).unwrap_or(0);
                    let above = |row: &&super::RowBounds| row.line.end() <= insert_offset;
                    let gap = 1 + table.rows[1..].iter().filter(above).count();
                    let to = gap - (gap > from) as usize;
                    if from > 0 && to != from {
                        r.move_table_row(table, from, to, &mut operations);
                    }
                }
                DragUnit::Column(from) => {
                    let before = |cell: &&CellBounds| cell.span.start() < insert_offset;
                    let gap = table.rows[0].cells.iter().filter(before).count();
                    let to = gap - (gap > from) as usize;
                    if from < table.columns() && to != from {
                        r.edit_table_columns(table, ColumnEdit::Move { from, to }, &mut operations);
                    }
                }
                DragUnit::Item => {}
            }
        }

        self.renderer.buffer.queue(operations);
        let mut resp = self.renderer.buffer.update();
        if resp.text_updated {
            self.renderer.bump_text_seq();
        }
        // the drag already positioned the viewport; don't scroll-to-cursor
        resp.selection_user_moved = false;
        resp
    }

    /// The overlay of a column drag, as a block drag's: drop indicator,
    /// source cutout, and the column on a card under the pointer.
    pub(crate) fn draw_dragged_column<'a>(
        &mut self, ui: &mut Ui, root: &'a AstNode<'a>, drag: BlockDrag, column: usize, p: Pos2,
    ) {
        let r = &self.renderer;
        let theme = r.ctx.get_lb_theme();
        let primary = theme.bg().get_color(theme.prefs().primary);
        let key = drag.parent_start;
        let cells: Vec<TableCellBox> = r
            .table_boxes(key)
            .filter(|cell_box| cell_box.column == column)
            .collect();
        let Some(src_rect) = cells
            .iter()
            .map(|cell_box| cell_box.rect)
            .reduce(Rect::union)
        else {
            return;
        };
        let painter = ui.painter();

        // Drop-gap indicator: soft capsule with a hollow leading dot, which
        // starts above the table so the card doesn't hide it.
        if let Some(gap) = r.drop_gap_for(&drag, p) {
            let (dot, half) = (4.0, 1.25);
            let (x, y0, y1) = (gap.at as f32, src_rect.top() - 3.0 * dot, src_rect.bottom());
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x - 3.0, y0), Pos2::new(x + 3.0, y1)),
                CornerRadius::same(3),
                primary.gamma_multiply(0.18),
            );
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x - half, y0 + dot), Pos2::new(x + half, y1)),
                CornerRadius::same(1),
                primary,
            );
            painter.circle_filled(Pos2::new(x, y0 + dot), dot, primary);
            painter.circle_filled(Pos2::new(x, y0 + dot), dot - 1.5, theme.neutral_bg());
        }

        // Source slot: an empty ghost of the card.
        let card_corner = CornerRadius::same(4);
        let fill = theme.neutral_bg();
        let stroke = Stroke::new(0.5_f32, theme.neutral_bg_tertiary());
        painter.rect_filled(src_rect, card_corner, fill);
        painter.rect_stroke(src_rect, card_corner, stroke, StrokeKind::Inside);

        // where the column starts, which may be left of what shows of it
        let table_left = r.table_boxes(key).map(|cell_box| cell_box.rect.left());
        let unscrolled = table_left.fold(src_rect.left(), f32::min) - r.table_scrolled(key);

        // Floating card; the header's fill and the rows' rules go with it.
        let offset = (p - src_rect.left_top()) - drag.grab_offset;
        let card = src_rect.translate(offset);
        painter.rect_filled(card, card_corner, fill);
        for cell_box in &cells {
            let cell = cell_box.rect.translate(offset);
            if cell_box.row == key {
                let top_corners = CornerRadius { sw: 0, se: 0, ..card_corner };
                painter.rect_filled(cell, top_corners, theme.neutral_bg_secondary());
            } else {
                let rule = Stroke::new(1.0_f32, theme.neutral_bg_tertiary());
                painter.hline(cell.x_range(), cell.top(), rule);
            }
        }
        painter.rect_stroke(card, card_corner, stroke, StrokeKind::Inside);

        self.paint_drag_float(ui, root, |r, ui| {
            let is_table = |node: &&'a AstNode<'a>| {
                let header = node
                    .first_child()
                    .filter(|_| matches!(node.data.borrow().value, NodeValue::Table(_)));
                header.is_some_and(|header| r.table_row_key(header) == key)
            };
            let Some(table) = root.descendants().find(is_table) else { return };
            let before: f32 = r.table_column_widths(table)[..column].iter().sum();
            let left = unscrolled + before + offset.x;
            // the card shows what the table showed of the column
            let mut ui = ui.new_child(UiBuilder::new().max_rect(card));
            ui.set_clip_rect(card.intersect(ui.clip_rect()));
            for row in table.children() {
                let row_key = r.table_row_key(row);
                let cell_box = cells.iter().find(|cell_box| cell_box.row == row_key);
                if let (Some(cell_box), Some(cell)) = (cell_box, row.children().nth(column)) {
                    let top = cell_box.rect.top() + offset.y;
                    r.show_table_cell(&mut ui, cell, Pos2::new(left, top));
                }
            }
        });
    }
}

impl<'ast> MdRender {
    /// Where a row's line starts after its container's prefix: what its
    /// painted cells and its bounds know it by.
    pub fn table_row_key(&self, row: &'ast AstNode<'ast>) -> Grapheme {
        self.node_line(row, self.node_first_line(row)).start()
    }
}
