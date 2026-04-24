use comrak::nodes::{AstNode, NodeHeading, NodeValue};
use egui::{CornerRadius, Rect, Stroke, StrokeKind, Ui, UiBuilder};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _, RangeIterExt as _};

use crate::tab::markdown_editor::widget::utils::wrap_layout::{FontFamily, Format};
use crate::theme::palette_v2::ThemeExt as _;

use super::MdRender;
use super::bounds::RangesExt as _;

pub(crate) mod block;
pub(crate) mod debug;
pub(crate) mod emoji_completions;
pub(crate) mod find;
pub(crate) mod inline;
pub(crate) mod link_completions;
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
                let children = self.sorted_children(node);
                let last_child = children.last().unwrap();
                range.1 = self.node_range(last_child).1;
            }

            // hack: comrak misreports the end column of an HtmlBlock that
            // sits inside a `>\t`-prefixed blockquote — it overshoots by
            // the tab's expanded-vs-source width (e.g. for `>\t<div>foo
            // </div>` it reports end col 18 instead of 16). The over-
            // wide end leaks the range onto the next line and causes the
            // following block's first line to be re-rendered as part of
            // the HtmlBlock. Confirmed in `probe_sourcepos_column_semantics`
            // that this misreport is unique to this combination — every
            // other tab-blockquote shape (paragraph, code span, list-item
            // child) reports source-aligned columns. Clamp to the end of
            // the sourcepos's last line.
            NodeValue::HtmlBlock(_) => {
                let last_line_idx = node_data.sourcepos.end.line.saturating_sub(1);
                if let Some(line) = self.bounds.source_lines.get(last_line_idx) {
                    range.1 = range.1.min(line.end());
                }
            }

            _ => {}
        }

        // Cache the result before returning
        self.set_cached_node_range(node, range);
        range
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
        let range_lines = self.range_split_newlines(range);

        let first_line = *range_lines.first().unwrap();
        let start_line_idx = self
            .bounds
            .source_lines
            .find_containing(first_line.start(), true, true)
            .start();

        let last_line = *range_lines.last().unwrap();
        let end_line_idx = self
            .bounds
            .source_lines
            .find_containing(last_line.end(), true, true)
            .end(); // note: preserves (inclusive, exclusive) behavior

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
            NodeValue::SpoileredText => self.text_format_spoilered_text(parent()),
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
        match hex {
            "#000000" => theme.neutral_fg(),
            "#111111" => theme.neutral_fg_secondary(),
            "#222222" => theme.fg().get_color(theme.prefs().primary),
            "#333333" => theme.fg().get_color(theme.prefs().secondary),
            "#444444" => theme.fg().get_color(theme.prefs().tertiary),
            "#555555" => theme.fg().get_color(theme.prefs().quaternary),
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

    /// Draws the background frame and per-row highlights for a completion popup.
    /// Text rendering is handled separately by each completion type.
    pub fn draw_completion_popup(
        &self, ui: &Ui, popup_rect: Rect, row_rects: &[Rect], selected: usize,
        hover_pos: Option<egui::Pos2>,
    ) {
        let vis = ui.visuals();
        let bg = vis.extreme_bg_color;
        let hover_bg = vis.widgets.hovered.bg_fill;
        let selected_bg = vis.selection.bg_fill.gamma_multiply(0.3);
        let border_color = vis.widgets.noninteractive.bg_stroke.color;

        let cr = CornerRadius::same(self.layout.completion_corner_radius);
        let painter = ui.painter();
        painter.rect(popup_rect, cr, bg, Stroke::new(1.0, border_color), StrokeKind::Outside);
        let last = row_rects.len().saturating_sub(1);
        for (idx, rect) in row_rects.iter().enumerate() {
            let row_cr = CornerRadius {
                nw: if idx == 0 { self.layout.completion_corner_radius } else { 0 },
                ne: if idx == 0 { self.layout.completion_corner_radius } else { 0 },
                sw: if idx == last { self.layout.completion_corner_radius } else { 0 },
                se: if idx == last { self.layout.completion_corner_radius } else { 0 },
            };
            if idx == selected {
                painter.rect_filled(*rect, row_cr, selected_bg);
            } else if hover_pos.is_some_and(|p| rect.contains(p)) {
                painter.rect_filled(*rect, row_cr, hover_bg);
            }
        }
    }
}
