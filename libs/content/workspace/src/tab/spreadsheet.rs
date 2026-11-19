use std::ops::RangeInclusive;

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

// Version 1 uses the workbook encoding of ironcalc_base 0.9.1.
const HEADER: &[u8] = b"LBSHEET\0\x01";
const ROWS: i32 = 1_048_576;
const COLUMNS: i32 = 16_384;
const COLUMN_WIDTH: f32 = 112.0;
const ROW_HEADER: f32 = 62.0;
const COLUMN_HEADER: f32 = 26.0;
const ALIGNMENTS: [(HorizontalAlignment, &str, &str); 3] = [
    (HorizontalAlignment::Left, "Align left", phosphor::TEXT_ALIGN_LEFT),
    (HorizontalAlignment::Center, "Align center", phosphor::TEXT_ALIGN_CENTER),
    (HorizontalAlignment::Right, "Align right", phosphor::TEXT_ALIGN_RIGHT),
];

#[derive(Clone, Copy, PartialEq, Eq)]
struct Cell {
    row: i32,
    column: i32,
}

impl Cell {
    const FIRST: Self = Self { row: 1, column: 1 };

    fn address(self) -> String {
        format!("{}{}", column_label(self.column), self.row)
    }

    fn rect(self, origin: egui::Pos2, height: f32) -> Rect {
        Rect::from_min_size(
            origin + vec2((self.column - 1) as f32 * COLUMN_WIDTH, (self.row - 1) as f32 * height),
            vec2(COLUMN_WIDTH, height),
        )
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
    sheet: u32,
    edit: Option<Box<MdEdit>>,
    error: Option<String>,
    seq: usize,
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
            sheet: 0,
            edit: None,
            error: None,
            seq: 0,
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
        let result = if redo && self.model.can_redo() {
            self.model.redo()
        } else if !redo && self.model.can_undo() {
            self.model.undo()
        } else {
            return;
        };
        self.record_change(result);
    }

