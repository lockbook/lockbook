use egui::text::LayoutJob;
use egui::{
    Align2, Button, Color32, Event, FontFamily, FontId, Id, Key, Modifiers, Rect, RichText, Sense,
    Stroke, StrokeKind, TextFormat, Ui, WidgetInfo,
};
use egui::{pos2, vec2};
use ironcalc_base::UserModel;
use ironcalc_base::expressions::types::Area;
use ironcalc_base::types::{CellType, HorizontalAlignment, Style, Workbook};
use lb_rs::Uuid;
use lb_rs::model::file_metadata::DocumentHmac;

use super::markdown_editor::{MdEdit, Syntax, View};
use super::{ExtendedOutput as _, Response, TabFailure};
use crate::style::chrome::{phosphor, phosphor_font_id};
use crate::theme::palette_v2::ThemeExt as _;

mod grid;
mod ranges;

// Version 1 uses the workbook encoding of ironcalc_base 0.9.1.
const HEADER: &[u8] = b"LBSHEET\0\x01";
const ROWS: i32 = 1_048_576;
const COLUMNS: i32 = 16_384;
const COLUMN_WIDTH: f32 = 112.0;
const ROW_HEADER: f32 = 62.0;
const COLUMN_HEADER: f32 = 26.0;
// IronCalc creates one or more undo records per cell in rectangular edits.
const MAX_EDIT_CELLS: i64 = 100_000;
const ALIGNMENTS: [(HorizontalAlignment, &str, &str); 3] = [
    (HorizontalAlignment::Left, "Align left", phosphor::TEXT_ALIGN_LEFT),
    (HorizontalAlignment::Center, "Align center", phosphor::TEXT_ALIGN_CENTER),
    (HorizontalAlignment::Right, "Align right", phosphor::TEXT_ALIGN_RIGHT),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    row: i32,
    column: i32,
}

impl Cell {
    const FIRST: Self = Self { row: 1, column: 1 };

    fn address(self) -> String {
        format!("{}{}", column_label(self.column), self.row)
    }

    fn offset(self, rows: i32, columns: i32) -> Self {
        Self { row: self.row + rows, column: self.column + columns }
    }

