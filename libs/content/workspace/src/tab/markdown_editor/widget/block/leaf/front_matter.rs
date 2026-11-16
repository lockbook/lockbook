//! Front matter as a grid: a row per property, the key in one column and
//! the value in the other, with the fences, colons, and quotes hidden.
//! A list is a row of chips, `true`/`false` a checkbox, and rows the grid
//! cannot edit in place show as source ([`crate::front_matter`]).

use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Pos2, Rect, Ui, Vec2};
use lb_rs::model::text::offset_types::{
    Grapheme, IntoRangeExt as _, RangeExt as _, RangeIterExt as _,
};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::front_matter::{FrontMatter, RowKind, Value};
use crate::tab::markdown_editor::input::{Event, FrontMatterAction};
use crate::tab::markdown_editor::widget::utils::wrap_layout::{
    EmbedKind, EmbedSpec, FontFamily, Format, Layout, StyleInfo, WrapUnitLayout,
};
use crate::tab::markdown_editor::{MdRender, TouchTarget};
use crate::theme::palette_v2::ThemeExt as _;

/// The key column takes at most this share of the block's width.
const KEY_COLUMN_MAX: f32 = 0.4;

/// One row's wrap units: a key beside a value, or source lines stacked.
enum RowUnits {
    Columns { key: Option<WrapUnitLayout>, value: Option<WrapUnitLayout> },
    Stack(Vec<WrapUnitLayout>),
}

impl RowUnits {
    fn height(&self, row_spacing: f32) -> f32 {
        match self {
            RowUnits::Columns { key, value } => {
                let k = key.as_ref().map_or(0.0, |u| u.height);
                let v = value.as_ref().map_or(0.0, |u| u.height);
                k.max(v)
            }
            RowUnits::Stack(units) => {
                let sum: f32 = units.iter().map(|u| u.height).sum();
                sum + row_spacing * units.len().saturating_sub(1) as f32
            }
        }
    }
}

/// A collapsed value longer than this is cut, with an ellipsis.
const PILL_VALUE_GRAPHEMES: usize = 28;

/// Click-target id of a collapsed row's pill, keyed by its line's start.
fn pill_id_salt(line: (Grapheme, Grapheme)) -> egui::Id {
    egui::Id::unique(("md_fm_pill", line.start().0))
}

/// At most [`PILL_VALUE_GRAPHEMES`] of `s`, an ellipsis standing for the rest.
fn cut(s: &str) -> String {
    let mut g = s.graphemes(true);
    let head: String = g.by_ref().take(PILL_VALUE_GRAPHEMES).collect();
    if g.next().is_some() { format!("{head}…") } else { head }
}

