use comrak::nodes::{AstNode, NodeHeading, NodeValue};
use egui::{Id, TextFormat, Ui, UiBuilder};
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RangeIterExt as _};

use super::Editor;
use super::bounds::RangesExt as _;

pub(crate) mod block;
pub(crate) mod find;
pub(crate) mod inline;
pub(crate) mod toolbar;
pub(crate) mod utils;

pub const MARGIN: f32 = 45.0; // space between the editor and window border; must be large enough to accommodate bordered elements e.g. code blocks
pub const MAX_WIDTH: f32 = 1000.0; // the maximum width of the editor before it starts adding padding

pub const INLINE_PADDING: f32 = 5.0; // the extra space granted to inline code for a border (both sides)
pub const ROW_HEIGHT: f32 = 20.0; // ...at default font size
pub const BLOCK_PADDING: f32 = 10.0; // between a table cell / code block and its contents (all sides)
pub const INDENT: f32 = 30.0; // enough space for two digits in a numbered list
pub const BULLET_RADIUS: f32 = 2.0;
pub const ROW_SPACING: f32 = 5.0; // must be large enough to accommodate bordered elements e.g. inline code
pub const BLOCK_SPACING: f32 = 10.0;

impl<'ast> Editor {
    /// Returns the range for the node.
    pub fn node_range(&self, node: &'ast AstNode<'ast>) -> (DocCharOffset, DocCharOffset) {
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
    pub fn range_lines(&self, range: (DocCharOffset, DocCharOffset)) -> (usize, usize) {
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

    pub fn text_format(&self, node: &AstNode<'_>) -> TextFormat {
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
            NodeValue::Image(_) => self.text_format_image(parent()),
            NodeValue::LineBreak => parent_text_format(),
            NodeValue::Link(_) => self.text_format_link(parent()),
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
            NodeValue::WikiLink(_) => self.text_format_wiki_link(parent()),

            // leaf_block
            NodeValue::CodeBlock(_) => self.text_format_code_block(parent()),
            NodeValue::DescriptionDetails => unimplemented!("extension disabled"),
            NodeValue::DescriptionTerm => unimplemented!("extension disabled"),
            NodeValue::Heading(NodeHeading { level, .. }) => {
                self.text_format_heading(parent(), *level)
            }
            NodeValue::HtmlBlock(_) => self.text_format_html_block(parent()),
            NodeValue::Paragraph => parent_text_format(),
            NodeValue::TableCell => parent_text_format(),
            NodeValue::TaskItem(_) => parent_text_format(),
            NodeValue::ThematicBreak => parent_text_format(),
        }
    }

    pub fn text_format_syntax(&self, node: &'ast AstNode<'ast>) -> TextFormat {
        let mono = self.text_format_code(node);

        TextFormat {
            color: if self.plaintext_mode {
                self.theme.fg().neutral_primary
            } else {
                self.theme.fg().neutral_quarternary
            },
            background: Default::default(),
            underline: Default::default(),
            strikethrough: Default::default(),
            italics: Default::default(),
            ..mono
        }
    }

    fn row_height(&self, node: &AstNode<'_>) -> f32 {
        let text_format = self.text_format(node).font_id;
        self.ctx.fonts(|fonts| fonts.row_height(&text_format))
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
            NodeValue::List(_) => self.compute_bounds_list(node),
            NodeValue::MultilineBlockQuote(_) => unimplemented!("extension disabled"),
            NodeValue::Table(_) => self.compute_bounds_table(node),
            NodeValue::TableRow(_) => self.compute_bounds_table_row(node),
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
}