    fn clamped(self) -> Self {
        Self { row: self.row.clamp(1, ROWS), column: self.column.clamp(1, COLUMNS) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellRange {
    first: Cell,
    last: Cell,
}

impl CellRange {
    fn new(anchor: Cell, end: Cell) -> Self {
        Self {
            first: Cell { row: anchor.row.min(end.row), column: anchor.column.min(end.column) },
            last: Cell { row: anchor.row.max(end.row), column: anchor.column.max(end.column) },
        }
    }

    fn contains(self, cell: Cell) -> bool {
        (self.first.row..=self.last.row).contains(&cell.row)
            && (self.first.column..=self.last.column).contains(&cell.column)
    }

    fn intersects(self, other: Self) -> bool {
        self.first.row <= other.last.row
            && other.first.row <= self.last.row
            && self.first.column <= other.last.column
            && other.first.column <= self.last.column
    }

    fn area(self, sheet: u32) -> Area {
        Area {
            sheet,
            row: self.first.row,
            column: self.first.column,
            width: self.last.column - self.first.column + 1,
            height: self.last.row - self.first.row + 1,
        }
    }

    fn address(self) -> String {
        if self.first == self.last {
            self.first.address()
        } else if self.first.row == 1 && self.last.row == ROWS {
            format!("{}:{}", column_label(self.first.column), column_label(self.last.column))
        } else if self.first.column == 1 && self.last.column == COLUMNS {
            format!("{}:{}", self.first.row, self.last.row)
        } else {
            format!("{}:{}", self.first.address(), self.last.address())
        }
    }
}

pub struct Spreadsheet {
    pub id: Uuid,
    pub hmac: Option<DocumentHmac>,
    pub recovery: Option<(Uuid, DocumentHmac)>,
    pub recovered: Option<Uuid>,
    pub reload_pending: bool,
    pub save_error: Option<String>,
    model: UserModel<'static>,
    selected: Cell,
    range_end: Cell,
    sheet: u32,
    grid: grid::State,
    edit: Option<Box<MdEdit>>,
    error: Option<String>,
    seq: usize,
    undo_steps: Vec<usize>,
    redo_steps: Vec<usize>,
}

impl Spreadsheet {
    pub fn new(id: Uuid, hmac: Option<DocumentHmac>, bytes: &[u8]) -> Result<Self, TabFailure> {
        Ok(Self {
            id,
            hmac,
            recovery: None,
            recovered: None,
            reload_pending: false,
            save_error: None,
            model: read_workbook(hmac, bytes).map_err(TabFailure::SimpleMisc)?,
            selected: Cell::FIRST,
            range_end: Cell::FIRST,
            sheet: 0,
            grid: grid::State::default(),
            edit: None,
            error: None,
            seq: 0,
            undo_steps: Vec::new(),
            redo_steps: Vec::new(),
        })
    }

    pub fn reload(&mut self, hmac: Option<DocumentHmac>, bytes: &[u8]) -> Result<(), TabFailure> {
        self.recovery = None;
        if self.hmac == hmac {
            return Ok(());
        }

        self.model = read_workbook(hmac, bytes).map_err(TabFailure::SimpleMisc)?;
        self.hmac = hmac;
        self.save_error = None;
        self.sheet = self
            .sheet
            .min(self.model.get_model().workbook.worksheets.len() as u32 - 1);
        self.grid = grid::State::default();
        self.undo_steps.clear();
        self.redo_steps.clear();
        Ok(())
    }

    pub fn snapshot(&self) -> Workbook {
        self.model.get_model().workbook.clone()
    }

    pub fn seq(&self) -> usize {
        self.seq
    }

    pub fn editing(&self) -> bool {
        self.edit.is_some()
    }

    pub fn edit_mut(&mut self) -> Option<&mut MdEdit> {
        self.edit.as_deref_mut()
    }

    fn input(&self) -> String {
        self.model
            .get_cell_content(self.sheet, self.selected.row, self.selected.column)
            .unwrap_or_default()
    }

    fn selection(&self) -> CellRange {
        CellRange::new(self.selected, self.range_end)
    }

    fn select(&mut self, cell: Cell) {
        self.selected = cell;
        self.range_end = cell;
    }

    fn cell_style(&self, cell: Cell) -> Style {
        self.model
            .get_model()
            .get_style_for_cell(self.sheet, cell.row, cell.column)
            .unwrap_or_default()
    }

    fn begin_edit(&mut self, ui: &Ui, id: Id, input: &str) {
        let mut field = MdEdit::empty(ui.ctx().clone());
        field.renderer.kind = Syntax::Plain;
        field.renderer.view = View::Source;
        field.renderer.single_line = true;
        field.file_id = self.id;
        field.set_text(input);
        let end = field.renderer.buffer.current.segs.last_cursor_position();
        field.renderer.buffer.current.selection = (end, end);
        self.edit = Some(Box::new(field));
        ui.memory_mut(|m| m.request_focus(id));
        ui.ctx().set_virtual_keyboard_shown(true);
        ui.ctx().request_repaint();
    }

    fn end_edit(&mut self, ui: &Ui, id: Id) {
        self.edit = None;
        self.error = None;
        ui.memory_mut(|m| m.request_focus(id.with("grid")));
        ui.ctx().set_virtual_keyboard_shown(false);
        ui.ctx().request_repaint();
    }

    fn commit(&mut self, ui: &Ui, id: Id) -> bool {
        let Some(field) = self.edit.as_ref() else { return true };
        let text = field
            .renderer
            .buffer
            .current
            .text
            .trim_end_matches(['\n', '\r'])
            .replace(['\n', '\r'], " ");
        if text != self.input() {
            let Cell { row, column } = self.selected;
            let result = self.model.set_user_input(self.sheet, row, column, &text);
            if !self.record_change(result) {
                return false;
            }
        }
        self.end_edit(ui, id);
        true
    }

    fn record_change(&mut self, result: Result<(), String>) -> bool {
        self.record_edit(result.map(|()| 1))
    }

    fn record_edit(&mut self, result: Result<usize, String>) -> bool {
        match result {
            Ok(steps) => {
                self.undo_steps.push(steps);
                self.redo_steps.clear();
                self.finish_change(Ok(()))
            }
            Err(error) => self.finish_change(Err(error)),
        }
    }

    fn finish_change(&mut self, result: Result<(), String>) -> bool {
        match result {
            Ok(()) => {
                self.error = None;
                self.seq += 1;
                let _ = self.model.flush_send_queue();
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    fn change_history(&mut self, redo: bool) {
        let (from, to) = if redo && self.model.can_redo() {
            (&mut self.redo_steps, &mut self.undo_steps)
        } else if !redo && self.model.can_undo() {
            (&mut self.undo_steps, &mut self.redo_steps)
        } else {
            return;
        };
        let steps = from.pop().unwrap_or(1);
        self.model.pause_evaluation();
        let result =
            (0..steps).try_for_each(|_| if redo { self.model.redo() } else { self.model.undo() });
        self.model.resume_evaluation();
        self.model.evaluate();
        if result.is_ok() {
            to.push(steps);
        }
        if self.finish_change(result) {
            let count = self.model.get_model().workbook.worksheets.len() as u32;
            if self.sheet >= count {
                self.sheet = count - 1;
                self.select(Cell::FIRST);
            }
        }
    }

    fn update_style(&mut self, property: &str, value: &str) {
        self.format_selection(property, value);
    }

    pub fn show(&mut self, ui: &mut Ui, read_only: bool, embedded: bool) -> Response {
        // The workspace justifies a single child to fill the tab.
        ui.vertical(|ui| self.show_contents(ui, read_only, embedded))
            .inner
    }

    fn show_contents(&mut self, ui: &mut Ui, read_only: bool, embedded: bool) -> Response {
        let mut response = Response::default();
        let id = ui.make_persistent_id("formula");
        let touch = matches!(
            ui.ctx().os(),
            egui::os::OperatingSystem::Android | egui::os::OperatingSystem::IOS
        );
        let height = if touch { 44.0 } else { 28.0 };
        let grid_id = id.with("grid");
        let was_editing = self.editing();
        if !embedded {
            let layer = ui.layer_id();
            ui.memory_mut(|m| {
                m.interested_in_focus(grid_id, layer);
                m.set_focus_lock_filter(
                    grid_id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                );
            });
        }
        if (read_only || embedded) && self.editing() {
            self.end_edit(ui, id);
        }
        let keyboard = !embedded
            && !self.editing()
            && !self.grid.dragging()
            && !egui::Popup::is_any_open(ui.ctx())
            && ui.memory(|m| m.focused().is_none_or(|focus| focus == grid_id));
        let reveal = keyboard && self.grid_keys(ui, id, read_only);

        if !embedded {
            if let Some(copy) = self.recovered {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Local changes were saved in a recovery copy.");
                    if ui.button("Open copy").clicked() {
                        response.open_file = Some(copy);
                    }
                });
            }
            self.toolbar(ui, grid_id, height, read_only);
            self.formula_bar(ui, id, height, read_only, &mut response);
            if let Some(error) = &self.error {
                ui.colored_label(ui.ctx().get_lb_theme().danger(), error);
            }
            if let Some(error) = &self.save_error {
                let retry = ui.horizontal_wrapped(|ui| {
                    ui.colored_label(ui.ctx().get_lb_theme().danger(), error);
                    ui.button("Retry").clicked()
                });
                if retry.inner {
                    self.save_error = None;
                    self.seq += 1;
                }
            }
        }

        let grid_options = grid::Options { row_height: height, touch, embedded, read_only };
        self.show_grid(ui, id, grid_options, reveal);
        if !embedded {
            self.worksheets(ui, id, height, read_only);
        }
        if !self.editing() {
            response.text_interaction_rect = None;
        }
        if was_editing != self.editing() {
            response.text_updated = true;
            response.selection_updated = true;
        }
        response
    }

    fn worksheets(&mut self, ui: &mut Ui, id: Id, height: f32, read_only: bool) {
        let sheets = self.model.get_worksheets_properties();
        let mut next = self.sheet;
        let text_color = ui.ctx().get_lb_theme().neutral_fg();
        ui.horizontal(|ui| {
            if !read_only
                && toolbar_icon(ui, phosphor::FILE_PLUS, "Add worksheet", height, true, None)
                && self.commit(ui, id)
            {
                let result = self.model.new_sheet();
                if self.record_change(result) {
                    next = self.model.get_selected_sheet();
                }
            }
            egui::ScrollArea::horizontal()
                .id_salt("worksheets")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().interact_size.y = height;
                        for (index, sheet) in sheets.iter().enumerate() {
                            let label = RichText::new(&sheet.name).color(text_color);
                            if ui
                                .selectable_label(index as u32 == self.sheet, label)
                                .clicked()
                            {
                                next = index as u32;
                            }
                        }
                    });
                });
        });
        if next != self.sheet && self.commit(ui, id) {
            self.sheet = next;
            self.select(Cell::FIRST);
            self.grid = grid::State::default();
            ui.memory_mut(|m| m.request_focus(id.with("grid")));
            ui.ctx().request_repaint();
        }
    }

