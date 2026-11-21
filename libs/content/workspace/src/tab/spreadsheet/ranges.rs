use ironcalc_base::expressions::types::CellReferenceIndex;
use ironcalc_base::{ClipboardData, Model};
use serde::Deserialize;

use super::grid::AxisKind;
use super::*;

#[derive(Clone, Copy)]
pub(super) enum AxisEdit {
    InsertBefore,
    InsertAfter,
    Delete,
}

#[derive(Clone, Copy)]
pub(super) enum Fill {
    Rows(i32),
    Columns(i32),
}

impl Fill {
    pub(super) fn toward(source: CellRange, cell: Cell) -> Option<Self> {
        let rows = (cell.row - cell.row.clamp(source.first.row, source.last.row)).abs();
        let columns =
            (cell.column - cell.column.clamp(source.first.column, source.last.column)).abs();
        match (rows, columns) {
            (0, 0) => None,
            _ if rows >= columns => Some(Self::Rows(cell.row)),
            _ => Some(Self::Columns(cell.column)),
        }
    }

    pub(super) fn extent(self, source: CellRange) -> CellRange {
        let mut target = source;
        match self {
            Self::Rows(row) => {
                target.first.row = source.first.row.min(row);
                target.last.row = source.last.row.max(row);
            }
            Self::Columns(column) => {
                target.first.column = source.first.column.min(column);
                target.last.column = source.last.column.max(column);
            }
        }
        target
    }
}

// UserModel returns a serializable clipboard with private fields. Read its payload
// using the same keys as the upstream native-to-web adapter.
#[derive(Deserialize)]
struct EngineClipboard {
    data: ClipboardData,
}

impl Spreadsheet {
    pub(super) fn sort_rows_by_selected_column(&mut self, descending: bool) {
        let Ok(worksheet) = self.model.get_model().workbook.worksheet(self.sheet) else {
            return;
        };
        let dimension = worksheet.dimension();
        let first = 2; // Keep row 1 as the table header.
        let last = dimension.max_row;
        if last < first || i64::from(last - first + 1) > MAX_EDIT_CELLS {
            self.error = Some("This table is too large to sort at once".into());
            return;
        }
        let column = self.selected.column;
        let hidden_rows = contiguous_spans(
            worksheet
                .rows
                .iter()
                .filter(|row| row.hidden && (first..=last).contains(&row.r))
                .map(|row| row.r),
        );
        let mut ordered = (first..=last)
            .map(|row| {
                self.model
                    .get_formatted_cell_value(self.sheet, row, column)
                    .map(|value| (row, value))
            })
            .collect::<Result<Vec<_>, _>>();
        let Ok(ref mut ordered) = ordered else {
            self.error = Some("Could not read values to sort".into());
            return;
        };
        ordered.sort_by(|(_, left), (_, right)| {
            let order = left.to_lowercase().cmp(&right.to_lowercase());
            if descending { order.reverse() } else { order }
        });
        let sheet = self.sheet;
        let result = self.try_bulk_edit(|model, _| {
            let mut steps = 0;
            for (start, end) in &hidden_rows {
                model.set_rows_hidden(sheet, *start, *end, false)?;
                steps += 1;
            }
            let mut current: Vec<i32> = (first..=last).collect();
            for (offset, (original_row, _)) in ordered.iter().enumerate() {
                let target = first + offset as i32;
                let Some(index) = current.iter().position(|row| row == original_row) else {
                    return Err("Could not complete the sort".into());
                };
                let source = first + index as i32;
                if source != target {
                    model.move_rows_action(sheet, source, 1, target - source)?;
                    let moved = current.remove(index);
                    current.insert(offset, moved);
                    steps += 1;
                }
            }
            Ok(steps)
        });
        if result.as_ref().is_ok_and(|steps| *steps == 0) {
            return;
        }
        self.record_edit(result);
    }

