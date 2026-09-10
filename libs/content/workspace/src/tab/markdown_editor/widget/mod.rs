use comrak::nodes::{AstNode, NodeHeading, NodeValue};
use egui::{Pos2, Rect, Sense, Ui, UiBuilder, Vec2, pos2, vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _, RangeIterExt as _};

use crate::style::chrome::{control_line_height, row_wash_inset};
use crate::style::typography::TypeRole;
use crate::style::{FG_HOVER, Radius, Space, ThemeExt as _, control_height};
use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format};

use super::MdRender;
use super::bounds::RangesExt as _;

pub(crate) mod block;
pub(crate) mod debug;
pub(crate) mod emoji_completions;
pub(crate) mod find;
pub(crate) mod inline;
pub(crate) mod link_completions;
pub(crate) mod outline;
pub(crate) mod toolbar;
pub(crate) mod utils;

impl<'ast> MdRender {
    /// Returns the range for the node.
    pub fn node_range(&self, node: &'ast AstNode<'ast>) -> (Grapheme, Grapheme) {
        // Check cache first
        if let Some(cached_range) = self.get_cached_node_range(node) {
            return cached_range;
        }

        let node_data = node.data.borrow();
        let mut range = self.sourcepos_to_range(node_data.sourcepos);

        match &node_data.value {
            // hack: comrak's sourcepos's are unstable (and indeed broken) for some
            // nested block situations. clamping paragraph ranges to their parent's
            // prevents the worst of the adverse consequences (e.g. double-rendering
            // source text).
            //
            // see: https://github.com/kivikakk/comrak/issues/567
            NodeValue::Paragraph => {
                let parent = node.parent().unwrap();
                let parent_range = self.node_range(parent);
                range.0 = range.0.max(parent_range.0);
                range.1 = range.1.min(parent_range.1);
            }

            // hack: "A line break (not in a code span or HTML tag) that is preceded
            // by two or more spaces and does not occur at the end of a block is
            // parsed as a hard line break" but we prefer to show the spaces since
            // we render soft breaks as hard breaks (which is up to our discretion).
            // https://github.github.com/gfm/#hard-line-breaks
            NodeValue::LineBreak => {
                range.0 = range.1 - 1; // include only the newline
            }

            // hack: GFM spec says "Blank lines preceding or following an indented
            // code block are not included in it" and I have observed the behavior
            // for following lines to be incorrect in e.g. "    f\n".
            NodeValue::CodeBlock(node_code_block) if !node_code_block.fenced => {
                for line_idx in self.range_lines(range).iter() {
                    let line = self.bounds.source_lines[line_idx];
                    let node_line = self.node_line(node, line);
                    if self.buffer[node_line].chars().any(|c| !c.is_whitespace()) {
                        range.1 = line.end();
                    }
                }
            }

            // hack: thematic breaks are emitted to contain all subsequent lines if
            // they are the last block in the document; we trim them to their first
            // line.
            NodeValue::ThematicBreak => {
                if let Some(line_idx) = self.range_lines(range).iter().next() {
                    let line = self.bounds.source_lines[line_idx];
                    range = range.trim(&line);
                }
            }

            // hack: list items are emitted to contain all lines until the next
            // block which would cause the cursor to be shown indented; we trim
            // trailing blank lines.
            NodeValue::Item(_) | NodeValue::TaskItem(_) => {
                let node_lines = self.range_lines(range);
                let mut last_nonempty_line_idx = node_lines.start();
                for line_idx in node_lines.iter() {
                    let line = self.bounds.source_lines[line_idx];
                    let node_line = self.node_line(node, line);
                    if !node_line.is_empty() {
                        last_nonempty_line_idx = line_idx;
                    }
                }

                let last_nonempty_line = self.bounds.source_lines[last_nonempty_line_idx];
                range.1 = last_nonempty_line.end();
            }

            NodeValue::List(_) => {
                let last_child = node.children().last().unwrap();
                range.1 = self.node_range(last_child).1;
            }

            // hack: comrak misreports the end column of an HtmlBlock that
            // sits inside a `>\t`-prefixed blockquote — it overshoots by
            // the tab's expanded-vs-source width (e.g. for `>\t<div>foo
            // </div>` it reports end col 18 instead of 16). The over-
            // wide end leaks the range onto the next line and causes the
            // following block's first line to be re-rendered as part of
            // the HtmlBlock. Confirmed via repro against comrak 0.50 that
            // this misreport is unique to this combination — every other
            // tab-blockquote shape (paragraph, code span, list-item
            // child) reports source-aligned columns. Clamp to the end of
            // the sourcepos's last line.
            NodeValue::HtmlBlock(_) => {
                let last_line_idx = node_data.sourcepos.end.line.saturating_sub(1);
                if let Some(line) = self.bounds.source_lines.get(last_line_idx) {
                    range.1 = range.1.min(line.end());
                }
            }

            // hack: for a Table inside a list Item, comrak reports
            // sourcepos columns for every row as if that row has the
            // same prefix as the Item's first line (e.g. `- `, 2
            // chars). A continuation line with different leading
            // whitespace (`\t` = 1 char, `    ` = 4 chars) gets the
            // same reported cols, so TableRow/TableCell/Text nodes
            // point at the wrong source bytes. Shift by
            // `(actual leading ws) - (item.padding)` per line to align
            // with actual source. Matches comrak issue #591 (closed
            // for indented-predecessor case, still present for
            // list-item continuation).
            // Any node descended from a Table whose rows hit comrak's
            // canonical-first-line quirk needs the shift, not just
            // Text/TableRow/TableCell — inline wrappers like Emph,
            // Strong, Code, Link have their own sourcepos that
            // otherwise points at pre-shift bytes. The helper's Table
            // ancestor check gates this; inline-vs-block distinction
            // doesn't matter.
            NodeValue::TableRow(_)
            | NodeValue::TableCell
            | NodeValue::Text(_)
            | NodeValue::Emph
            | NodeValue::Strong
            | NodeValue::Strikethrough
            | NodeValue::Code(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::Link(_)
            | NodeValue::Image(_)
            | NodeValue::Highlight
            | NodeValue::Underline
            | NodeValue::Superscript
            | NodeValue::Subscript
            | NodeValue::SpoileredText
            | NodeValue::Math(_)
            | NodeValue::ShortCode(_)
            | NodeValue::WikiLink(_)
            | NodeValue::FootnoteReference(_)
            | NodeValue::SoftBreak
            | NodeValue::Escaped
            | NodeValue::EscapedTag(_)
            | NodeValue::Subtext => {
                if let Some(shift) = self.table_in_item_continuation_shift(node) {
                    if shift != 0 {
                        // Redo the sourcepos→byte conversion, applying
                        // the shift in BYTE space *before* ceiling to
                        // grapheme boundaries. Taking the ceiled
                        // grapheme back to bytes and shifting there
                        // compounds the ceil with the shift, yielding
                        // a grapheme one or two clusters past where
                        // the actual content ends (observed with
                        // Devanagari in tab-indented table cells).
                        let buf_byte_end = self.buffer.current.text.len();
                        let sp = node_data.sourcepos;
                        let line_to_byte =
                            |lc: comrak::nodes::LineColumn,
                             plus_one_for_exclusive: bool|
                             -> lb_rs::model::text::offset_types::Byte {
                                let line_idx = lc.line.saturating_sub(1);
                                let col =
                                    if plus_one_for_exclusive { lc.column + 1 } else { lc.column };
                                let col_idx = col.saturating_sub(1);
                                let line = self.bounds.source_lines[line_idx];
                                let line_start = self.offset_to_byte(line.start()).0 as isize;
                                let shifted = line_start
                                    .saturating_add(col_idx as isize)
                                    .saturating_add(shift)
                                    .max(0) as usize;
                                lb_rs::model::text::offset_types::Byte(shifted.min(buf_byte_end))
                            };
                        let start_byte = line_to_byte(sp.start, false);
                        let end_byte = line_to_byte(sp.end, true);
                        range = (
                            self.buffer.current.segs.byte_to_char_ceil(start_byte),
                            self.buffer.current.segs.byte_to_char_ceil(end_byte),
                        );
                    }
                }
            }

            _ => {}
        }

        // Cache the result before returning
        self.set_cached_node_range(node, range);
        range
    }

    /// Byte shift to apply to a `TableRow`/`TableCell`/`Text` node
    /// when comrak's sourcepos columns don't match source — applies
    /// only when a `Table`'s first row sits on a list `Item`'s first
    /// line (e.g. `- | a | b |`, table immediately after the marker).
    /// In that shape comrak reports subsequent rows' cols as if they
    /// share the item-marker prefix, even when continuation lines use
    /// a different indent (tab, 4sp, etc.). Shift by
    /// `actual_leading_ws - item.padding` to align with source. For
    /// tables that *aren't* the item's first content (starting on a
    /// continuation line), comrak already reports source-aligned cols
    /// — skip the shift. Also restricted to top-level items (nested
    /// cases would need ancestor-aware leading-ws accounting).
    fn table_in_item_continuation_shift(&self, node: &'ast AstNode<'ast>) -> Option<isize> {
        let mut cur = Some(node);
        let table = loop {
            let n = cur?;
            if matches!(&n.data.borrow().value, NodeValue::Table(_)) {
                break n;
            }
            cur = n.parent();
        };
        let item = table.parent()?;
        let padding = match &item.data.borrow().value {
            NodeValue::Item(nl) => nl.padding,
            NodeValue::TaskItem(_) => match item.parent().map(|p| p.data.borrow().value.clone()) {
                Some(NodeValue::List(nl)) => nl.padding,
                _ => return None,
            },
            _ => return None,
        };
        let list = item.parent()?;
        let list_parent = list.parent()?;
        if !matches!(&list_parent.data.borrow().value, NodeValue::Document) {
            return None;
        }
        // Only when the Table's first row is on the Item's first line
        // (comrak's quirk only fires in that shape).
        let item_first_line = item.data.borrow().sourcepos.start.line;
        if table.data.borrow().sourcepos.start.line != item_first_line {
            return None;
        }
        let node_line = node.data.borrow().sourcepos.start.line;
        if node_line == item_first_line {
            return Some(0);
        }
        let line_idx = node_line.saturating_sub(1);
        let line = self.bounds.source_lines.get(line_idx)?;
        let text = &self.buffer[*line];
        let leading_ws = text.chars().take_while(|c| matches!(c, ' ' | '\t')).count();
        Some(leading_ws as isize - padding as isize)
    }

    /// Creates a UI that assigns ids using the node range.
    // By default, egui ids are assigned to ui's and widgets based on the parent
    // ui's id and incremented with each addition to a given parent. Because
    // editor text may be clickable, text allocates ids and affects future ids.
    // When the editor reveal state changes, more or fewer interactable text
    // units may be shown, and all assigned ids may change. When an iOS user
    // taps the editor, iOS first sends a selection event in a standalone frame
    // which affects the reveal state, then by the time the tap is released, the
    // widget being tapped may have had its id changed and will not register as
    // clicked. This function creates a consistently idenified ui based on the
    // node range to prevent ids from changing mid tap and therefore prevents
    // taps from failing. Note that this range does not and need not survive
    // edits to the document itself.
    pub fn node_ui(&mut self, ui: &mut Ui, node: &'ast AstNode<'ast>) -> Ui {
        ui.new_child(
            UiBuilder::new()
                .id_salt(self.node_range(node)) // <- the magic
                .layer_id(ui.layer_id())
                .max_rect(ui.max_rect()),
        )
    }

    /// Returns the lines spanned by the given range.
    pub fn range_lines(&self, range: (Grapheme, Grapheme)) -> (usize, usize) {
        // The first and last "split" positions are just `range.0` and
        // `range.1` — splitting on newlines doesn't change either
        // endpoint. Look those up directly in `source_lines` instead
        // of materializing a Vec of every intermediate line.
        let start_line_idx = self
            .bounds
            .source_lines
            .find_containing(range.0, true, true)
            .start();
        let end_line_idx = self
            .bounds
            .source_lines
            .find_containing(range.1, true, true)
            .end(); // (inclusive, exclusive) preserved
        (start_line_idx, end_line_idx)
    }

    pub fn text_format(&self, node: &AstNode<'_>) -> Format {
        let parent = || node.parent().unwrap();
        let parent_text_format = || self.text_format(parent());

        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) => parent_text_format(),
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(node_alert) => self.text_format_alert(parent(), node_alert),
            NodeValue::BlockQuote => self.text_format_block_quote(parent()),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.text_format_document(),
            NodeValue::FootnoteDefinition(_) => self.text_format_footnote_definition(parent()),
            NodeValue::Item(_) => parent_text_format(),
            NodeValue::List(_) => parent_text_format(),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => parent_text_format(),
            NodeValue::TableRow(is_header_row) => {
                self.text_format_table_row(parent(), *is_header_row)
            }

            // inline
            NodeValue::Code(_) => self.text_format_code(parent()),
            NodeValue::Emph => self.text_format_emph(parent()),
            NodeValue::Escaped => self.text_format_escaped(parent()),
            NodeValue::EscapedTag(_) => self.text_format_escaped_tag(parent()),
            NodeValue::FootnoteReference(_) => self.text_format_footnote_reference(parent()),
            NodeValue::Highlight => self.text_format_highlight(parent()),
            NodeValue::HtmlInline(_) => self.text_format_html_inline(parent()),
            NodeValue::Image(ni) => {
                self.text_format_link(parent(), self.link_state_for_url(&ni.url))
            }
            NodeValue::LineBreak => parent_text_format(),
            NodeValue::Link(nl) => {
                self.text_format_link(parent(), self.link_state_for_url(&nl.url))
            }
            NodeValue::Math(_) => self.text_format_math(parent()),
            NodeValue::ShortCode(_) => self.text_format_short_code(parent()),
            NodeValue::SoftBreak => parent_text_format(),
            NodeValue::SpoileredText => self.text_format_spoilered_text(node, parent()),
            NodeValue::Strikethrough => self.text_format_strikethrough(parent()),
            NodeValue::Strong => self.text_format_strong(parent()),
            NodeValue::Subscript => self.text_format_subscript(parent()),
            NodeValue::Subtext => unimplemented!("extension disabled"),
            NodeValue::Superscript => self.text_format_superscript(parent()),
            NodeValue::Text(_) => parent_text_format(),
            NodeValue::Underline => self.text_format_underline(parent()),
            NodeValue::WikiLink(nwl) => {
                self.text_format_link(parent(), self.link_state_for_wikilink(&nwl.url))
            }

            // leaf_block
            NodeValue::CodeBlock(_) => self.text_format_code_block(parent()),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(_) => parent_text_format(),
            NodeValue::HtmlBlock(_) => self.text_format_html_block(parent()),
            NodeValue::Paragraph => parent_text_format(),
            NodeValue::TableCell => parent_text_format(),
            NodeValue::TaskItem(_) => parent_text_format(),
            NodeValue::ThematicBreak => parent_text_format(),
        }
    }

    pub fn syntax_color_for_hex(&self, hex: &str) -> egui::Color32 {
        let theme = self.ctx.get_lb_theme();
        if let Some(code) = theme.code_variant() {
            return match hex {
                "#000000" => code.text,
                "#111111" => code.comment,
                "#222222" => code.class,
                "#333333" => code.function,
                "#444444" => code.keyword,
                "#555555" => code.string,
                "#666666" => code.number,
                _ => code.text,
            };
        }
        match hex {
            "#000000" => theme.neutral_fg(),
            "#111111" => theme.neutral_fg_secondary(),
            "#222222" => theme.fg().get_color(theme.prefs().primary),
            "#333333" => theme.fg().get_color(theme.prefs().secondary),
            "#444444" => theme.fg().get_color(theme.prefs().tertiary),
            "#555555" | "#666666" => theme.fg().get_color(theme.prefs().quaternary),
            _ => theme.neutral_fg(),
        }
    }

    pub fn text_format_syntax(&self) -> Format {
        Format {
            family: FontFamily::Mono,
            bold: false,
            italic: false,
            color: self.ctx.get_lb_theme().neutral_fg_secondary(),
            underline: false,
            strikethrough: false,
            background: egui::Color32::TRANSPARENT,
            border: egui::Color32::TRANSPARENT,
            spoiler: false,
            superscript: false,
            subscript: false,
        }
    }

    pub fn row_height(&self, node: &AstNode<'_>) -> f32 {
        match &node.data.borrow().value {
            NodeValue::Heading(NodeHeading { level, .. }) => self.heading_row_height(*level),
            _ => self.layout.row_height,
        }
    }

    pub fn compute_bounds(&mut self, node: &'ast AstNode<'ast>) {
        let value = &node.data.borrow().value;
        match value {
            NodeValue::FrontMatter(_) => {}
            NodeValue::Raw(_) => unreachable!("can only be created programmatically"),

            // container_block
            NodeValue::Alert(node_alert) => self.compute_bounds_alert(node, node_alert),
            NodeValue::BlockQuote => self.compute_bounds_block_quote(node),
            NodeValue::DescriptionItem(_) => unimplemented!("extension disabled"),
            NodeValue::DescriptionList => unimplemented!("extension disabled"),
            NodeValue::Document => self.compute_bounds_document(node),
            NodeValue::FootnoteDefinition(_) => self.compute_bounds_footnote_definition(node),
            NodeValue::Item(_) => self.compute_bounds_item(node),
            NodeValue::List(_) => self.compute_bounds_block_children(node),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => self.compute_bounds_block_children(node),
            NodeValue::TableRow(_) => self.compute_bounds_block_children(node),
            NodeValue::TaskItem(_) => self.compute_bounds_task_item(node),

            // inline
            NodeValue::Code(_) => {}
            NodeValue::Emph => {}
            NodeValue::Escaped => {}
            NodeValue::EscapedTag(_) => {}
            NodeValue::FootnoteReference(_) => {}
            NodeValue::Highlight => {}
            NodeValue::HtmlInline(_) => {}
            NodeValue::Image(_) => {}
            NodeValue::LineBreak => {}
            NodeValue::Link(_) => {}
            NodeValue::Math(_) => {}
            NodeValue::ShortCode(_) => {}
            NodeValue::SoftBreak => {}
            NodeValue::SpoileredText => {}
            NodeValue::Strikethrough => {}
            NodeValue::Strong => {}
            NodeValue::Subscript => {}
            NodeValue::Subtext => {}
            NodeValue::Superscript => {}
            NodeValue::Text(_) => {}
            NodeValue::Underline => {}
            NodeValue::WikiLink(_) => {}

            // leaf_block
            NodeValue::CodeBlock(_) => {}
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, setext, .. }) => {
                self.compute_bounds_heading(node, *level, *setext)
            }
            NodeValue::HtmlBlock(_) => {}
            NodeValue::Paragraph => self.compute_bounds_paragraph(node),
            NodeValue::TableCell => self.compute_bounds_table_cell(node),
            NodeValue::ThematicBreak => {}
        }
    }

    /// Overlay plate + row washes for a completion popup. Text is glyphon,
    /// painted by each completion type after this.
    pub fn draw_completion_popup(
        &self, ui: &mut Ui, popup_rect: Rect, row_rects: &[Rect], selected: usize,
        hover_pos: Option<egui::Pos2>,
    ) {
        let t = ui.ctx().get_lb_theme();
        let mut child = ui.new_child(UiBuilder::new().max_rect(popup_rect));
        crate::style::chrome::canvas_overlay_frame(&t, Space::Xxs)
            .inner_margin(0.0)
            .show(&mut child, |ui| {
                ui.allocate_exact_size(popup_rect.size(), Sense::hover());
            });

        for (idx, rect) in row_rects.iter().enumerate() {
            let over = idx == selected || hover_pos.is_some_and(|p| rect.contains(p));
            if !over {
                continue;
            }
            let wash = rect.shrink(row_wash_inset());
            ui.painter().rect_filled(
                wash,
                Radius::Sm.corner(),
                t.wash_toward_neutral_fg(t.neutral_bg(), FG_HOVER),
            );
        }
    }
}