    fn toolbar(&mut self, ui: &mut Ui, grid_id: Id, height: f32, read_only: bool) {
        let enabled = !read_only && !self.editing();
        let compact = ui.available_width() < height * 9.0 + 145.0;
        let group_text_styles = ui.available_width() < height * 8.0 + 24.0;
        let style = self.cell_style(self.selected);
        let alignment = style
            .alignment
            .as_ref()
            .map_or(&HorizontalAlignment::General, |alignment| &alignment.horizontal);
        let seq = self.seq;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (redo, icon, label, available) in [
                (false, phosphor::ARROW_COUNTER_CLOCKWISE, "Undo", self.model.can_undo()),
                (true, phosphor::ARROW_CLOCKWISE, "Redo", self.model.can_redo()),
            ] {
                if toolbar_icon(ui, icon, label, height, enabled && available, None) {
                    self.change_history(redo);
                }
            }
            if !compact {
                ui.separator();
            }
            ui.add_enabled_ui(enabled, |ui| {
                self.number_format_menu(ui, &style.num_fmt, compact, height);
            });
            if !compact {
                ui.separator();
            }
            self.text_style_controls(ui, &style, height, enabled, group_text_styles);
            if compact {
                ui.add_enabled_ui(enabled, |ui| self.alignment_menu(ui, alignment, height));
            } else {
                ui.separator();
                for (value, label, icon) in ALIGNMENTS {
                    let active = &value == alignment;
                    if toolbar_icon(ui, icon, label, height, enabled, Some(active)) {
                        let value = if active { HorizontalAlignment::General } else { value };
                        self.update_style("alignment.horizontal", &value.to_string());
                    }
                }
            }
            ui.add_enabled_ui(!self.editing(), |ui| self.selection_menu(ui, height, read_only));
        });
        if self.seq != seq {
            ui.memory_mut(|memory| memory.request_focus(grid_id));
            ui.ctx().request_repaint();
        }
    }

    fn selection_menu(&mut self, ui: &mut Ui, height: f32, read_only: bool) {
        enum Action {
            Copy,
            Clear,
            Fill(ranges::Fill, CellRange),
        }

        let range = self.selection();
        let button =
            Button::new(icon_text(phosphor::DOTS_THREE, 18.0)).min_size(vec2(height, height));
        let (trigger, action) = egui::containers::menu::MenuButton::from_button(
            button.frame_when_inactive(false),
        )
        .ui(ui, |ui| {
            ui.spacing_mut().interact_size.y = height;
            let mut action = ui.button("Copy cells").clicked().then_some(Action::Copy);
            if !read_only {
                if ui
                    .add_enabled(range.last.row > range.first.row, Button::new("Fill down"))
                    .clicked()
                {
                    let mut source = range;
                    source.last.row = source.first.row;
                    action = Some(Action::Fill(ranges::Fill::Rows(range.last.row), source));
                }
                if ui
                    .add_enabled(range.last.column > range.first.column, Button::new("Fill right"))
                    .clicked()
                {
                    let mut source = range;
                    source.last.column = source.first.column;
                    action = Some(Action::Fill(ranges::Fill::Columns(range.last.column), source));
                }
                if ui.button("Clear contents").clicked() {
                    action = Some(Action::Clear);
                }
            }
            if action.is_some() {
                ui.close();
            }
            action
        });
        trigger.widget_info(|| {
            WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Cell actions")
        });
        trigger.on_hover_text("Cell actions");
        match action.and_then(|response| response.inner) {
            Some(Action::Copy) => self.copy_selection(ui),
            Some(Action::Clear) => {
                self.clear_selection();
            }
            Some(Action::Fill(fill, source)) => self.fill_selection(source, fill),
            None => {}
        }
    }

    fn text_style_controls(
        &mut self, ui: &mut Ui, style: &Style, height: f32, enabled: bool, grouped: bool,
    ) {
        let toggles = [
            (phosphor::TEXT_B, "Bold", "font.b", style.font.b),
            (phosphor::TEXT_ITALIC, "Italic", "font.i", style.font.i),
            (phosphor::TEXT_UNDERLINE, "Underline", "font.u", style.font.u),
        ];
        if grouped {
            ui.add_enabled_ui(enabled, |ui| {
                let choices =
                    toggles.map(|(_, label, property, active)| (label, active, (property, active)));
                let button = Button::new((
                    icon_text(phosphor::TEXT_AA, 18.0),
                    icon_text(phosphor::CARET_DOWN, 10.0),
                ));
                if let Some((property, active)) =
                    toolbar_menu(ui, button, "Text style", height, choices)
                {
                    self.update_style(property, if active { "false" } else { "true" });
                }
            });
        } else {
            for (icon, label, property, active) in toggles {
                if toolbar_icon(ui, icon, label, height, enabled, Some(active)) {
                    self.update_style(property, if active { "false" } else { "true" });
                }
            }
        }
    }

    fn number_format_menu(&mut self, ui: &mut Ui, current: &str, compact: bool, height: f32) {
        let formats = self.model.get_fmt_settings();
        let options = [
            ("General", "general"),
            ("Number", formats.number_fmt.as_str()),
            ("Percent", "0.00%"),
            ("Currency", formats.currency_format.as_str()),
            ("Date", formats.short_date.as_str()),
        ];
        let current_label = options
            .iter()
            .find(|(_, value)| value.eq_ignore_ascii_case(current))
            .map_or("Custom", |(label, _)| *label);
        let label = if compact { "123" } else { current_label };
        let button = Button::new((label, icon_text(phosphor::CARET_DOWN, 10.0)));
        let choices =
            options.map(|(label, value)| (label, value.eq_ignore_ascii_case(current), value));
        let name = format!("Number format ({current_label})");
        if let Some(value) = toolbar_menu(ui, button, &name, height, choices)
            .filter(|value| !value.eq_ignore_ascii_case(current))
        {
            self.update_style("num_fmt", value);
        }
    }

    fn alignment_menu(&mut self, ui: &mut Ui, current: &HorizontalAlignment, height: f32) {
        let icon = ALIGNMENTS
            .iter()
            .find(|(value, _, _)| value == current)
            .map_or(phosphor::TEXT_ALIGN_LEFT, |(_, _, icon)| *icon);
        let button = Button::new((icon_text(icon, 18.0), icon_text(phosphor::CARET_DOWN, 10.0)));
        let choices = std::iter::once((HorizontalAlignment::General, "Automatic", ""))
            .chain(ALIGNMENTS)
            .map(|(value, label, _)| (label, &value == current, value));
        if let Some(value) = toolbar_menu(ui, button, "Horizontal alignment", height, choices)
            .filter(|value| value != current)
        {
            self.update_style("alignment.horizontal", &value.to_string());
        }
    }

    fn formula_bar(
        &mut self, ui: &mut Ui, id: Id, height: f32, read_only: bool, response: &mut Response,
    ) {
        let mut apply = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            let address = self.selection().address();
            let width = (address.len() as f32 * 8.0 + 6.0).clamp(ROW_HEADER, 132.0);
            ui.add_sized(vec2(width, height), egui::Label::new(address).truncate());
            ui.label(icon_text(phosphor::FUNCTION, 16.0).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.editing() {
                    cancel = toolbar_icon(ui, phosphor::X, "Cancel (Esc)", height, true, None);
                    apply = toolbar_icon(ui, phosphor::CHECK, "Apply (Enter)", height, true, None);
                }
                let size = vec2(ui.available_width().max(1.0), height);
                let sense = if read_only { Sense::hover() } else { Sense::click() };
                let (rect, field_response) = ui.allocate_exact_size(size, sense);
                if !self.editing() {
                    field_response.widget_info(|| {
                        WidgetInfo::labeled(
                            egui::Role::TextInput,
                            !read_only,
                            "Cell value or formula",
                        )
                    });
                    if !read_only {
                        if field_response.clicked() {
                            self.begin_edit(ui, id, &self.input());
                        }
                        if field_response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                        }
                    }
                }
                let theme = ui.ctx().get_lb_theme();
                let outlined = self.editing() || field_response.has_focus();
                let border = if outlined { theme.accent() } else { theme.neutral() };
                ui.painter().rect(
                    rect,
                    4.0,
                    ui.visuals().extreme_bg_color,
                    Stroke::new(1.0, border),
                    StrokeKind::Inside,
                );
                let mut text_rect = rect.shrink2(vec2(8.0, 4.0));
                if let Some(field) = self.edit_mut() {
                    text_rect.min.y += (height - 28.0) / 2.0;
                    text_rect.max.y = rect.bottom();
                    response.text_interaction_rect = Some(text_rect);
                    if ui.memory(|m| m.has_focus(id)) {
                        if field.renderer.ime_marked.is_none() {
                            apply |= ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                        }
                        cancel |= ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
                    }
                    let applied = field.handle_input(ui.ctx(), id);
                    response.text_updated |= applied.text_updated;
                    response.selection_updated |= applied.selection_user_moved;
                    apply |= field.renderer.buffer.current.text.contains('\n');
                    field.show(ui, text_rect, id);
                } else {
                    let input = self.input();
                    let (text, color) = if input.is_empty() && !read_only {
                        ("Enter a value or formula", theme.neutral_fg_secondary())
                    } else {
                        (input.as_str(), theme.neutral_fg())
                    };
                    paint_text(ui, text_rect, text, Align2::LEFT_CENTER, color);
                }
            });
        });
        if cancel {
            self.end_edit(ui, id);
        } else if apply {
            self.commit(ui, id);
        }
        if !self.editing() {
            response.text_interaction_rect = None;
        }
    }

    fn grid_keys(&mut self, ui: &Ui, id: Id, read_only: bool) -> bool {
        let (delta, extend, edit, typing, clear, undo, redo, copy, paste) = ui.input_mut(|i| {
            let extend = i.modifiers.shift;
            let modifiers = if extend { Modifiers::SHIFT } else { Modifiers::NONE };
            let delta = [
                (Key::ArrowDown, (1, 0)),
                (Key::ArrowUp, (-1, 0)),
                (Key::ArrowRight, (0, 1)),
                (Key::ArrowLeft, (0, -1)),
            ]
            .into_iter()
            .find_map(|(key, delta)| i.consume_key(modifiers, key).then_some(delta));
            let edit = !read_only
                && (i.consume_key(Modifiers::NONE, Key::F2)
                    || i.consume_key(Modifiers::NONE, Key::Enter));
            let typing = !read_only
                && !i.modifiers.command
                && i.events.iter().any(|event| match event {
                    Event::Text(text) => text.chars().any(|c| !c.is_control()),
                    _ => false,
                });
            let clear = !read_only && i.consume_key(Modifiers::NONE, Key::Delete);
            let redo = !read_only && i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
            let undo = !read_only && !redo && i.consume_key(Modifiers::COMMAND, Key::Z);
            let mut copy = false;
            let mut paste = None;
            i.events.retain(|event| match event {
                Event::Copy => {
                    copy = true;
                    false
                }
                Event::Paste(text) if !read_only => {
                    paste = Some(text.clone());
                    false
                }
                _ => true,
            });
            (delta, extend, edit, typing, clear, undo, redo, copy, paste)
        });
        if clear {
            self.clear_selection();
        } else if undo || redo {
            self.change_history(redo);
        }
        if copy {
            self.copy_selection(ui);
        }
        if let Some(text) = paste {
            self.paste(&text);
        }
        if let Some((rows, columns)) = delta {
            if extend {
                self.range_end = self.range_end.offset(rows, columns).clamped();
            } else {
                self.select(self.selected.offset(rows, columns).clamped());
            }
        }
        if edit || (typing && !self.editing()) {
            let input = if typing { String::new() } else { self.input() };
            self.begin_edit(ui, id, &input);
        }
        delta.is_some()
    }
}