    pub(super) fn filter_rows_by_selected_value(&mut self) {
        let Ok(worksheet) = self.model.get_model().workbook.worksheet(self.sheet) else {
            return;
        };
        let last = worksheet.dimension().max_row;
        if last < 2 || i64::from(last - 1) > MAX_EDIT_CELLS {
            self.error = Some("This table is too large to filter at once".into());
            return;
        }
        let column = self.selected.column;
        let mut previously_hidden = worksheet
            .rows
            .iter()
            .filter(|row| row.hidden && (2..=last).contains(&row.r))
            .map(|row| row.r)
            .collect::<Vec<_>>();
        let selected_value = self
            .model
            .get_formatted_cell_value(self.sheet, self.selected.row, column)
            .unwrap_or_default();
        let mut hidden = Vec::new();
        let mut row = 2;
        while row <= last {
            let matches = self
                .model
                .get_formatted_cell_value(self.sheet, row, column)
                .is_ok_and(|value| value == selected_value);
            if !matches {
                let start = row;
                while row <= last
                    && self
                        .model
                        .get_formatted_cell_value(self.sheet, row, column)
                        .is_ok_and(|value| value != selected_value)
                {
                    row += 1;
                }
                hidden.push((start, row - 1));
            } else {
                row += 1;
            }
        }
        let sheet = self.sheet;
        let mut desired_hidden = hidden
            .into_iter()
            .flat_map(|(start, end)| start..=end)
            .collect::<Vec<_>>();
        previously_hidden.sort_unstable();
        desired_hidden.sort_unstable();
        if previously_hidden == desired_hidden {
            return;
        }
        let clear_spans = contiguous_spans(previously_hidden);
        let hide_spans = contiguous_spans(desired_hidden);
        let result = self.try_bulk_edit(|model, _| {
            let mut steps = 0;
            for (start, end) in &clear_spans {
                model.set_rows_hidden(sheet, *start, *end, false)?;
                steps += 1;
            }
            for (start, end) in hide_spans {
                model.set_rows_hidden(sheet, start, end, true)?;
                steps += 1;
            }
            Ok(steps)
        });
        self.record_edit(result);
    }

    pub(super) fn clear_row_filter(&mut self) {
        let Ok(worksheet) = self.model.get_model().workbook.worksheet(self.sheet) else {
            return;
        };
        let last = worksheet.dimension().max_row;
        if last < 2 {
            return;
        }
        let hidden = contiguous_spans(
            worksheet
                .rows
                .iter()
                .filter(|row| row.hidden && (2..=last).contains(&row.r))
                .map(|row| row.r),
        );
        if hidden.is_empty() {
            return;
        }
        let sheet = self.sheet;
        let result = self.try_bulk_edit(|model, _| {
            for (start, end) in &hidden {
                model.set_rows_hidden(sheet, *start, *end, false)?;
            }
            Ok(hidden.len())
        });
        self.record_edit(result);
    }

    pub(super) fn merge_selection(&mut self, center: bool) {
        let range = self.selection();
        let area = range.area(self.sheet);
        if range == CellRange::new(range.first, range.first) {
            return;
        }
        let result = check_size(&area).and_then(|()| {
            if center {
                self.model.merge_cells_center(&area)
            } else {
                self.model.merge_cells(&area)
            }
        });
        if self.record_change(result) {
            self.selected = range.first;
            self.range_end = range.last;
        }
    }

    pub(super) fn unmerge_selection(&mut self) {
        let area = self.selection().area(self.sheet);
        let result = self.model.unmerge_cells(&area);
        self.record_change(result);
    }

    pub(super) fn move_axis(&mut self, axis: AxisKind, delta: i32) {
        let range = self.selection();
        let (start, count) = match axis {
            AxisKind::Row if range.first.column == 1 && range.last.column == COLUMNS => {
                (range.first.row, range.last.row - range.first.row + 1)
            }
            AxisKind::Column if range.first.row == 1 && range.last.row == ROWS => {
                (range.first.column, range.last.column - range.first.column + 1)
            }
            _ => return,
        };
        let sheet = self.sheet;
        let result = self.try_bulk_edit(|model, _| match axis {
            AxisKind::Row => model.move_rows_action(sheet, start, count, delta),
            AxisKind::Column => model.move_columns_action(sheet, start, count, delta),
        });
        if self.record_change(result) {
            let start = start + delta;
            self.selected = match axis {
                AxisKind::Row => Cell { row: start, column: 1 },
                AxisKind::Column => Cell { row: 1, column: start },
            };
            self.range_end = match axis {
                AxisKind::Row => Cell { row: start + count - 1, column: COLUMNS },
                AxisKind::Column => Cell { row: ROWS, column: start + count - 1 },
            };
        }
    }

