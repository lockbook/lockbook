use egui::{CursorIcon, PointerButton, Pos2, Vec2};
use ironcalc_base::{COLUMN_WIDTH_FACTOR, ROW_HEIGHT_FACTOR};

use super::ranges::Fill;
use super::*;

// UserModel reports its dimensions in pixels. Scale the defaults to the existing
// Lockbook cell sizes so sheets keep their proportions across desktop and touch.
const ENGINE_COLUMN_WIDTH: f64 = 90.0;
const ENGINE_ROW_HEIGHT: f64 = 25.0;

#[derive(Clone, Copy)]
pub(super) struct Options {
    pub(super) row_height: f32,
    pub(super) touch: bool,
    pub(super) embedded: bool,
    pub(super) read_only: bool,
}

#[derive(Default)]
pub(super) struct State {
    drag: Option<Drag>,
}

impl State {
    pub(super) fn dragging(&self) -> bool {
        self.drag.is_some()
    }
}

#[derive(Clone, Copy)]
enum Drag {
    Select,
    Move { grab: Cell },
    Fill,
    Resize { axis: AxisKind, index: i32, size: f32, press: Pos2 },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AxisKind {
    Row,
    Column,
}

impl AxisKind {
    fn cursor(self) -> CursorIcon {
        match self {
            Self::Row => CursorIcon::ResizeVertical,
            Self::Column => CursorIcon::ResizeHorizontal,
        }
    }

    fn position(self, point: Pos2) -> f32 {
        match self {
            Self::Row => point.y,
            Self::Column => point.x,
        }
    }

    fn selection_indices(self, range: CellRange) -> Option<(i32, i32)> {
        match self {
            Self::Row if range.first.column == 1 && range.last.column == COLUMNS => {
                Some((range.first.row, range.last.row))
            }
            Self::Column if range.first.row == 1 && range.last.row == ROWS => {
                Some((range.first.column, range.last.column))
            }
            _ => None,
        }
    }

    fn header_selection(self, index: i32) -> (Cell, Cell) {
        match self {
            Self::Row => (Cell { row: index, column: 1 }, Cell { row: index, column: COLUMNS }),
            Self::Column => (Cell { row: 1, column: index }, Cell { row: ROWS, column: index }),
        }
    }
}

// Store changes to the default size, with the cumulative offset at each change.
// Scrolling far from A1 therefore never walks the intervening rows or columns.
struct Axis {
    default: f64,
    last: i32,
    changes: Vec<SizeChange>,
}

#[derive(Clone, Copy)]
struct SizeChange {
    index: i32,
    offset: f64,
    delta: f64,
}

impl Axis {
    fn new(default: f64, last: i32, sizes: impl Iterator<Item = (i32, i32, f64)>) -> Self {
        let mut events = Vec::new();
        for (start, end, size) in sizes {
            let delta = size - default;
            if delta != 0.0 && start >= 1 && end <= last && start <= end {
                events.extend([(start, delta), (end + 1, -delta)]);
            }
        }
        events.sort_unstable_by_key(|&(index, _)| index);
        let mut changes = Vec::new();
        let mut previous = SizeChange { index: 1, offset: 0.0, delta: 0.0 };
        for (index, delta) in events {
            let offset = previous.offset + f64::from(index - previous.index) * previous.delta;
            previous = SizeChange { index, offset, delta: previous.delta + delta };
            changes.push(previous);
        }
        Self { default, last, changes }
    }

    fn offset(&self, index: i32) -> f32 {
        let base = f64::from(index - 1) * self.default;
        let at = self.changes.partition_point(|change| change.index <= index);
        if at == 0 {
            return base as f32;
        }
        let change = &self.changes[at - 1];
        (base + change.offset + f64::from(index - change.index) * change.delta) as f32
    }

    fn index_at(&self, offset: f32) -> i32 {
        if self.changes.is_empty() {
            return ((f64::from(offset) / self.default).floor() as i32 + 1).clamp(1, self.last);
        }
        let (mut first, mut last) = (1, self.last);
        while first < last {
            let mid = first + (last - first) / 2;
            if offset < self.offset(mid + 1) {
                last = mid;
            } else {
                first = mid + 1;
            }
        }
        first
    }

    fn visible_indices(&self, first: i32, last: i32) -> impl Iterator<Item = i32> + '_ {
        let mut index = first;
        std::iter::from_fn(move || {
            if index > last {
                return None;
            }
            let current = self.index_at(self.offset(index)).max(index);
            index = current + 1;
            (current <= last && self.offset(index) > self.offset(current)).then_some(current)
        })
    }
}

