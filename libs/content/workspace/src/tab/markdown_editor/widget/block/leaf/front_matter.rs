//! Front matter as a grid: a row per property, the key in one column and
//! the value in the other, with the fences, colons, and quotes hidden.
//! Rows the grid cannot edit in place show as source
//! ([`crate::front_matter`]).

use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Pos2, Ui};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _, RangeIterExt as _};

use crate::front_matter::{FrontMatter, RowKind, Value};
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout, WrapUnitLayout};
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
            color: self.ctx.get_lb_theme().neutral_fg_secondary(),
            ..self.text_format_document()
        }
    }

    fn text_format_front_matter_error(&self) -> Format {
        Format { color: self.ctx.get_lb_theme().danger(), ..self.text_format_syntax() }
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
                        // the colon's own offset stays the key's, so a caret
                        // at the key's end draws there, not in this column
                        let after_colon = (sep.start() + 1).min(sep.end());
                        match value {
                            Value::Text { text, quotes } => {
                                let open = quotes.map_or(text.start(), |(open, _)| open.end());
                                hidden(&mut v, (after_colon, open));
                                if text.is_empty() {
                                    hidden(&mut v, *text);
                                } else {
                                    v.push_source(*text, &self.buffer[*text], value_format.clone());
                                }
                                let close = quotes.map_or(text.end(), |(_, close)| close.start());
                                hidden(&mut v, (close, row.line.end()));
                            }
                            Value::Empty { at, .. } => {
                                hidden(&mut v, (after_colon, *at));
                                hidden(&mut v, (*at, row.line.end()));
                            }
                        }
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

    pub fn height_front_matter(&self, node: &'ast AstNode<'ast>) -> f32 {
        let Some(fm) = &self.bounds.front_matter else {
            return self.height_fenced_code_block(node, &Self::frontmatter_as_code_block());
        };
        let width = self.width(node);
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
        let (key_column, units) = self.front_matter_units(&fm, width);
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
            y += height + row_spacing;
        }
    }
}