pub(crate) fn completion_row_h() -> f32 {
    control_height()
}

pub(crate) fn completion_font() -> f32 {
    TypeRole::Body.size()
}

pub(crate) fn completion_line_h() -> f32 {
    control_line_height()
}

pub(crate) fn completion_path_font() -> f32 {
    TypeRole::Mono.size()
}

fn completion_plate_pad() -> f32 {
    Space::Xs.pts()
}

fn completion_row_pad_x() -> f32 {
    Space::Sm.pts()
}

/// Plate pad + row inset on both sides (width beyond the glyphon content).
pub(crate) fn completion_chrome_w() -> f32 {
    2.0 * (completion_plate_pad() + completion_row_pad_x())
}

pub(crate) fn completion_popup_size(content_w: f32, n: usize) -> Vec2 {
    vec2(
        content_w + completion_chrome_w(),
        n as f32 * completion_row_h() + 2.0 * completion_plate_pad(),
    )
}

pub(crate) fn completion_row_rects(popup: Rect, n: usize) -> Vec<Rect> {
    let pad = completion_plate_pad();
    let h = completion_row_h();
    let w = (popup.width() - 2.0 * pad).max(1.0);
    let x = popup.left() + pad;
    let y0 = popup.top() + pad;
    (0..n)
        .map(|i| Rect::from_min_size(pos2(x, y0 + i as f32 * h), vec2(w, h)))
        .collect()
}