    pub(super) fn clear_selection(&mut self) -> bool {
        let mut area = self.selection().area(self.sheet);
        if check_size(&area).is_err() {
            area = self.used_selection().area(self.sheet);
        }
        if area.width == 1
            && area.height == 1
            && self
                .model
                .get_cell_content(self.sheet, area.row, area.column)
                .is_ok_and(|value| value.is_empty())
            && self.model.get_cell_link(self.sheet, area.row, area.column) == Ok(None)
        {
            return false;
        }
        let result = check_size(&area).and_then(|()| self.model.range_clear_contents(&area));
        self.record_change(result)
    }

    pub(super) fn format_selection(&mut self, property: &str, value: &str) {
        let area = self.selection().area(self.sheet);
        // IronCalc represents full rows and columns using axis styles.
        let result =
            if area.height == ROWS || area.width == COLUMNS { Ok(()) } else { check_size(&area) };
        let result = result.and_then(|()| self.model.update_range_style(&area, property, value));
        self.record_change(result);
    }

    pub(super) fn clear_formatting(&mut self) {
        let area = self.selection().area(self.sheet);
        let result =
            if area.height == ROWS || area.width == COLUMNS { Ok(()) } else { check_size(&area) };
        let result = result.and_then(|()| self.model.range_clear_formatting(&area));
        self.record_change(result);
    }

    pub(super) fn edit_axis(&mut self, axis: AxisKind, edit: AxisEdit) {
        let mut range = self.selection();
        let (first, last, limit) = match axis {
            AxisKind::Row => (&mut range.first.row, &mut range.last.row, ROWS),
            AxisKind::Column => (&mut range.first.column, &mut range.last.column, COLUMNS),
        };
        let count = *last - *first + 1;
        let index = if matches!(edit, AxisEdit::InsertAfter) { *last + 1 } else { *first };
        if index + count - 1 > limit {
            self.error = Some("The inserted cells would extend past the worksheet".into());
            return;
        }
        if i64::from(count) > MAX_EDIT_CELLS {
            self.error = Some("Select fewer rows or columns to edit at once".into());
            return;
        }
        let sheet = self.sheet;
        let result = self.try_bulk_edit(|model, _| match (axis, edit) {
            (AxisKind::Row, AxisEdit::Delete) => model.delete_rows(sheet, index, count),
            (AxisKind::Column, AxisEdit::Delete) => model.delete_columns(sheet, index, count),
            (AxisKind::Row, _) => model.insert_rows(sheet, index, count),
            (AxisKind::Column, _) => model.insert_columns(sheet, index, count),
        });
        if self.record_change(result) {
            *first = index;
            *last = if matches!(edit, AxisEdit::Delete) { index } else { index + count - 1 };
            self.selected = range.first;
            self.range_end = range.last;
        }
    }