pub fn encode_workbook(workbook: &Workbook) -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    bytes.extend(bitcode::encode(workbook));
    bytes
}

fn read_workbook(hmac: Option<DocumentHmac>, bytes: &[u8]) -> Result<UserModel<'static>, String> {
    if bytes.is_empty() && hmac.is_none() {
        return UserModel::new_empty("Spreadsheet", "en", "UTC", "en");
    }
    let payload = bytes
        .strip_prefix(HEADER)
        .ok_or("Unsupported spreadsheet format")?;
    let workbook = UserModel::from_bytes(payload, "en")
        .map_err(|error| format!("Could not open spreadsheet: {error}"))?;
    if workbook.get_model().workbook.worksheets.is_empty() {
        return Err("Spreadsheet has no worksheets".into());
    }
    Ok(workbook)
}

fn column_label(mut column: i32) -> String {
    let mut letters = Vec::with_capacity(3);
    while column > 0 {
        column -= 1;
        letters.push(char::from(b'A' + (column % 26) as u8));
        column /= 26;
    }
    letters.into_iter().rev().collect()
}

fn icon_text(icon: &str, size: f32) -> RichText {
    RichText::new(icon).font(phosphor_font_id(size))
}

fn toolbar_icon(
    ui: &mut Ui, icon: &str, label: &str, height: f32, enabled: bool, active: Option<bool>,
) -> bool {
    let mut text = icon_text(icon, 18.0);
    if active == Some(true) {
        text = text.color(ui.ctx().get_lb_theme().neutral_fg());
    }
    let mut button = Button::new(text)
        .min_size(vec2(height, height))
        .frame_when_inactive(active == Some(true));
    if let Some(active) = active {
        button = button.selected(active);
    }
    let response = ui.add_enabled(enabled, button);
    response.widget_info(|| match active {
        Some(active) => WidgetInfo::selected(egui::Role::Button, enabled, active, label),
        None => WidgetInfo::labeled(egui::Role::Button, enabled, label),
    });
    response.on_hover_text(label).clicked()
}