pub(crate) fn completion_text_rect(row: Rect) -> Rect {
    let pad = completion_row_pad_x();
    let lh = completion_line_h();
    Rect::from_min_size(
        pos2(row.left() + pad, row.center().y - lh / 2.0),
        vec2((row.width() - 2.0 * pad).max(1.0), lh),
    )
}

/// Gap kept between the completion popup and every window edge. Matches
/// the overlay scrollbar footprint (`BAR_WIDTH` 6 + inset 2) plus a little
/// air so the popup clears the bar.
const COMPLETION_POPUP_MARGIN: f32 = 12.0;

/// Positions a completion popup near the text cursor while keeping it
/// inside the visible window with a [`COMPLETION_POPUP_MARGIN`] gap from
/// every edge. Prefers placing the popup above the cursor, falls back to
/// below, and clamps both axes (and the width) so it never draws
/// off-screen or up against an edge.
pub(crate) fn completion_popup_rect(
    cursor_top: Pos2, cursor_bot: Pos2, size: Vec2, screen_rect: Rect,
) -> Rect {
    let area = screen_rect.shrink(COMPLETION_POPUP_MARGIN);

    let width = size.x.min(area.width());
    let height = size.y;

    let x = cursor_top.x.min(area.max.x - width).max(area.min.x);

    let fits_above = cursor_top.y - height >= area.min.y;
    let fits_below = cursor_bot.y + height <= area.max.y;
    let y = if fits_above {
        cursor_top.y - height
    } else if fits_below {
        cursor_bot.y
    } else {
        cursor_bot.y.min(area.max.y - height).max(area.min.y)
    };

    Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height))
}