#[derive(Default)]
struct HeaderInput {
    select: Option<(AxisKind, i32)>,
    fit: Option<(AxisKind, i32)>,
    drag: Option<Drag>,
}

struct Viewport {
    origin: Pos2,
    pane: Rect,
    body: Rect,
    rows: Axis,
    columns: Axis,
}

impl Viewport {
    fn range_rect(&self, range: CellRange) -> Rect {
        Rect::from_min_max(
            self.origin
                + vec2(self.columns.offset(range.first.column), self.rows.offset(range.first.row)),
            self.origin
                + vec2(
                    self.columns.offset(range.last.column + 1),
                    self.rows.offset(range.last.row + 1),
                ),
        )
    }

    fn cell_at(&self, position: Pos2) -> Cell {
        let local = self.body.clamp(position) - self.origin;
        Cell { row: self.rows.index_at(local.y), column: self.columns.index_at(local.x) }
    }

    fn visible(&self) -> CellRange {
        CellRange::new(self.cell_at(self.body.min), self.cell_at(self.body.max))
    }
}

impl Spreadsheet {
    pub(super) fn show_grid(&mut self, ui: &mut Ui, id: Id, options: Options, reveal: bool) {
        let Options { row_height: height, touch, embedded, .. } = options;
        let max_height = if embedded {
            (height * 8.0).min(ui.available_height())
        } else {
            (ui.available_height() - height - 4.0).max(0.0)
        };
        let header_height = if touch { COLUMN_HEADER + 10.0 } else { COLUMN_HEADER };
        egui::ScrollArea::both()
            .id_salt(("grid", self.worksheet().sheet_id))
            .auto_shrink([false, false])
            .max_height(max_height)
            .scroll_source(if embedded {
                egui::scroll_area::ScrollSource::NONE
            } else {
                Default::default()
            })
            .show_viewport(ui, |ui, viewport| {
                let Ok(sheet) = self.model.get_model().workbook.worksheet(self.sheet) else {
                    return;
                };
                let filter = self.row_filter();
                let rows = Axis::new(
                    height.into(),
                    ROWS,
                    sheet
                        .rows
                        .iter()
                        .filter(|row| {
                            filter.is_none_or(|filter| filter.hidden_span(row.r).is_none())
                        })
                        .map(|row| {
                            let size = if row.hidden {
                                0.0
                            } else {
                                let size = row.height * ROW_HEIGHT_FACTOR / ENGINE_ROW_HEIGHT
                                    * f64::from(height);
                                size.clamp(f64::from(height) * 0.65, 2000.0)
                            };
                            (row.r, row.r, size)
                        })
                        .chain(filter.into_iter().flat_map(|filter| {
                            filter.hidden.iter().map(|&(start, end)| (start, end, 0.0))
                        })),
                );
                let columns = Axis::new(
                    COLUMN_WIDTH.into(),
                    COLUMNS,
                    sheet
                        .cols
                        .iter()
                        .filter(|column| column.custom_width || column.hidden)
                        .map(|column| {
                            let size = if column.hidden {
                                0.0
                            } else {
                                let size = column.width * COLUMN_WIDTH_FACTOR / ENGINE_COLUMN_WIDTH
                                    * f64::from(COLUMN_WIDTH);
                                size.clamp(36.0, 1600.0)
                            };
                            (column.min, column.max, size)
                        }),
                );
                let min = ui.min_rect().min;
                let header = vec2(ROW_HEADER, header_height);
                let pane = viewport.translate(min.to_vec2());
                let body = Rect::from_min_max(pane.min + header, pane.max);
                if !embedded {
                    ui.set_min_size(
                        header + vec2(columns.offset(COLUMNS + 1), rows.offset(ROWS + 1)),
                    );
                } else {
                    ui.set_min_height(max_height);
                }
                if !body.is_positive() {
                    return;
                }
                let viewport = Viewport { origin: min + header, pane, body, rows, columns };
                if reveal {
                    let mut rect =
                        viewport.range_rect(CellRange::new(self.range_end, self.range_end));
                    rect.min -= header;
                    ui.scroll_to_rect(rect, None);
                }
                self.paint_grid(ui, id, &viewport, options);
            });
    }