impl<'ast> MdRender {
    pub(crate) fn frontmatter_as_code_block() -> NodeCodeBlock {
        NodeCodeBlock {
            fenced: true,
            fence_char: b'-',
            fence_length: 3,
            fence_offset: 0,
            info: "yaml".to_string(),
            literal: String::new(),
            closed: true,
        }
    }

    fn text_format_front_matter_key(&self) -> Format {
        Format {
            family: FontFamily::Mono,
            color: self.ctx.get_lb_theme().neutral_fg_secondary(),
            ..self.text_format_document()
        }
    }

    fn text_format_front_matter_error(&self) -> Format {
        Format { color: self.ctx.get_lb_theme().danger(), ..self.text_format_syntax() }
    }

    /// A list item's capsule: tags take the accent, other lists a plain
    /// surface.
    fn text_format_front_matter_chip(&self, tag: bool) -> Format {
        let t = self.ctx.get_lb_theme();
        if tag {
            Format {
                family: FontFamily::Mono,
                color: t.accent(),
                background: t.accent().gamma_multiply(0.14),
                ..self.text_format_document()
            }
        } else {
            Format { background: t.neutral_bg_secondary(), ..self.text_format_document() }
        }
    }

    /// A checkbox value in `rect`, its text row: a task item's checkbox,
    /// clickable over the row's half spacing as a task item's is.
    pub fn show_front_matter_checkbox(
        &mut self, ui: &mut Ui, scope: egui::Id, rect: Rect, checked: bool, check: Grapheme,
    ) {
        let space = Rect::from_center_size(rect.center(), Vec2::splat(self.layout.row_height));
        let clickable = space.expand(self.layout.row_spacing / 2.0);
        let sense = if self.readonly { egui::Sense::hover() } else { egui::Sense::click() };
        let id = scope.with(("md_fm_checkbox", check.0));
        self.show_checkbox(ui, id, [space, clickable, clickable], checked, check, sense);
    }

    /// The key column's width: the widest key, within its share.
    fn front_matter_key_column(&self, fm: &FrontMatter<Grapheme>, width: f32) -> f32 {
        let row_height = self.layout.row_height;
        let mut widest: f32 = 0.0;
        for row in &fm.rows {
            let key = match &row.kind {
                RowKind::Property { key, .. } | RowKind::Pending { key } => key,
                _ => continue,
            };
            if key.is_empty() {
                continue;
            }
            let unit = self.compute_section_layout_new(
                *key,
                width,
                row_height,
                self.text_format_front_matter_key(),
            );
            let right = unit
                .rows
                .iter()
                .flat_map(|r| r.fragments.iter())
                .map(|f| f.rect.right())
                .fold(0.0, f32::max);
            widest = widest.max(right);
        }
        widest.min(width * KEY_COLUMN_MAX)
    }

    /// The value column of a property row: its text, chips, or checkbox,
    /// with the syntax around them hidden.
    fn layout_front_matter_value(
        &self, v: &mut Layout, key: (Grapheme, Grapheme), sep: (Grapheme, Grapheme),
        value: &Value<Grapheme>, line_end: Grapheme,
    ) {
        let value_format = self.text_format_document();
        let hidden = |layout: &mut Layout, range: (Grapheme, Grapheme)| {
            layout.push_override(range, "", value_format.clone());
        };
        // the colon's own offset stays the key's, so a caret at the key's
        // end draws there, not in this column
        let after_colon = (sep.start() + 1).min(sep.end());
        match value {
            Value::Text { text, quotes, checked } => {
                let open = quotes.map_or(text.start(), |(open, _)| open.end());
                // a caret at either end sits beside the box; inside, the
                // word shows to be edited
                let inside = self
                    .reveal_selection
                    .is_some_and(|s| text.start() < s.start() && s.end() < text.end());
                let boxed = checked.filter(|_| !inside);
                if boxed.is_none() {
                    hidden(v, (after_colon, open));
                }
                match boxed {
                    // the box spans the hidden gap too, so a caret there
                    // draws at its edge as at a text value's start
                    Some(checked) => {
                        // a text row's band, split where body text sits
                        let row_height = self.layout.row_height;
                        let (baseline, _) = self.body_metrics(row_height);
                        v.push_embed(EmbedSpec {
                            advance: row_height,
                            ascent: row_height * baseline,
                            descent: row_height * (1.0 - baseline),
                            source_range: (after_colon, text.end()),
                            url: String::new(),
                            kind: EmbedKind::Checkbox { checked, check: text.start() },
                        });
                    }
                    _ if text.is_empty() => hidden(v, *text),
                    _ => v.push_source(*text, &self.buffer[*text], value_format.clone()),
                }
                let close = quotes.map_or(text.end(), |(_, close)| close.start());
                hidden(v, (close, line_end));
            }
            Value::Empty { at, .. } => {
                hidden(v, (after_colon, *at));
                hidden(v, (*at, line_end));
            }
            Value::List { items, open, .. } => {
                let chip = self.text_format_front_matter_chip(&self.buffer[key] == "tags");
                if items.is_empty() {
                    hidden(v, (after_colon, *open + 1));
                    hidden(v, (*open + 1, line_end));
                    return;
                }
                let mut prev = after_colon;
                for (j, item) in items.iter().enumerate() {
                    if j == 0 {
                        hidden(v, (prev, item.text.start()));
                    } else {
                        // the comma or dash between chips reads as a space
                        v.push_override((prev, item.text.start()), " ", value_format.clone());
                    }
                    v.style_open(StyleInfo {
                        format: chip.clone(),
                        source_range: item.text,
                        chip: true,
                        wrap: false,
                        caret_inside: true,
                    });
                    if item.text.is_empty() {
                        hidden(v, item.text);
                    } else {
                        v.push_source(item.text, &self.buffer[item.text], chip.clone());
                    }
                    v.style_close();
                    prev = item.text.end();
                }
                hidden(v, (prev, line_end));
            }
        }
    }

    /// The key column's width and each row's wrap units.
    fn front_matter_units(&self, fm: &FrontMatter<Grapheme>, width: f32) -> (f32, Vec<RowUnits>) {
        let row_height = self.layout.row_height;
        let key_column = self.front_matter_key_column(fm, width);
        let gap = self.layout.block_padding;
        let value_width = (width - key_column - gap).max(0.0);
        let last = fm.rows.len().saturating_sub(1);
        let key_format = self.text_format_front_matter_key();
        let value_format = self.text_format_document();
        let hidden = |layout: &mut Layout, range: (Grapheme, Grapheme)| {
            layout.push_override(range, "", value_format.clone());
        };

        let rows = fm
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let (first, is_last) = (i == 0, i == last);
                match &row.kind {
                    RowKind::Property { key, sep, value } => {
                        let mut k = Layout::new((row.line.start(), key.end()));
                        if first {
                            hidden(&mut k, fm.head);
                        }
                        if key.is_empty() {
                            hidden(&mut k, *key);
                        } else {
                            k.push_source(*key, &self.buffer[*key], key_format.clone());
                        }
                        let mut v = Layout::new((sep.start(), row.line.end()));
                        self.layout_front_matter_value(&mut v, *key, *sep, value, row.line.end());
                        if is_last {
                            hidden(&mut v, fm.tail);
                        }
                        RowUnits::Columns {
                            key: Some(self.compute_layout_from(k, key_column.max(1.0), row_height)),
                            value: Some(self.compute_layout_from(v, value_width, row_height)),
                        }
                    }
                    RowKind::Pending { key } => {
                        let mut k = Layout::new((row.line.start(), key.end()));
                        if first {
                            hidden(&mut k, fm.head);
                        }
                        k.push_source(*key, &self.buffer[*key], key_format.clone());
                        // trailing whitespace and the tail hide after the key,
                        // in its own unit, so the caret typing the key stays
                        let mut trail = Layout::new((key.end(), row.line.end()));
                        let mut any = false;
                        if key.end() < row.line.end() {
                            hidden(
                                &mut trail,
                                ((key.end() + 1).min(row.line.end()), row.line.end()),
                            );
                            any = true;
                        }
                        if is_last {
                            hidden(&mut trail, fm.tail);
                            any = true;
                        }
                        RowUnits::Columns {
                            key: Some(self.compute_layout_from(k, key_column.max(1.0), row_height)),
                            value: any
                                .then(|| self.compute_layout_from(trail, value_width, row_height)),
                        }
                    }
                    RowKind::Blank => {
                        let mut l = Layout::new(row.line);
                        if first {
                            hidden(&mut l, fm.head);
                        }
                        hidden(&mut l, row.line);
                        if is_last {
                            hidden(&mut l, fm.tail);
                        }
                        RowUnits::Columns {
                            key: None,
                            value: Some(self.compute_layout_from(l, width, row_height)),
                        }
                    }
                    RowKind::Source { error } => {
                        let format = if *error {
                            self.text_format_front_matter_error()
                        } else {
                            self.text_format_syntax()
                        };
                        let lines = self.range_lines(row.line);
                        let n = lines.iter().count();
                        let units = lines
                            .iter()
                            .enumerate()
                            .map(|(j, line_idx)| {
                                let line = self.bounds.source_lines[line_idx].trim(&row.line);
                                let mut l = Layout::new(line);
                                if first && j == 0 {
                                    hidden(&mut l, fm.head);
                                }
                                l.push_source(line, &self.buffer[line], format.clone());
                                if is_last && j + 1 == n {
                                    hidden(&mut l, fm.tail);
                                }
                                self.compute_layout_from(l, width, row_height)
                            })
                            .collect();
                        RowUnits::Stack(units)
                    }
                }
            })
            .collect();
        (key_column, rows)
    }

    /// The block as one wrapped row of pills, `key value` each, in source
    /// order; a pill stands for its whole line.
    fn front_matter_strip(&self, fm: &FrontMatter<Grapheme>, width: f32) -> WrapUnitLayout {
        let t = self.ctx.get_lb_theme();
        let plain = self.text_format_document();
        let key_fmt =
            Format { background: t.neutral_bg_secondary(), ..self.text_format_front_matter_key() };
        let value_fmt = Format { background: t.neutral_bg_secondary(), ..plain.clone() };
        let tag_fmt = Format { family: FontFamily::Mono, color: t.accent(), ..value_fmt.clone() };
        let source_fmt =
            Format { background: t.neutral_bg_secondary(), ..self.text_format_syntax() };
        let mut l = Layout::new(fm.node());
        l.push_override(fm.head, "", plain.clone());
        let mut any = false;
        for row in &fm.rows {
            let (key_end, key, value, value_fmt) = match &row.kind {
                RowKind::Property { key, value, .. } => {
                    let text = match value {
                        Value::Text { text, .. } => cut(&self.buffer[*text]),
                        Value::Empty { .. } => String::new(),
                        Value::List { items, .. } => cut(&items
                            .iter()
                            .map(|i| &self.buffer[i.text])
                            .collect::<Vec<_>>()
                            .join(" · ")),
                    };
                    let tag = &self.buffer[*key] == "tags";
                    (
                        key.end(),
                        self.buffer[*key].to_string(),
                        text,
                        if tag { &tag_fmt } else { &value_fmt },
                    )
                }
                RowKind::Pending { key } => {
                    (key.end(), self.buffer[*key].to_string(), String::new(), &value_fmt)
                }
                RowKind::Blank => continue,
                RowKind::Source { .. } => {
                    let first = self.bounds.source_lines[self.range_lines(row.line).start()];
                    (row.line.start(), String::new(), cut(self.buffer[first].trim()), &source_fmt)
                }
            };
            if any {
                l.push_override(row.line.start().into_range(), " ", plain.clone());
            }
            any = true;
            l.interaction_open(pill_id_salt(row.line), egui::Sense::click());
            l.style_open(StyleInfo {
                format: key_fmt.clone(),
                source_range: row.line,
                chip: true,
                wrap: false,
                caret_inside: false,
            });
            if !key.is_empty() {
                l.push_override((row.line.start(), key_end), &key, key_fmt.clone());
            }
            let value =
                if key.is_empty() || value.is_empty() { value } else { format!("  {value}") };
            if value.is_empty() {
                l.push_override((key_end, row.line.end()), "", plain.clone());
            } else {
                l.push_override((key_end, row.line.end()), &value, value_fmt.clone());
            }
            l.style_close();
            l.interaction_close();
        }
        l.push_override(fm.tail, "", plain);
        self.compute_layout_from(l, width, self.layout.row_height)
    }

    /// The chevron in the gutter that collapses or opens the block: on
    /// desktop while the pointer is over the block, always on touch and
    /// while collapsed.
    fn show_front_matter_toggle(&mut self, ui: &mut Ui, top_left: Pos2, size: Vec2) {
        if !self.interactive || self.painting_drag_float {
            return;
        }
        let (_, icon_size, space) = Self::fold_button_size_icon_size_space(
            top_left,
            self.layout.row_height,
            self.layout.indent,
        );
        let toggle = Event::FrontMatter(FrontMatterAction::ToggleCollapsed);
        self.push_gutter_target(space, &toggle);
        let collapsed = self.front_matter_collapsed;
        let pointer = ui.input(|i| i.pointer.latest_pos().unwrap_or_default());
        let block = Rect::from_min_size(top_left, size);
        let target = space.expand2(Vec2::new(6.0, 4.0));
        if !(self.touch_mode || collapsed || block.contains(pointer) || target.contains(pointer)) {
            return;
        }
        let id = ui.scope_id().with("md_fm_toggle");
        let tips = ["Show Properties", "Hide Properties"];
        self.show_gutter_chevron(ui, id, (icon_size, space), toggle, collapsed, false, tips);
    }

    pub fn height_front_matter(&self, node: &'ast AstNode<'ast>) -> f32 {
        let Some(fm) = &self.bounds.front_matter else {
            return self.height_fenced_code_block(node, &Self::frontmatter_as_code_block());
        };
        let width = self.width(node);
        if self.front_matter_collapsed {
            return self.front_matter_strip(fm, width).height;
        }
        self.front_matter_rows_height(&self.front_matter_units(fm, width).1)
    }

    fn front_matter_rows_height(&self, units: &[RowUnits]) -> f32 {
        let rows: f32 = units
            .iter()
            .map(|u| u.height(self.layout.row_spacing))
            .sum();
        rows + self.layout.block_spacing * units.len().saturating_sub(1) as f32
    }

    pub fn show_front_matter(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2) {
        let Some(fm) = self.bounds.front_matter.clone() else {
            self.show_fenced_code_block(ui, node, top_left, &Self::frontmatter_as_code_block());
            return;
        };
        let width = self.width(node);
        if self.front_matter_collapsed {
            let strip = self.front_matter_strip(&fm, width);
            self.show_wrap_layout(ui, top_left, &strip);
            self.show_front_matter_toggle(ui, top_left, Vec2::new(width, strip.height));
            return;
        }
        let (key_column, units) = self.front_matter_units(&fm, width);
        let height = self.front_matter_rows_height(&units);
        self.show_front_matter_toggle(ui, top_left, Vec2::new(width, height));
        let row_spacing = self.layout.row_spacing;
        let value_x = top_left.x + key_column + self.layout.block_padding;
        let mut y = top_left.y;
        for units in units {
            let height = units.height(row_spacing);
            match units {
                // a row with no key, a blank one, spans the block
                RowUnits::Columns { key: None, value: Some(value) } => {
                    self.show_wrap_layout(ui, Pos2::new(top_left.x, y), &value);
                }
                RowUnits::Columns { key, value } => {
                    if let Some(key) = key {
                        self.show_wrap_layout(ui, Pos2::new(top_left.x, y), &key);
                    }
                    if let Some(value) = value {
                        self.show_wrap_layout(ui, Pos2::new(value_x, y), &value);
                    }
                }
                RowUnits::Stack(units) => {
                    let mut line_y = y;
                    for unit in units {
                        self.show_wrap_layout(ui, Pos2::new(top_left.x, line_y), &unit);
                        line_y += unit.height + row_spacing;
                    }
                }
            }
            y += height + self.layout.block_spacing;
        }
    }

    /// A click on a collapsed block's pill opens it on that row. Must run
    /// after `interact_fragments`.
    pub fn handle_front_matter_interactions(&mut self, ui: &egui::Ui) {
        let Some(fm) = &self.bounds.front_matter else { return };
        let parent_base = ui.scope_id();
        // a pill opens the block with the caret on its row
        if self.front_matter_collapsed && self.interactive {
            let pills: Vec<((Grapheme, Grapheme), Grapheme)> =
                fm.rows.iter().map(|row| (row.line, row.exit())).collect();
            for (line, caret) in pills {
                let id = parent_base.with(pill_id_salt(line));
                let Some(response) = self.interaction_responses.get(&id).cloned() else { continue };
                let open = Event::Select { region: caret.into_range().into() };
                self.touch_consume_interaction(id, TouchTarget::Tap(open.clone()));
                crate::style::control_cursor(ui, response.rect);
                if response.clicked() {
                    self.render_events.push(open);
                }
            }
        }
    }
}