    fn update_style(&mut self, property: &str, value: &str) {
        let Cell { row, column } = self.selected;
        let area = Area { sheet: self.sheet, row, column, width: 1, height: 1 };
        let result = self.model.update_range_style(&area, property, value);
        self.record_change(result);
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
            self.toolbar(ui, grid_id, height, !read_only && !self.editing());
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

        if let Some((cell, double_clicked)) = self.grid(ui, height, embedded, reveal) {
            if !self.editing() {
                ui.memory_mut(|m| m.request_focus(grid_id));
            }
            if cell != self.selected && self.commit(ui, id) {
                self.selected = cell;
                response.text_interaction_rect = None;
                ui.ctx().request_repaint();
            }
            if double_clicked && !touch && !read_only && !self.editing() {
                self.begin_edit(ui, id, &self.input());
            }
        }
        if !embedded {
            let sheets = self.model.get_worksheets_properties();
            let mut next = self.sheet;
            let text_color = ui.ctx().get_lb_theme().neutral_fg();
            ui.horizontal(|ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("worksheets")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (index, sheet) in sheets.iter().enumerate() {
                                let label = egui::RichText::new(&sheet.name).color(text_color);
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
                self.selected = Cell::FIRST;
                response.text_interaction_rect = None;
                ui.memory_mut(|m| m.request_focus(grid_id));
                ui.ctx().request_repaint();
            }
        }
        if was_editing != self.editing() {
            response.text_updated = true;
            response.selection_updated = true;
        }
        response
    }

    fn toolbar(&mut self, ui: &mut Ui, grid_id: Id, height: f32, enabled: bool) {
        let compact = ui.available_width() < height * 8.0 + 145.0;
        let group_text_styles = ui.available_width() < height * 7.0 + 24.0;
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
        });
        if self.seq != seq {
            ui.memory_mut(|memory| memory.request_focus(grid_id));
            ui.ctx().request_repaint();
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
            ui.add_sized(vec2(ROW_HEADER, height), egui::Label::new(self.selected.address()));
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
        let (delta, edit, typing, clear, undo, redo) = ui.input_mut(|i| {
            let delta = if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                (1, 0)
            } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                (-1, 0)
            } else if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
                (0, 1)
            } else if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                (0, -1)
            } else {
                (0, 0)
            };
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
            (delta, edit, typing, clear, undo, redo)
        });
        let Cell { row, column } = self.selected;
        if clear && !self.input().is_empty() {
            let result = self.model.set_user_input(self.sheet, row, column, "");
            self.record_change(result);
        } else if undo || redo {
            self.change_history(redo);
        }
        self.selected = Cell {
            row: (row + delta.0).clamp(1, ROWS),
            column: (column + delta.1).clamp(1, COLUMNS),
        };
        if edit || typing {
            let input = if typing { String::new() } else { self.input() };
            self.begin_edit(ui, id, &input);
        }
        delta != (0, 0)
    }

    fn grid(&self, ui: &mut Ui, height: f32, embedded: bool, reveal: bool) -> Option<(Cell, bool)> {
        let theme = ui.ctx().get_lb_theme();
        let max_height = if embedded {
            (height * 8.0).min(ui.available_height())
        } else {
            (ui.available_height() - 32.0).max(0.0)
        };
        egui::ScrollArea::both()
            .id_salt(("grid", self.sheet))
            .auto_shrink([false, false])
            .max_height(max_height)
            .scroll_source(if embedded {
                egui::scroll_area::ScrollSource::NONE
            } else {
                Default::default()
            })
            .show_viewport(ui, |ui, viewport| {
                let origin = ui.min_rect().min + vec2(ROW_HEADER, COLUMN_HEADER);
                if !embedded {
                    ui.set_min_size(vec2(
                        ROW_HEADER + COLUMN_WIDTH * COLUMNS as f32,
                        COLUMN_HEADER + height * ROWS as f32,
                    ));
                } else {
                    ui.set_min_height(max_height);
                }
                let viewport_rect = viewport.translate(ui.min_rect().min.to_vec2());
                let corner = viewport_rect.min;
                let body =
                    Rect::from_min_max(corner + vec2(ROW_HEADER, COLUMN_HEADER), viewport_rect.max);
                let clip = ui.clip_rect();
                ui.set_clip_rect(clip.intersect(body));
                let columns = visible_indices(
                    viewport.min.x,
                    viewport.max.x - ROW_HEADER,
                    COLUMN_WIDTH,
                    COLUMNS,
                );
                let rows =
                    visible_indices(viewport.min.y, viewport.max.y - COLUMN_HEADER, height, ROWS);
                let mut clicked = None;
                let border = Stroke::new(
                    1.0,
                    theme.wash_toward_neutral_fg(ui.visuals().extreme_bg_color, 0.16),
                );
                if reveal {
                    ui.scroll_to_rect(
                        self.selected
                            .rect(origin, height)
                            .expand2(vec2(ROW_HEADER, COLUMN_HEADER)),
                        None,
                    );
                }
                for row in rows.clone() {
                    for column in columns.clone() {
                        let cell = Cell { row, column };
                        let rect = cell.rect(origin, height);
                        let selected = cell == self.selected;
                        let value = self
                            .model
                            .get_formatted_cell_value(self.sheet, row, column)
                            .unwrap_or_default();
                        if !embedded {
                            let response = ui.interact(
                                rect,
                                ui.scope_id().with((row, column)),
                                Sense::click(),
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::Role::Button,
                                    true,
                                    selected,
                                    format!("{} {value}", cell.address()),
                                )
                            });
                            if response.clicked() || response.double_clicked() {
                                clicked = Some((cell, response.double_clicked()));
                            }
                        }
                        if selected {
                            let color =
                                theme.wash_toward_neutral_fg(ui.visuals().extreme_bg_color, 0.07);
                            ui.painter().rect_filled(rect, 0.0, color);
                            ui.painter().rect_stroke(
                                rect,
                                0.0,
                                Stroke::new(2.0, theme.accent()),
                                StrokeKind::Inside,
                            );
                        } else {
                            ui.painter()
                                .rect_stroke(rect, 0.0, border, StrokeKind::Middle);
                        }
                        if !value.is_empty() {
                            let number = self.model.get_cell_type(self.sheet, row, column)
                                == Ok(CellType::Number);
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
                ui.set_clip_rect(clip);
                let header_fill = ui.visuals().panel_fill;
                let header_text = theme.neutral_fg_secondary();
                let column_clip =
                    Rect::from_min_max(pos2(body.min.x, corner.y), pos2(body.max.x, body.min.y));
                let row_clip =
                    Rect::from_min_max(pos2(corner.x, body.min.y), pos2(body.min.x, body.max.y));
                for column in columns {
                    let left = Cell { row: 1, column }.rect(origin, height).left();
                    let rect = Rect::from_min_size(
                        pos2(left, corner.y),
                        vec2(COLUMN_WIDTH, COLUMN_HEADER),
                    );
                    header(
                        ui,
                        rect,
                        column_clip,
                        header_fill,
                        border,
                        &column_label(column),
                        header_text,
                    );
                }
                for row in rows {
                    let top = Cell { row, column: 1 }.rect(origin, height).top();
                    let rect = Rect::from_min_size(pos2(corner.x, top), vec2(ROW_HEADER, height));
                    header(ui, rect, row_clip, header_fill, border, &row.to_string(), header_text);
                }
                let corner_rect = Rect::from_min_size(corner, vec2(ROW_HEADER, COLUMN_HEADER));
                ui.painter()
                    .rect(corner_rect, 0.0, header_fill, border, StrokeKind::Inside);
                clicked
            })
            .inner
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

fn visible_indices(from: f32, to: f32, step: f32, last: i32) -> RangeInclusive<i32> {
    let first = (from / step).floor() as i32 + 1;
    let end = (to / step).ceil() as i32;
    first.max(1)..=end.min(last)
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

fn header(
    ui: &Ui, rect: Rect, clip: Rect, fill: Color32, border: Stroke, text: &str, color: Color32,
) {
    let rect_clip = rect.intersect(clip).intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(rect_clip);
    painter.rect(rect, 0.0, fill, border, StrokeKind::Middle);
    painter.text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(12.0), color);
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
        ctx: &Context, sheet: &mut Spreadsheet, events: Vec<Event>, size: egui::Vec2,
        read_only: bool,
    ) -> (Response, egui::FullOutput) {
        let mut response = Response::default();
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
        let tree = full.platform_output.accesskit_update.expect("widget tree");
        let bounds = tree
            .nodes
            .into_iter()
            .find(|(_, node)| node.label() == Some(label))
            .unwrap_or_else(|| panic!("missing control {label}"))
            .1
            .bounds()
            .expect("control bounds");
        pos2(((bounds.x0 + bounds.x1) / 2.0) as f32, ((bounds.y0 + bounds.y1) / 2.0) as f32)
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