fn toolbar_menu<'a, T>(
    ui: &mut Ui, button: Button<'_>, name: &str, height: f32,
    choices: impl IntoIterator<Item = (&'a str, bool, T)>,
) -> Option<T> {
    let button = button
        .min_size(vec2(height, height))
        .frame_when_inactive(false);
    let (trigger, chosen) = egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        ui.set_min_width(124.0);
        ui.spacing_mut().interact_size.y = height;
        choices.into_iter().find_map(|(label, selected, value)| {
            if ui.selectable_label(selected, label).clicked() {
                ui.close();
                Some(value)
            } else {
                None
            }
        })
    });
    trigger.widget_info(|| WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), name));
    trigger.on_hover_text(name);
    chosen.and_then(|response| response.inner)
}

fn paint_cell_value(ui: &Ui, rect: Rect, value: &str, style: &Style, number: bool, color: Color32) {
    let alignment = style
        .alignment
        .as_ref()
        .map(|alignment| &alignment.horizontal);
    let align = match alignment {
        Some(HorizontalAlignment::Center | HorizontalAlignment::CenterContinuous) => {
            Align2::CENTER_CENTER
        }
        Some(HorizontalAlignment::Left) => Align2::LEFT_CENTER,
        Some(HorizontalAlignment::Right) => Align2::RIGHT_CENTER,
        _ if number => Align2::RIGHT_CENTER,
        _ => Align2::LEFT_CENTER,
    };
    let font = &style.font;
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if !font.b && !font.i && !font.u {
        painter.text(align.pos_in_rect(&rect), align, value, FontId::proportional(14.0), color);
        return;
    }
    let family = if font.b {
        FontFamily::Name("Bold".into())
    } else if font.i {
        FontFamily::Name("Italic".into())
    } else {
        FontFamily::Proportional
    };
    let format = TextFormat {
        font_id: FontId::new(14.0, family),
        color,
        italics: font.b && font.i,
        underline: if font.u { Stroke::new(1.0, color) } else { Stroke::NONE },
        ..Default::default()
    };
    let galley = painter.layout_job(LayoutJob::single_section(value.to_owned(), format));
    let pos = align.align_size_within_rect(galley.size(), rect).min;
    painter.galley(pos, galley, color);
}