    fn paint_grid(&mut self, ui: &mut Ui, id: Id, viewport: &Viewport, options: Options) {
        let Options { row_height: height, touch, embedded, read_only } = options;
        let theme = ui.ctx().get_lb_theme();
        let accent = theme.accent();
        let background = ui.visuals().extreme_bg_color;
        let border = Stroke::new(1.0, theme.wash_toward_neutral_fg(background, 0.16));
        let show_grid_lines = self.worksheet().show_grid_lines;
        let range = self.selection();
        let visible = viewport.visible();
        let merged_cells = self.model.get_merged_cells(self.sheet).unwrap_or_default();
        let clip = ui.clip_rect();
        let body_clip = clip.intersect(viewport.body);
        let mut click = None;
        let mut start = None;

        ui.set_clip_rect(body_clip);
        if !embedded {
            ui.interact(viewport.body, id.with("grid"), Sense::focusable_noninteractive())
                .widget_info(|| WidgetInfo::labeled(egui::Role::Grid, true, "Spreadsheet grid"));
        }
        let columns: Vec<_> = viewport
            .columns
            .visible_indices(visible.first.column, visible.last.column)
            .collect();
        for row in viewport
            .rows
            .visible_indices(visible.first.row, visible.last.row)
        {
            for &column in &columns {
                let cell = Cell { row, column };
                let merged = merged_cells
                    .iter()
                    .find(|merged| merged.contains(row, column));
                if merged.is_some_and(|merged| merged.row != row || merged.column != column) {
                    continue;
                }
                let rect = merged.map_or_else(
                    || viewport.range_rect(CellRange::new(cell, cell)),
                    |merged| {
                        viewport.range_rect(CellRange::new(
                            cell,
                            Cell { row: merged.last_row(), column: merged.last_column() },
                        ))
                    },
                );
                let selected = range.contains(cell);
                let value = self
                    .model
                    .get_formatted_cell_value(self.sheet, row, column)
                    .unwrap_or_default();
                if !embedded {
                    let sense = if touch { Sense::click() } else { Sense::click_and_drag() };
                    let response = ui.interact(rect, ui.scope_id().with((row, column)), sense);
                    response.widget_info(|| {
                        WidgetInfo::selected(
                            egui::Role::Button,
                            true,
                            selected,
                            format!("{} {value}", cell.address()),
                        )
                    });
                    if response.drag_started_by(PointerButton::Primary) {
                        click = Some((cell, false));
                        start = Some(Drag::Select);
                    } else if response.clicked() || response.double_clicked() {
                        click = Some((cell, response.double_clicked()));
                    }
                }
                if selected && cell != self.selected {
                    ui.painter()
                        .rect_filled(rect, 0.0, accent.linear_multiply(0.13));
                }
                if show_grid_lines {
                    ui.painter()
                        .rect_stroke(rect, 0.0, border, StrokeKind::Middle);
                }
                if !value.is_empty() {
                    let number =
                        self.model.get_cell_type(self.sheet, row, column) == Ok(CellType::Number);
                    paint_cell_value(
                        ui,
                        rect.shrink2(vec2(6.0, 0.0)),
                        &value,
                        &self.cell_style(cell),
                        number,
                        theme.neutral_fg(),
                    );
                }
            }
        }
        if !embedded {
            let rect = viewport.range_rect(range);
            let selection_outline = Stroke::new(2.0, accent);
            ui.painter()
                .rect_stroke(rect, 0.0, selection_outline, StrokeKind::Inside);
            if range.first != range.last && visible.contains(self.selected) {
                let active = viewport.range_rect(CellRange::new(self.selected, self.selected));
                ui.painter()
                    .rect_stroke(active, 0.0, Stroke::new(1.0, accent), StrokeKind::Inside);
            }
            if !self.editing() && self.grid.drag.is_none() {
                let whole_axis = [AxisKind::Row, AxisKind::Column]
                    .into_iter()
                    .any(|axis| axis.selection_indices(range).is_some());
                if !whole_axis {
                    start = self
                        .range_handles(ui, viewport, rect, touch, read_only)
                        .or(start);
                }
            }
        }

        ui.set_clip_rect(clip);
        let headers = self.paint_headers(ui, viewport, visible, options, border);

        if embedded {
            return;
        }
        if let Some((axis, index)) = headers.select {
            if self.commit(ui, id) {
                let (cell, end) = axis.header_selection(index);
                let extend = ui.input(|i| i.modifiers.shift)
                    && axis.selection_indices(self.selection()).is_some();
                if !extend {
                    self.select(cell);
                }
                self.range_end = end;
                self.focus_grid(ui, id);
            }
        }
        if let Some((cell, double)) = click {
            let extend = ui.input(|i| i.modifiers.shift);
            if self.commit(ui, id) {
                if extend {
                    self.range_end = cell;
                } else {
                    self.select(cell);
                }
                self.focus_grid(ui, id);
                if double && !extend && !touch && !read_only {
                    self.begin_edit(ui, id, &self.input());
                }
            } else {
                start = None;
            }
        }
        if let Some((axis, index)) = headers.fit {
            self.auto_size(ui, axis, index, height);
            self.focus_grid(ui, id);
        }
        if let Some(drag) = headers.drag.or(start) {
            self.grid.drag = Some(drag);
            self.focus_grid(ui, id);
        }
        if let Some(rect) = self.drag_grid(ui, viewport, height, read_only) {
            ui.painter()
                .with_clip_rect(clip.intersect(viewport.pane))
                .rect_stroke(rect, 0.0, Stroke::new(2.0, accent), StrokeKind::Inside);
        }
    }