    pub(super) fn move_selection(&mut self, target: CellRange) {
        let source = self.selection();
        if source == target {
            return;
        }
        let sheet = self.sheet;
        let result = (|| {
            check_size(&source.area(sheet))?;
            self.model.set_selected_sheet(sheet)?;
            self.model
                .set_selected_cell(source.first.row, source.first.column)?;
            self.model.set_selected_range(
                source.first.row,
                source.first.column,
                source.last.row,
                source.last.column,
            )?;
            let [first_row, first_column, last_row, last_column] =
                self.model.get_selected_view().range;
            let bounds = (first_row, first_column, last_row, last_column);
            if bounds
                != (source.first.row, source.first.column, source.last.row, source.last.column)
            {
                return Err("Select the entire merged range before moving it".into());
            }
            let clipboard = self.model.copy_to_clipboard()?;
            let payload = serde_json::to_value(clipboard).map_err(|error| error.to_string())?;
            let mut clipboard: EngineClipboard =
                serde_json::from_value(payload).map_err(|error| error.to_string())?;
            // IronCalc's clipboard omits trailing cells outside the used area.
            // Include those cells so the drop matches the outline in the grid.
            for row in first_row..=last_row {
                let columns = clipboard.data.entry(row).or_default();
                for column in first_column..=last_column {
                    if let std::collections::hash_map::Entry::Vacant(entry) = columns.entry(column)
                    {
                        let style = self.cell_style(Cell { row, column });
                        let payload =
                            serde_json::json!({"text": "", "is_spill": false, "style": style});
                        entry.insert(
                            serde_json::from_value(payload).map_err(|error| error.to_string())?,
                        );
                    }
                }
            }
            self.model
                .set_selected_cell(target.first.row, target.first.column)?;
            let view = self.model.get_selected_view();
            if (view.row, view.column) != (target.first.row, target.first.column) {
                return Err("Drop on the first cell of the merged range".into());
            }
            self.try_bulk_edit(|model, before| {
                let corrections = if source.intersects(target) {
                    moved_formulas(before, sheet, source, target)?
                } else {
                    Vec::new()
                };
                model.paste_from_clipboard(sheet, bounds, &clipboard.data, true)?;
                let mut steps = 1;
                for (cell, formula) in corrections {
                    if model.get_cell_content(sheet, cell.row, cell.column)? != formula {
                        model.set_user_input(sheet, cell.row, cell.column, &formula)?;
                        steps += 1;
                    }
                }
                Ok(steps)
            })
        })();
        if self.record_edit(result) {
            self.select(target.first);
            self.range_end = target.last;
        }
    }

    pub(super) fn fill_selection(&mut self, source: CellRange, fill: Fill) {
        let extent = fill.extent(source);
        let area = source.area(self.sheet);
        let result = check_size(&extent.area(self.sheet)).and_then(|()| {
            self.try_bulk_edit(|model, _| match fill {
                Fill::Rows(row) => model.auto_fill_rows(&area, row),
                Fill::Columns(column) => model.auto_fill_columns(&area, column),
            })
        });
        if self.record_change(result) {
            self.select(extent.first);
            self.range_end = extent.last;
        }
    }