fn paint_text(ui: &Ui, rect: Rect, text: &str, align: Align2, color: Color32) {
    if !text.is_empty() {
        ui.painter()
            .with_clip_rect(rect.intersect(ui.clip_rect()))
            .text(align.pos_in_rect(&rect), align, text, FontId::proportional(14.0), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tab::{ContentState, TabContent};
    use crate::test_utils::ws::{TestWs, test_ctx};
    use egui::{Context, Event, PointerButton, RawInput};

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn click(pos: egui::Pos2) -> Vec<Event> {
        let button = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        vec![Event::PointerMoved(pos), button(true), button(false)]
    }

    fn show(
        ctx: &Context, sheet: &mut Spreadsheet, mut events: Vec<Event>, size: egui::Vec2,
        read_only: bool,
    ) -> (Response, egui::FullOutput) {
        let mut response = Response::default();
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                Event::Key { modifiers, .. } | Event::PointerButton { modifiers, .. } => {
                    Some(*modifiers)
                }
                _ => None,
            })
            .unwrap_or_default();
        events.insert(0, Event::ModifiersChanged(modifiers));
        let mut full = ctx.run_pass(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
                events,
                ..Default::default()
            },
            |ctx| {
                ctx.root_ui(|ui| {
                    egui::CentralPanel::default()
                        .show(ui, |ui| response = sheet.show(ui, read_only, false));
                })
            },
        );
        full.textures_delta.clear();
        (response, full)
    }

    fn control_position(full: egui::FullOutput, label: &str) -> egui::Pos2 {
        control_rect(full, label).center()
    }

    fn control_rect(full: egui::FullOutput, label: &str) -> Rect {
        let tree = full.platform_output.accesskit_update.expect("widget tree");
        let bounds = tree
            .nodes
            .into_iter()
            .find(|(_, node)| node.label() == Some(label))
            .unwrap_or_else(|| panic!("missing control {label}"))
            .1
            .bounds()
            .expect("control bounds");
        Rect::from_min_max(
            pos2(bounds.x0 as f32, bounds.y0 as f32),
            pos2(bounds.x1 as f32, bounds.y1 as f32),
        )
    }

    fn drag(
        ctx: &Context, sheet: &mut Spreadsheet, size: egui::Vec2, from: egui::Pos2, to: egui::Pos2,
    ) {
        let [_, press, _] = click(from).try_into().unwrap();
        let [_, _, release] = click(to).try_into().unwrap();
        show(ctx, sheet, vec![Event::PointerMoved(from)], size, false);
        show(ctx, sheet, vec![press], size, false);
        show(ctx, sheet, vec![Event::PointerMoved(to)], size, false);
        show(ctx, sheet, vec![release], size, false);
    }

    #[test]
    fn spreadsheet_range_drag_formats_and_clears_all_selected_cells() {
        let ctx = test_ctx();
        ctx.enable_accesskit();
        let size = vec2(800.0, 600.0);
        let mut sheet = Spreadsheet::new(Uuid::nil(), None, &[]).unwrap();
        sheet.model.set_user_input(0, 1, 1, "1").unwrap();
        sheet.model.set_user_input(0, 2, 2, "2").unwrap();
        let a1 = control_position(show(&ctx, &mut sheet, vec![], size, false).1, "A1 1");
        let b2 = control_position(show(&ctx, &mut sheet, vec![], size, false).1, "B2 2");
        drag(&ctx, &mut sheet, size, b2, a1);
        assert_eq!(sheet.selection().address(), "A1:B2");
        assert_eq!(sheet.selected.address(), "B2");

        let bold = control_position(show(&ctx, &mut sheet, vec![], size, false).1, "Bold");
        show(&ctx, &mut sheet, click(bold), size, false);
        assert!(sheet.cell_style(Cell::FIRST).font.b);
        assert!(sheet.cell_style(Cell { row: 2, column: 1 }).font.b);
        show(&ctx, &mut sheet, vec![key(Key::Delete)], size, false);
        assert!(sheet.model.get_cell_content(0, 1, 1).unwrap().is_empty());
        assert!(sheet.model.get_cell_content(0, 2, 2).unwrap().is_empty());
        assert_eq!(sheet.seq(), 2);
        sheet.change_history(false);
        assert_eq!(sheet.model.get_cell_content(0, 2, 2).unwrap(), "2");
    }

    #[test]
    fn spreadsheet_drag_move_and_fill_update_formula_references() {
        let ctx = test_ctx();
        ctx.enable_accesskit();
        let size = vec2(800.0, 600.0);
        let mut sheet = Spreadsheet::new(Uuid::nil(), None, &[]).unwrap();
        for (row, column, text) in [(1, 1, "5"), (1, 2, "=A1*2"), (3, 1, "=B1+1"), (2, 2, "7")] {
            sheet.model.set_user_input(0, row, column, text).unwrap();
        }
        let a1 = control_rect(show(&ctx, &mut sheet, vec![], size, false).1, "A1 5");
        let b1 = control_rect(show(&ctx, &mut sheet, vec![], size, false).1, "B1 10");
        drag(&ctx, &mut sheet, size, a1.center(), b1.center());
        drag(&ctx, &mut sheet, size, a1.center_top() + vec2(0.0, 2.0), b1.center());
        assert_eq!(sheet.selection().address(), "B1:C1");
        assert!(sheet.model.get_cell_content(0, 1, 1).unwrap().is_empty());
        assert_eq!(sheet.model.get_cell_content(0, 1, 3).unwrap(), "=B1*2");
        assert_eq!(sheet.model.get_cell_content(0, 3, 1).unwrap(), "=C1+1");

        let c1 = control_rect(show(&ctx, &mut sheet, vec![], size, false).1, "C1 10");
        show(&ctx, &mut sheet, click(c1.center()), size, false);
        drag(
            &ctx,
            &mut sheet,
            size,
            c1.right_bottom() - vec2(3.0, 3.0),
            c1.center() + vec2(0.0, 28.0),
        );
        assert_eq!(sheet.model.get_cell_content(0, 2, 3).unwrap(), "=B2*2");
        assert_eq!(sheet.model.get_formatted_cell_value(0, 2, 3).unwrap(), "14");
        sheet.change_history(false);
        sheet.change_history(false);
        assert_eq!(sheet.model.get_cell_content(0, 1, 1).unwrap(), "5");
        assert_eq!(sheet.model.get_cell_content(0, 3, 1).unwrap(), "=B1+1");
    }

    #[test]
    fn spreadsheet_resizes_headers_and_persists_dimensions() {
        let ctx = test_ctx();
        ctx.enable_accesskit();
        let size = vec2(800.0, 600.0);
        let mut sheet = Spreadsheet::new(Uuid::nil(), None, &[]).unwrap();
        let (_, full) = show(&ctx, &mut sheet, vec![], size, false);
        let column = control_position(full, "Resize column A");
        drag(&ctx, &mut sheet, size, column, column + vec2(44.0, 0.0));
        let row = control_position(show(&ctx, &mut sheet, vec![], size, false).1, "Resize row 1");
        drag(&ctx, &mut sheet, size, row, row + vec2(0.0, 28.0));
        assert_eq!(sheet.seq(), 2);
        let reopened =
            Spreadsheet::new(sheet.id, Some([1; 32]), &encode_workbook(&sheet.snapshot())).unwrap();
        assert!(
            (reopened.model.get_column_width(0, 1).unwrap() - 90.0 * 156.0 / 112.0).abs() < 0.001
        );
        assert_eq!(reopened.model.get_row_height(0, 1).unwrap(), 50.0);
        sheet.change_history(false);
        assert_eq!(sheet.model.get_row_height(0, 1).unwrap(), 25.0);
    }

    #[test]
    fn spreadsheet_shift_selection_and_tabular_clipboard_work_from_the_grid() {
        let ctx = test_ctx();
        let size = vec2(800.0, 600.0);
        let mut sheet = Spreadsheet::new(Uuid::nil(), None, &[]).unwrap();
        show(&ctx, &mut sheet, vec![Event::Paste("5\t=A1*2\n7\t=A2*2".into())], size, false);
        assert_eq!(sheet.selection().address(), "A1:B2");
        let (_, full) = show(&ctx, &mut sheet, vec![Event::Copy], size, false);
        assert!(
            full.platform_output
                .commands
                .contains(&egui::OutputCommand::CopyText("5\t10\n7\t14".into()))
        );
        show(&ctx, &mut sheet, vec![key(Key::ArrowRight)], size, false);
        let shift_right = Event::Key {
            modifiers: Modifiers::SHIFT,
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
        };
        show(&ctx, &mut sheet, vec![shift_right], size, false);
        assert_eq!(sheet.selection().address(), "B1:C1");
    }

    #[test]
    fn spreadsheet_touch_handle_extends_selection_without_stealing_pan() {
        let ctx = test_ctx();
        ctx.enable_accesskit();
        ctx.set_os(egui::os::OperatingSystem::IOS);
        let size = vec2(320.0, 450.0);
        let mut sheet = Spreadsheet::new(Uuid::nil(), None, &[]).unwrap();
        let touch_drag = |sheet: &mut Spreadsheet, from: egui::Pos2, to: egui::Pos2| {
            let touch = |phase, pos| Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(0),
                phase,
                pos,
                force: None,
            };
            let [_, press, _] = click(from).try_into().unwrap();
            let [_, _, release] = click(to).try_into().unwrap();
            show(&ctx, sheet, vec![touch(egui::TouchPhase::Start, from), press], size, false);
            show(
                &ctx,
                sheet,
                vec![touch(egui::TouchPhase::Move, to), Event::PointerMoved(to)],
                size,
                false,
            );
            show(
                &ctx,
                sheet,
                vec![touch(egui::TouchPhase::End, to), release, Event::PointerGone],
                size,
                false,
            );
        };

        let (_, full) = show(&ctx, &mut sheet, vec![], size, false);
        let handle = control_position(full, "Drag to extend selection");
        let (_, full) = show(&ctx, &mut sheet, vec![], size, false);
        let b2 = control_position(full, "B2 ");
        touch_drag(&mut sheet, handle, b2);
        assert_eq!(sheet.selection().address(), "A1:B2");
        let (_, full) = show(&ctx, &mut sheet, vec![], size, false);
        let b5 = control_position(full, "B5 ");
        touch_drag(&mut sheet, b5, b5 - vec2(0.0, 100.0));
        let (_, full) = show(&ctx, &mut sheet, vec![], size, false);
        assert!(control_position(full, "B5 ").y < b5.y - 20.0);
        assert_eq!(sheet.selection().address(), "A1:B2");
        assert_eq!(sheet.seq(), 0);
    }

    #[test]
    fn spreadsheet_adds_a_worksheet_and_can_undo_creation() {
        let ctx = test_ctx();
        ctx.enable_accesskit();
        let size = vec2(800.0, 600.0);
        let mut sheet = Spreadsheet::new(Uuid::nil(), None, &[]).unwrap();
        let add = control_position(show(&ctx, &mut sheet, vec![], size, false).1, "Add worksheet");
        show(&ctx, &mut sheet, click(add), size, false);
        assert_eq!(sheet.sheet, 1);
        assert_eq!(sheet.snapshot().worksheets.len(), 2);
        sheet.change_history(false);
        assert_eq!(sheet.sheet, 0);
        assert_eq!(sheet.snapshot().worksheets.len(), 1);
    }

    #[test]
    fn spreadsheet_formula_commits_and_round_trips() {
        let ctx = test_ctx();
        let mut sheet = Spreadsheet::new(Uuid::new_v4(), None, &[]).unwrap();
        let size = vec2(800.0, 600.0);
        show(&ctx, &mut sheet, vec![key(Key::F2)], size, false);
        show(&ctx, &mut sheet, vec![Event::Text("21".into()), key(Key::Enter)], size, false);
        show(&ctx, &mut sheet, vec![key(Key::ArrowRight)], size, false);
        show(&ctx, &mut sheet, vec![key(Key::F2)], size, false);
        show(&ctx, &mut sheet, vec![Event::Text("=A1*2".into()), key(Key::Enter)], size, false);
        assert_eq!(sheet.seq(), 2);
        let reopened =
            Spreadsheet::new(sheet.id, Some([1; 32]), &encode_workbook(&sheet.snapshot())).unwrap();
        assert_eq!(reopened.model.get_cell_content(0, 1, 2).unwrap(), "=A1*2");
        assert_eq!(reopened.model.get_formatted_cell_value(0, 1, 2).unwrap(), "42");
    }

    #[test]
    fn spreadsheet_touch_selects_and_edits_a_cell_with_a_narrow_keyboard_viewport() {
        let ctx = test_ctx();
        ctx.set_os(egui::os::OperatingSystem::Android);
        let mut sheet = Spreadsheet::new(Uuid::new_v4(), None, &[]).unwrap();
        let size = vec2(360.0, 360.0);
        show(&ctx, &mut sheet, vec![], size, false);
        show(&ctx, &mut sheet, click(pos2(242.0, 154.0)), size, false);
        assert_eq!(sheet.selected.address(), "B1");
        show(&ctx, &mut sheet, click(pos2(120.0, 82.0)), size, false);
        let (response, _) = show(&ctx, &mut sheet, vec![Event::Text("=6*7".into())], size, false);
        let rect = response.text_interaction_rect.expect("native input bounds");
        assert!(rect.height() >= 30.0 && rect.right() < size.x);
        show(&ctx, &mut sheet, click(pos2(288.0, 82.0)), size, false);
        assert_eq!(sheet.model.get_formatted_cell_value(0, 1, 2).unwrap(), "42");

        show(&ctx, &mut sheet, vec![], size, false);
        show(&ctx, &mut sheet, click(pos2(30.0, 30.0)), size, false);
        assert!(sheet.input().is_empty());
        show(&ctx, &mut sheet, vec![], size, false);
        show(&ctx, &mut sheet, click(pos2(76.0, 30.0)), size, false);
        assert_eq!(sheet.input(), "=6*7");

        show(&ctx, &mut sheet, click(pos2(120.0, 82.0)), size, true);
        assert!(!sheet.editing());
    }

    #[test]
    fn spreadsheet_number_format_menu_persists_the_selected_format() {
        let ctx = test_ctx();
        ctx.enable_accesskit();
        ctx.set_os(egui::os::OperatingSystem::Android);
        let mut sheet = Spreadsheet::new(Uuid::new_v4(), None, &[]).unwrap();
        let size = vec2(320.0, 420.0);
        sheet.model.set_user_input(0, 1, 1, "0.25").unwrap();
        let (_, full) = show(&ctx, &mut sheet, vec![], size, false);
        let format_menu = control_position(full, "Number format (General)");
        show(&ctx, &mut sheet, click(format_menu), size, false);
        let (_, full) = show(&ctx, &mut sheet, vec![], size, false);
        let percent = control_position(full, "Percent");
        show(&ctx, &mut sheet, click(percent), size, false);
        assert_eq!(sheet.seq(), 1);

        let reopened =
            Spreadsheet::new(sheet.id, Some([1; 32]), &encode_workbook(&sheet.snapshot())).unwrap();
        assert_eq!(reopened.model.get_formatted_cell_value(0, 1, 1).unwrap(), "25.00%");
    }

    #[test]
    fn spreadsheet_typing_replaces_the_active_cell_on_commit() {
        let ctx = test_ctx();
        let mut sheet = Spreadsheet::new(Uuid::new_v4(), None, &[]).unwrap();
        let size = vec2(800.0, 600.0);
        sheet.model.set_user_input(0, 1, 1, "old value").unwrap();
        show(&ctx, &mut sheet, vec![], size, false);
        show(&ctx, &mut sheet, vec![Event::Text("=6*7".into())], size, false);
        assert_eq!(sheet.input(), "old value");
        show(&ctx, &mut sheet, vec![key(Key::Enter)], size, false);
        assert_eq!(sheet.input(), "=6*7");
    }

    #[test]
    fn spreadsheet_rejects_invalid_files() {
        assert!(Spreadsheet::new(Uuid::nil(), Some([1; 32]), &[]).is_err());
        assert!(Spreadsheet::new(Uuid::nil(), None, b"LBSHEET\0\x02bad").is_err());
        assert!(Spreadsheet::new(Uuid::nil(), None, HEADER).is_err());
    }

    #[test]
    fn spreadsheet_grid_fills_the_workspace_tab() {
        if ::test_utils::without_server() {
            return;
        }
        let mut test = TestWs::new();
        test.ws.desktop_tab_policy = true;
        let root = test.ws.core.get_root().unwrap().id;
        let file = test
            .ws
            .create_file_at(
                root,
                "Grid.lbsheet",
                lb_rs::model::file_metadata::FileType::Document,
                true,
            )
            .unwrap();
        test.ws.open_file(file.id, true, false);
        test.settle();
        test.frame_with(click(pos2(350.0, 336.0)));

        let selected = |test: &TestWs| {
            let ContentState::Open(TabContent::Spreadsheet(sheet)) =
                &test.ws.current_tab().unwrap().content
            else {
                panic!("spreadsheet tab is not open");
            };
            sheet.selected
        };
        let initial = selected(&test);
        assert_eq!(initial.address(), "C9");

        test.frame_with(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(-COLUMN_WIDTH * 3.0, -28.0 * 5.0),
            modifiers: Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Trackpad,
        }]);
        for _ in 0..4 {
            test.frame();
        }
        test.frame_with(click(pos2(350.0, 336.0)));
        let scrolled = selected(&test);
        assert!(scrolled.row > initial.row && scrolled.column > initial.column);
    }

    #[test]
    fn spreadsheet_autosaves_and_recovers_a_concurrent_write() {
        if ::test_utils::without_server() {
            return;
        }
        let mut test = TestWs::new();
        test.ws.cfg.set_auto_save(true);
        let root = test.ws.core.get_root().unwrap().id;
        let file = test
            .ws
            .create_file_at(
                root,
                "Budget.lbsheet",
                lb_rs::model::file_metadata::FileType::Document,
                true,
            )
            .unwrap();
        test.ws.open_file(file.id, true, false);
        test.settle();
        test.frame_with(vec![key(Key::F2)]);
        test.frame_with(vec![Event::Text("=6*7".into()), key(Key::Enter)]);
        let start = std::time::Instant::now();
        while !test.ws.saves_settled() {
            assert!(start.elapsed() < std::time::Duration::from_secs(5));
            std::thread::sleep(std::time::Duration::from_millis(10));
            test.frame();
        }
        let saved = test
            .ws
            .core
            .read_document_with_hmac(file.id, false)
            .unwrap();
        let model = read_workbook(saved.0, &saved.1).unwrap();
        assert_eq!(model.get_formatted_cell_value(0, 1, 1).unwrap(), "42");

        test.ws.core.sync().unwrap();
        let other = test.other_device();
        test.ws.cfg.set_auto_save(false);
        test.frame_with(vec![key(Key::F2)]);
        test.frame_with(vec![Event::Text("+1".into()), key(Key::Enter)]);
        let mut incoming = Spreadsheet::new(file.id, saved.0, &saved.1).unwrap();
        incoming.model.set_user_input(0, 1, 1, "99").unwrap();
        let bytes = encode_workbook(&incoming.snapshot());
        other.write_document(file.id, &bytes).unwrap();
        other.sync().unwrap();
        test.ws.core.sync().unwrap();
        test.ws.save_all_tabs();
        test.idle(file.id);

        let current = test.ws.core.read_document(file.id, false).unwrap();
        assert_eq!(current, bytes);
        let copy = test
            .ws
            .core
            .list_metadatas()
            .unwrap()
            .into_iter()
            .find(|file| file.name.starts_with("Budget (Recovered "))
            .expect("recovery copy");
        let (hmac, bytes) = test
            .ws
            .core
            .read_document_with_hmac(copy.id, false)
            .unwrap();
        let model = read_workbook(hmac, &bytes).unwrap();
        assert_eq!(model.get_formatted_cell_value(0, 1, 1).unwrap(), "43");
    }
}