    fn paint_headers(
        &self, ui: &mut Ui, viewport: &Viewport, visible: CellRange, options: Options,
        border: Stroke,
    ) -> HeaderInput {
        let header_fill = ui.visuals().panel_fill;
        let range = self.selection();
        let theme = ui.ctx().get_lb_theme();
        let accent = theme.accent();
        let clip = ui.clip_rect();
        let Options { touch, embedded, read_only, .. } = options;
        let resize_enabled = !read_only && !self.editing();
        let mut input = HeaderInput::default();
        for axis in [AxisKind::Column, AxisKind::Row] {
            let (first, last, header_clip) = match axis {
                AxisKind::Column => (
                    visible.first.column,
                    visible.last.column,
                    Rect::from_min_max(
                        pos2(viewport.body.left(), viewport.pane.top()),
                        pos2(viewport.pane.right(), viewport.body.top()),
                    ),
                ),
                AxisKind::Row => (
                    visible.first.row,
                    visible.last.row,
                    Rect::from_min_max(
                        pos2(viewport.pane.left(), viewport.body.top()),
                        pos2(viewport.body.left(), viewport.pane.bottom()),
                    ),
                ),
            };
            ui.set_clip_rect(header_clip.intersect(clip));
            let sizes = match axis {
                AxisKind::Column => &viewport.columns,
                AxisKind::Row => &viewport.rows,
            };
            for index in sizes.visible_indices(first, last) {
                let (cell, _) = axis.header_selection(index);
                let cell_rect = viewport.range_rect(CellRange::new(cell, cell));
                let (rect, label, selected) = match axis {
                    AxisKind::Column => (
                        Rect::from_x_y_ranges(cell_rect.x_range(), header_clip.y_range()),
                        column_label(index),
                        (range.first.column..=range.last.column).contains(&index),
                    ),
                    AxisKind::Row => (
                        Rect::from_x_y_ranges(header_clip.x_range(), cell_rect.y_range()),
                        index.to_string(),
                        (range.first.row..=range.last.row).contains(&index),
                    ),
                };
                let painter = ui.painter();
                painter.rect(rect, 0.0, header_fill, border, StrokeKind::Middle);
                if selected && !embedded {
                    painter.rect_filled(rect.shrink(0.5), 0.0, accent.linear_multiply(0.17));
                }
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    &label,
                    FontId::proportional(12.0),
                    theme.neutral_fg(),
                );
                if !embedded {
                    let (name, resize, size) = match axis {
                        AxisKind::Column => ("column", rect.right(), rect.width()),
                        AxisKind::Row => ("row", rect.bottom(), rect.height()),
                    };
                    let header_id = ui.scope_id().with((name, index));
                    let mut label_rect = rect;
                    let mut hit = rect;
                    let margin = if touch { 12.0 } else { 5.0 };
                    if resize_enabled {
                        match axis {
                            AxisKind::Column => {
                                hit.min.x = (resize - margin).max(rect.left());
                                label_rect.max.x = hit.min.x;
                            }
                            AxisKind::Row => {
                                hit.min.y = (resize - margin).max(rect.top());
                                label_rect.max.y = hit.min.y;
                            }
                        }
                    }
                    let header = ui.interact(label_rect, header_id, Sense::click());
                    header.widget_info(|| {
                        WidgetInfo::selected(
                            egui::Role::Button,
                            true,
                            selected,
                            format!("{name} {label}"),
                        )
                    });
                    if header.clicked() {
                        input.select = Some((axis, index));
                    }
                    if resize_enabled {
                        let handle = ui
                            .interact(hit, header_id.with("resize"), Sense::click_and_drag())
                            .on_hover_and_drag_cursor(axis.cursor());
                        handle.widget_info(|| {
                            WidgetInfo::labeled(
                                egui::Role::Splitter,
                                true,
                                format!("Resize {name} {label}"),
                            )
                        });
                        if handle.hovered() {
                            let edge = match axis {
                                AxisKind::Column => [rect.right_top(), rect.right_bottom()],
                                AxisKind::Row => [rect.left_bottom(), rect.right_bottom()],
                            };
                            ui.painter().line_segment(edge, Stroke::new(2.0, accent));
                        }
                        if handle.double_clicked() {
                            input.fit = Some((axis, index));
                        } else if handle.drag_started_by(PointerButton::Primary) {
                            if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                                input.drag = Some(Drag::Resize { axis, index, size, press });
                            }
                        }
                    }
                }
            }
        }
        ui.set_clip_rect(clip);
        let corner = Rect::from_min_max(viewport.pane.min, viewport.body.min);
        ui.painter()
            .rect(corner, 0.0, header_fill, border, StrokeKind::Inside);

        input
    }

    fn focus_grid(&self, ui: &Ui, id: Id) {
        ui.memory_mut(|m| m.request_focus(id.with("grid")));
        ui.ctx().request_repaint();
    }

    fn range_handles(
        &self, ui: &Ui, viewport: &Viewport, rect: Rect, touch: bool, read_only: bool,
    ) -> Option<Drag> {
        let accent = ui.ctx().get_lb_theme().accent();
        let mut start = None;
        if !read_only {
            let grip = if touch {
                Rect::from_center_size(pos2(rect.center().x, rect.top() + 5.0), vec2(42.0, 20.0))
                    .intersect(rect)
            } else {
                Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.top() + 5.0))
            };
            let response = ui
                .interact(grip, ui.scope_id().with("move cells"), Sense::click_and_drag())
                .on_hover_and_drag_cursor(CursorIcon::Grab);
            response.widget_info(|| {
                WidgetInfo::labeled(egui::Role::Button, true, "Drag to move selected cells")
            });
            if touch {
                ui.painter().rect_filled(
                    Rect::from_center_size(grip.center(), vec2(22.0, 3.0)),
                    1.5,
                    accent,
                );
            }
            if response.drag_started_by(PointerButton::Primary) {
                if let Some(position) = ui.input(|i| i.pointer.press_origin()) {
                    start = Some(Drag::Move { grab: viewport.cell_at(position) });
                }
            }
        }
        if touch || !read_only {
            let size = if touch { 11.0 } else { 7.0 };
            let center = rect.right_bottom() - vec2(size * 0.5, size * 0.5);
            let handle = Rect::from_center_size(center, Vec2::splat(size));
            let hit = handle.expand(if touch { 10.0 } else { 2.0 });
            let label = if touch { "Drag to extend selection" } else { "Drag to fill cells" };
            let response = ui
                .interact(hit, ui.scope_id().with("extend cells"), Sense::click_and_drag())
                .on_hover_and_drag_cursor(CursorIcon::Crosshair);
            response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, label));
            response.clone().on_hover_text(label);
            ui.painter()
                .rect_filled(handle, if touch { 5.5 } else { 1.0 }, accent);
            if response.drag_started_by(PointerButton::Primary) {
                start = Some(if touch { Drag::Select } else { Drag::Fill });
            }
        }
        start
    }

    fn drag_grid(
        &mut self, ui: &Ui, viewport: &Viewport, height: f32, read_only: bool,
    ) -> Option<Rect> {
        let drag = self.grid.drag?;
        let (position, held, released, cancelled) = ui.input_mut(|i| {
            let cancelled = i.consume_key(Modifiers::NONE, Key::Escape)
                || i.events.iter().any(|event| {
                    matches!(event, Event::Touch { phase: egui::TouchPhase::Cancel, .. })
                });
            (
                i.pointer.interact_pos(),
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                cancelled,
            )
        });
        if !held || cancelled || (read_only && !matches!(drag, Drag::Select)) {
            self.grid.drag = None;
        }
        if cancelled || (!held && !released) || (read_only && !matches!(drag, Drag::Select)) {
            ui.ctx().request_repaint();
            return None;
        }
        let position = position?;
        let range = self.selection();
        if let Drag::Resize { axis, index, size, press } = drag {
            let desired = size + axis.position(position) - axis.position(press);
            let size = resize_limit(axis, desired, height);
            if released {
                self.resize(axis, index, size, height);
                ui.ctx().request_repaint();
                return None;
            }
            let rect = match axis {
                AxisKind::Column => {
                    let x = viewport.origin.x + viewport.columns.offset(index) + size;
                    Rect::from_min_size(
                        pos2(x, viewport.pane.top()),
                        vec2(1.0, viewport.pane.height()),
                    )
                }
                AxisKind::Row => {
                    let y = viewport.origin.y + viewport.rows.offset(index) + size;
                    Rect::from_min_size(
                        pos2(viewport.pane.left(), y),
                        vec2(viewport.pane.width(), 1.0),
                    )
                }
            };
            return Some(rect);
        }

        let cell = viewport.cell_at(position);
        if held {
            let delta = edge_scroll(viewport.body, position);
            if delta != Vec2::ZERO {
                ui.scroll_with_delta_animation(delta, egui::style::ScrollAnimation::none());
                ui.ctx().request_repaint();
            }
        }
        let drop = released && viewport.body.expand(2.0).contains(position);
        let previous_end = self.range_end;
        let preview = match drag {
            Drag::Select => {
                self.range_end = cell;
                None
            }
            Drag::Fill => Fill::toward(range, cell).map(|fill| {
                if drop {
                    self.fill_selection(range, fill);
                }
                viewport.range_rect(fill.extent(range))
            }),
            Drag::Move { grab } => {
                let rows = (cell.row - grab.row).clamp(1 - range.first.row, ROWS - range.last.row);
                let columns = (cell.column - grab.column)
                    .clamp(1 - range.first.column, COLUMNS - range.last.column);
                let target = CellRange::new(
                    range.first.offset(rows, columns),
                    range.last.offset(rows, columns),
                );
                if drop {
                    self.move_selection(target);
                }
                (target != range).then(|| viewport.range_rect(target))
            }
            Drag::Resize { .. } => None,
        };
        if released || self.range_end != previous_end {
            ui.ctx().request_repaint();
        }
        preview
    }

    fn auto_size(&mut self, ui: &Ui, axis: AxisKind, index: i32, height: f32) {
        let mut size = if axis == AxisKind::Row { height } else { 0.0 };
        if axis == AxisKind::Column {
            if let Ok(sheet) = self.model.get_model().workbook.worksheet(self.sheet) {
                for row in sheet.sheet_data.rows() {
                    let text = self
                        .model
                        .get_formatted_cell_value(self.sheet, row, index)
                        .unwrap_or_default();
                    if !text.is_empty() {
                        let galley = ui.painter().layout_no_wrap(
                            text,
                            FontId::proportional(14.0),
                            Color32::WHITE,
                        );
                        size = size.max(galley.size().x + 14.0);
                    }
                }
            }
            if size == 0.0 {
                size = COLUMN_WIDTH;
            }
        }
        self.resize(axis, index, resize_limit(axis, size, height), height);
    }

    fn resize(&mut self, axis: AxisKind, index: i32, size: f32, height: f32) {
        let (first, last) = axis
            .selection_indices(self.selection())
            .filter(|&(first, last)| (first..=last).contains(&index))
            .unwrap_or((index, index));
        if last - first >= 1_000 {
            self.error = Some("Select fewer rows or columns to resize at once".into());
            return;
        }
        let result = match axis {
            AxisKind::Column => self.model.set_columns_width(
                self.sheet,
                first,
                last,
                f64::from(size) * ENGINE_COLUMN_WIDTH / f64::from(COLUMN_WIDTH),
            ),
            AxisKind::Row => self.model.set_rows_height(
                self.sheet,
                first,
                last,
                f64::from(size) * ENGINE_ROW_HEIGHT / f64::from(height),
            ),
        };
        self.record_change(result);
    }
}

fn resize_limit(axis: AxisKind, size: f32, height: f32) -> f32 {
    match axis {
        AxisKind::Column => size.clamp(36.0, 1600.0),
        AxisKind::Row => size.clamp(height * 0.65, 1200.0),
    }
}

fn edge_scroll(body: Rect, position: Pos2) -> Vec2 {
    let beyond = |value: f32, low: f32, high: f32| {
        let margin = ((high - low) / 3.0).min(24.0);
        (value.clamp(low + margin, high - margin) - value).clamp(-36.0, 36.0) * 0.6
    };
    vec2(
        beyond(position.x, body.left(), body.right()),
        beyond(position.y, body.top(), body.bottom()),
    )
}