    pub(super) fn copy_selection(&mut self, ui: &Ui) {
        match self.selection_text() {
            Ok(text) => ui.ctx().copy_text(text),
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn cut_selection(&mut self, ui: &Ui) {
        match self.selection_text() {
            Ok(text) => {
                ui.ctx().copy_text(text);
                self.clear_selection();
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn paste(&mut self, text: &str) {
        let result = self.paste_text(text);
        match result {
            Ok(Some(range)) => {
                self.record_change(Ok(()));
                self.select(range.first);
                self.range_end = range.last;
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }

    fn used_selection(&self) -> CellRange {
        let range = self.selection();
        let Ok(sheet) = self.model.get_model().workbook.worksheet(self.sheet) else {
            return range;
        };
        let dimension = sheet.dimension();
        CellRange {
            first: range.first,
            last: Cell {
                row: range.last.row.min(dimension.max_row).max(range.first.row),
                column: range
                    .last
                    .column
                    .min(dimension.max_column)
                    .max(range.first.column),
            },
        }
    }

    fn selection_text(&self) -> Result<String, String> {
        let range = self.selection();
        // A header selection spans an entire axis, so copy only its used portion.
        let range = if range.last.row - range.first.row == ROWS - 1
            || range.last.column - range.first.column == COLUMNS - 1
        {
            self.used_selection()
        } else {
            range
        };
        check_size(&range.area(self.sheet))?;
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_writer(Vec::new());
        for row in range.first.row..=range.last.row {
            let fields = (range.first.column..=range.last.column)
                .map(|column| self.model.get_formatted_cell_value(self.sheet, row, column))
                .collect::<Result<Vec<_>, _>>()?;
            writer
                .write_record(fields)
                .map_err(|error| error.to_string())?;
        }
        let bytes = writer.into_inner().map_err(|error| error.to_string())?;
        let mut text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
        text.pop(); // The writer terminates the final row with a newline.
        Ok(text)
    }

    fn paste_text(&mut self, text: &str) -> Result<Option<CellRange>, String> {
        let first = self.selection().first;
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let mut records = Vec::new();
        let mut width = 0;
        for record in reader.records() {
            let record = record.map_err(|error| error.to_string())?;
            width = width.max(record.len());
            if width > (COLUMNS - first.column + 1) as usize
                || records.len() >= (ROWS - first.row + 1) as usize
            {
                return Err("The clipboard data does not fit in this worksheet".into());
            }
            if width * (records.len() + 1) > MAX_EDIT_CELLS as usize {
                return Err("Too many cells to paste at once".into());
            }
            records.push(record);
        }
        if records.is_empty() {
            return Ok(None);
        }
        let range = CellRange::new(first, first.offset(records.len() as i32 - 1, width as i32 - 1));
        // IronCalc expects every row in a paste to have the same number of fields.
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_writer(Vec::new());
        for record in records {
            let fields = record.iter().chain(std::iter::repeat("")).take(width);
            writer
                .write_record(fields)
                .map_err(|error| error.to_string())?;
        }
        let bytes = writer.into_inner().map_err(|error| error.to_string())?;
        let text = std::str::from_utf8(&bytes).map_err(|error| error.to_string())?;
        self.model.set_selected_sheet(self.sheet)?;
        self.model.set_selected_cell(first.row, first.column)?;
        let area = range.area(self.sheet);
        self.try_bulk_edit(|model, _| model.paste_csv_string(&area, text))?;
        Ok(Some(range))
    }

    fn try_bulk_edit<T>(
        &mut self, edit: impl FnOnce(&mut UserModel<'static>, &[u8]) -> Result<T, String>,
    ) -> Result<T, String> {
        // Bulk operations can fail after modifying cells, before adding an undo entry.
        let before = self.model.to_bytes();
        self.model.pause_evaluation();
        let result = edit(&mut self.model, &before);
        self.model.resume_evaluation();
        if result.is_err() && self.model.to_bytes() != before {
            self.model = UserModel::from_bytes(&before, "en")?;
            self.undo_steps.clear();
            self.redo_steps.clear();
        }
        if result.is_ok() {
            self.model.evaluate();
        }
        result
    }
}

fn moved_formulas(
    before: &[u8], sheet: u32, source: CellRange, target: CellRange,
) -> Result<Vec<(Cell, String)>, String> {
    // IronCalc's cut updates references in destination formulas outside the
    // source even though they have already been updated by the move itself.
    let mut original = Model::from_bytes(before, "en")?;
    let area = source.area(sheet);
    let mut formulas = Vec::new();
    for row in source.first.row..=source.last.row {
        for column in source.first.column..=source.last.column {
            let cell = target
                .first
                .offset(row - source.first.row, column - source.first.column);
            if source.contains(cell) {
                continue;
            }
            let content = original.get_localized_cell_content(sheet, row, column)?;
            if content.starts_with('=') {
                let from = CellReferenceIndex { sheet, row, column };
                let to = CellReferenceIndex { sheet, row: cell.row, column: cell.column };
                let formula = original.move_cell_value_to_area(&content, &from, &to, &area)?;
                formulas.push((cell, formula));
            }
        }
    }
    Ok(formulas)
}

fn check_size(area: &Area) -> Result<(), String> {
    if i64::from(area.width) * i64::from(area.height) > MAX_EDIT_CELLS {
        return Err("Too many cells to edit at once. Select a smaller range.".into());
    }
    Ok(())
}

fn contiguous_spans(rows: impl IntoIterator<Item = i32>) -> Vec<(i32, i32)> {
    let mut rows: Vec<_> = rows.into_iter().collect();
    rows.sort_unstable();
    rows.dedup();
    let mut spans = Vec::new();
    for row in rows {
        match spans.last_mut() {
            Some((_, last)) if *last + 1 == row => *last = row,
            _ => spans.push((row, row)),
        }
    }
    spans
}
