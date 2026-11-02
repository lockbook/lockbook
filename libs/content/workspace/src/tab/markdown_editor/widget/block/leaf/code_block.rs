use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use comrak::nodes::{AstNode, NodeCodeBlock};
use egui::{Align2, Color32, Context, Pos2, Rangef, Rect, Sense, Ui, UiBuilder, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _, RangeIterExt};

use comrak::nodes::NodeValue;

use crate::style::{FG_HOVER, Radius, phosphor, phosphor_font_id, tip_text};
use crate::syntax::{Highlights, Lang, Role, agreeing, roles_in};
use crate::tab::markdown_editor::input::Event;
use crate::tab::markdown_editor::row_scroll::RowScroller;
use crate::tab::markdown_editor::widget::utils::consume_indent_columns;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{
    FontFamily, Format, Layout, Row, WrapUnitLayout,
};
use crate::tab::markdown_editor::{MdRender, TouchTarget};

use crate::theme::palette_v2::ThemeExt as _;

/// Code lines in the editor are laid out this wide: they never wrap.
const UNBOUNDED: f32 = 100_000.0;

/// A line of a code block as laid out to paint, `top` its top edge.
struct CodeRow {
    line: (Grapheme, Grapheme),
    top: f32,
    layout: WrapUnitLayout,
    /// Code scrolls with the block; a revealed fence stays where it is.
    scrolls: bool,
}

/// How far right a laid-out row reaches.
fn row_right(row: &Row) -> f32 {
    row.fragments
        .iter()
        .map(|frag| frag.rect.right())
        .fold(0., f32::max)
}

/// A code block's copy button, painted over the text once it is.
pub struct CopyButton {
    rect: Rect,
    /// The block's fill as it shows, which the button covers its code with.
    ground: Color32,
    hovered: bool,
    /// Shows a check: the code was just copied.
    copied: bool,
}

impl CopyButton {
    pub fn paint(&self, ui: &Ui) {
        let theme = ui.ctx().get_lb_theme();
        let fill = if self.hovered {
            theme.wash_toward_neutral_fg(self.ground, FG_HOVER)
        } else {
            self.ground
        };
        let (glyph, ink) = if self.copied {
            (phosphor::CHECK, theme.neutral_fg())
        } else if self.hovered {
            (phosphor::COPY, theme.neutral_fg())
        } else {
            (phosphor::COPY, theme.neutral_fg_secondary())
        };
        let painter = ui.painter();
        painter.rect_filled(self.rect, Radius::Sm.corner(), fill);
        let size = self.rect.height() * 0.6;
        painter.text(self.rect.center(), Align2::CENTER_CENTER, glyph, phosphor_font_id(size), ink);
    }
}

impl<'ast> MdRender {
    pub fn text_format_code_block(&self, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        Format { family: FontFamily::Mono, ..parent_text_format }
    }

    pub fn height_code_block(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> f32 {
        if node_code_block.fenced {
            self.height_fenced_code_block(node, node_code_block)
        } else {
            self.height_indented_code_block(node, node_code_block, false)
        }
    }

    pub fn show_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_code_block: &NodeCodeBlock,
    ) {
        if node_code_block.fenced {
            self.show_fenced_code_block(ui, node, top_left, node_code_block);
        } else {
            self.show_indented_code_block(ui, node, top_left, node_code_block, false);
        }
    }

    pub fn height_fenced_code_block(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> f32 {
        if self.diagram_collapsed(node, node_code_block) {
            return self.height_diagram(node, node_code_block);
        }
        let width = self.width(node) - 2. * self.layout.block_padding;
        let row_height = self.layout.row_height;

        let mut result = self.layout.block_padding;
        result -= self.layout.row_spacing;

        let reveal = self.reveal_fenced_code_block(node, node_code_block);
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in first_line_idx..=last_line_idx {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            let is_opening_fence = line_idx == first_line_idx;
            let is_closing_fence = !is_opening_fence
                && line_idx == last_line_idx
                && self.is_closing_fence(node, node_code_block, line);

            if is_opening_fence || is_closing_fence {
                if reveal {
                    result += self.layout.row_spacing;
                    let fence_layout = self.compute_section_layout_new(
                        node_line,
                        width,
                        row_height,
                        self.text_format_syntax(),
                    );
                    result += fence_layout.height;
                }
            } else {
                result += self.layout.row_spacing;
                result += self.height_code_block_line(node, node_code_block, line, false);
            }
        }

        result + self.layout.block_padding + self.height_diagram_preview(node, node_code_block)
    }

    pub fn show_fenced_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_code_block: &NodeCodeBlock,
    ) {
        if self.diagram_collapsed(node, node_code_block) {
            self.show_diagram(ui, node, top_left, node_code_block);
            return;
        }
        let width = self.width(node);
        let preview_height = self.height_diagram_preview(node, node_code_block);
        let height = self.height_fenced_code_block(node, node_code_block) - preview_height;
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));
        let inner_width = width - 2. * self.layout.block_padding;
        let row_height = self.layout.row_height;

        let mut rows = Vec::new();
        let mut y = top_left.y + self.layout.block_padding - self.layout.row_spacing; // makes spacing logic simpler
        let reveal = self.reveal_fenced_code_block(node, node_code_block);
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in first_line_idx..=last_line_idx {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            let is_opening_fence = line_idx == first_line_idx;
            let is_closing_fence = !is_opening_fence
                && line_idx == last_line_idx
                && self.is_closing_fence(node, node_code_block, line);

            if is_opening_fence || is_closing_fence {
                if reveal {
                    y += self.layout.row_spacing;
                    let layout = self.compute_section_layout_new(
                        node_line,
                        inner_width,
                        row_height,
                        self.text_format_syntax(),
                    );
                    y += layout.height;
                    rows.push(CodeRow { line, top: y - layout.height, layout, scrolls: false });
                }
            } else {
                y += self.layout.row_spacing;
                let layout = self.code_row_layout(node, node_code_block, line, false);
                y += layout.height;
                rows.push(CodeRow { line, top: y - layout.height, layout, scrolls: true });
            }
        }
        self.show_code_rows(ui, node, rect, rows);
        self.show_diagram_preview(ui, node, rect.left_bottom(), node_code_block);
    }

    fn reveal_fenced_code_block(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> bool {
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);

        if first_line_idx == last_line_idx {
            return true; // only opening fence: always reveal
        }
        if first_line_idx + 1 == last_line_idx
            && self.is_closing_fence(node, node_code_block, self.bounds.source_lines[last_line_idx])
        {
            return true; // only opening + closing fence: always reveal
        }

        // Reveal when a reveal range touches any of the block's source
        // lines, *including* a container prefix (`> `): a cursor on the
        // prefix of an otherwise-collapsed fence line would otherwise have
        // no fragment, since the fence line isn't rendered.
        self.node_lines(node).iter().any(|line_idx| {
            let line = self.bounds.source_lines[line_idx];
            self.range_revealed(line, true)
        })
    }

    pub fn height_indented_code_block(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock, synthetic: bool,
    ) -> f32 {
        let mut result = 0.;
        let row_height = self.layout.row_height;

        let reveal = self.reveal_indented_code_block(node, synthetic);
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in first_line_idx..=last_line_idx {
            let line = self.bounds.source_lines[line_idx];

            if reveal {
                let node_line = self.node_line(node, line);
                let l = self.compute_section_layout_new(
                    node_line,
                    self.code_width(node),
                    row_height,
                    self.text_format_syntax(),
                );
                result += l.height;
            } else {
                result += self.height_code_block_line(node, node_code_block, line, synthetic);
            }

            if line_idx != last_line_idx {
                result += self.layout.row_spacing;
            }
        }

        result + 2. * self.layout.block_padding
    }

    // indented code blocks don't have an info string; the `info` parameter is
    // just used for syntax highlighting when rendering unsupported node types
    // like html blocks as code
    pub fn show_indented_code_block(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_code_block: &NodeCodeBlock, synthetic: bool,
    ) {
        let width = self.width(node);
        let row_height = self.layout.row_height;
        let height = self.height_indented_code_block(node, node_code_block, synthetic);
        let rect = Rect::from_min_size(top_left, Vec2::new(width, height));

        let mut rows = Vec::new();
        let mut y = top_left.y + self.layout.block_padding;
        let reveal = self.reveal_indented_code_block(node, synthetic);
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let layout = if reveal {
                let node_line = self.node_line(node, line);
                self.compute_section_layout_new(
                    node_line,
                    self.code_width(node),
                    row_height,
                    self.text_format_syntax(),
                )
            } else {
                self.code_row_layout(node, node_code_block, line, synthetic)
            };
            let h = layout.height;
            rows.push(CodeRow { line, top: y, layout, scrolls: true });
            y += h + self.layout.row_spacing;
        }
        self.show_code_rows(ui, node, rect, rows);
    }

    /// Paints a code block's box and its rows. Code does not wrap: a block
    /// whose code is wider than the box scrolls along its rows. A code
    /// block proper gets a button that copies its code.
    fn show_code_rows(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, rect: Rect, rows: Vec<CodeRow>,
    ) {
        let theme = self.ctx.get_lb_theme();
        crate::style::paint_doc_border(ui.painter(), rect, 2, &theme);

        let padding = self.layout.block_padding;
        let row_height = self.layout.row_height;
        let text_left = rect.left() + padding;
        let shown = rect.width() - 2. * padding;
        let content = rows
            .iter()
            .filter(|row| row.scrolls)
            .map(|row| row.layout.rows.iter().map(row_right).fold(0., f32::max))
            .fold(0., f32::max);
        let copies =
            self.interactive && matches!(node.data.borrow().value, NodeValue::CodeBlock(_));
        let button = self.copy_button_rect(rect);
        // on touch the button always shows: the code scrolls clear of it
        let content = if copies && self.touch_mode {
            content + rect.right() - button.left() - padding
        } else {
            content
        };
        let start = self.node_range(node).start();
        let ordinal = self.bounds.code_blocks.binary_search(&start);
        let key = RowScroller::Code(ordinal.unwrap_or_else(|ordinal| ordinal));
        let scrolls = content > shown + 0.5;
        let scroll = if scrolls && self.painting_drag_float {
            self.row_scrolled(key)
        } else if scrolls {
            self.row_scrollers
                .push((rect.intersect(ui.clip_rect()), key));
            let viewport = Rect::from_x_y_ranges(text_left..=text_left + shown, rect.y_range());
            self.row_scroll(ui, key, viewport, content, |r, at, _| {
                let row = rows
                    .iter()
                    .find(|row| row.scrolls && row.layout.source_range.contains(at, true, true))?;
                let x = row
                    .layout
                    .rows
                    .iter()
                    .flat_map(|row| &row.fragments)
                    .find(|frag| frag.source_range.contains(at, true, true))
                    .map_or(0., |frag| r.fragment_x(frag, at));
                Some(Rangef::new(x - row_height, x + row_height))
            })
        } else {
            self.row_scrolls.remove(&key);
            0.
        };

        // code scrolls inside the border, which stays put over the fades
        let inside = rect.shrink(1.);
        let clip = inside.x_range().intersection(ui.clip_rect().x_range());
        let clip = Rect::from_x_y_ranges(clip, ui.clip_rect().y_range());
        let mut clipped = ui.new_child(UiBuilder::new().max_rect(rect));
        clipped.set_clip_rect(clip);
        for row in &rows {
            let top_left = Pos2::new(text_left, row.top);
            if row.scrolls && scrolls {
                self.show_wrap_layout(&mut clipped, top_left - Vec2::new(scroll, 0.), &row.layout);
                if !self.painting_drag_float {
                    let y = row.top..=row.top + row.layout.height;
                    self.code_line_clips
                        .push((Rect::from_x_y_ranges(clip.x_range(), y), row.layout.source_range));
                }
            } else {
                self.show_wrap_layout(ui, top_left, &row.layout);
            }
            // Container gutter columns sit at the code block's outer left (the
            // deepest container's content origin), not inside the padded box.
            self.show_block_line_prefixes(
                ui,
                node,
                row.line,
                Pos2::new(rect.left(), row.top),
                row_height,
            );
        }

        // the fill composited: what the fades and the button cover code with
        let ground = theme.neutral_bg();
        if scrolls {
            // edge to edge, but a revealed fence stays put, unfaded
            let half_spacing = self.layout.row_spacing / 2.;
            fn fence(row: Option<&CodeRow>) -> Option<&CodeRow> {
                row.filter(|row| !row.scrolls)
            }
            let top = fence(rows.first())
                .map_or(inside.top(), |fence| fence.top + fence.layout.height + half_spacing);
            let bottom =
                fence(rows.last()).map_or(inside.bottom(), |fence| fence.top - half_spacing);
            let content = content + inside.width() - shown;
            let bands = [(top, bottom, ground)];
            self.row_scroll_chrome(ui, key, inside, clip, content, &bands);
        }

        if copies {
            self.show_copy_button(ui, node, rect, button, ground);
        }
    }

    /// Where a code block in `rect` has its copy button: over the end of its
    /// first row.
    fn copy_button_rect(&self, rect: Rect) -> Rect {
        let size = self.layout.row_height + self.layout.row_spacing;
        let inset = self.layout.block_padding - self.layout.row_spacing / 2.;
        let min = rect.right_top() + Vec2::new(-inset - size, inset);
        Rect::from_min_size(min, Vec2::splat(size))
    }

    /// Recompute [`Bounds::code_blocks`]. Depends on text only.
    pub fn calc_code_block_bounds(&mut self, root: &'ast AstNode<'ast>) {
        let drawn_as_code = |node: &&'ast AstNode<'ast>| {
            matches!(
                node.data.borrow().value,
                NodeValue::CodeBlock(_) | NodeValue::HtmlBlock(_) | NodeValue::FrontMatter(_)
            )
        };
        let code_blocks: Vec<_> = root
            .descendants()
            .filter(drawn_as_code)
            .map(|node| self.node_range(node).start())
            .collect();
        // scroll offsets go by a code block's place among them
        if code_blocks.len() != self.bounds.code_blocks.len() {
            self.row_scrolls
                .retain(|key, _| !matches!(key, RowScroller::Code(_)));
        }
        self.bounds.code_blocks = code_blocks;
    }

    /// The button at a code block's top right that copies its code, shown
    /// while the pointer is over the block (always, on touch).
    fn show_copy_button(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, rect: Rect, button: Rect,
        ground: Color32,
    ) {
        if self.painting_drag_float {
            return;
        }
        let range = self.node_range(node);
        let now = ui.input(|i| i.time);
        let copied = self
            .code_copied
            .filter(|(copied, until)| *copied == range && now < *until);
        if let Some((_, until)) = copied {
            ui.ctx().request_repaint_after_secs((until - now) as f32);
        }
        if !self.touch_mode && copied.is_none() && !ui.rect_contains_pointer(rect) {
            return;
        }

        let copy = Event::CopyCode { node: range };
        let resp = ui.interact(button, ui.scope_id().with(("code_copy", range)), Sense::click());
        if resp.clicked() {
            self.render_events.push(copy.clone());
        }
        let hovered = resp.hovered();
        tip_text(ui.ctx(), &resp, if copied.is_some() { "Copied" } else { "Copy code" });
        // a finger's reach, on the first row only
        let target = Rect::from_min_max(
            Pos2::new(button.right() - 44., rect.top()),
            Pos2::new(rect.right(), button.bottom() + self.layout.row_spacing / 2.),
        );
        self.touch_targets.push((target, TouchTarget::Tap(copy)));
        self.copy_buttons.push(CopyButton {
            rect: button,
            ground,
            hovered,
            copied: copied.is_some(),
        });
    }

    fn reveal_indented_code_block(&self, node: &'ast AstNode<'ast>, synthetic: bool) -> bool {
        let mut reveal = false;
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);

            // reveal if selection is inside the indentation, but not at the
            // indentation's end / content's start
            let indentation = (
                node_line.start(),
                node_line
                    .end()
                    .min(node_line.start() + if synthetic { 0 } else { 4 }),
            );
            if self.range_revealed(indentation, false)
                || self
                    .reveal_ranges()
                    .any(|rr| rr.contains(indentation.start(), true, true))
            {
                reveal = true;
                break;
            }
        }
        reveal
    }

    /// Compute the `(chunk_start, end)` slice of `node_line` that's the
    /// actual content of a code-block line (fence indent stripped for
    /// fenced; 4-col indent stripped for indented, combined with parent
    /// item padding if any).
    pub(crate) fn code_line_range(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        node_line: (Grapheme, Grapheme), synthetic: bool,
    ) -> (Grapheme, Grapheme) {
        let NodeCodeBlock { fenced, fence_offset, .. } = node_code_block;
        if *fenced {
            // "If the leading code fence is indented N spaces, then up to N spaces
            // of indentation are removed from each line of the content..."
            // https://github.github.com/gfm/#fenced-code-blocks
            let text = &self.buffer[node_line];
            let indentation = text
                .chars()
                .take_while(|&c| c == ' ')
                .count()
                .min(*fence_offset);
            (node_line.start() + indentation, node_line.end())
        } else {
            // "An indented code block ... minus four spaces of indentation."
            // https://github.github.com/gfm/#indented-code-blocks
            //
            // Strip column-aware. If inside a list item, combine with
            // the item's padding so a tab straddling the boundary
            // doesn't get attributed entirely to one side.
            let target_cols = if synthetic {
                0
            } else {
                let parent_item_padding = node
                    .ancestors()
                    .find_map(|a| match &a.data.borrow().value {
                        NodeValue::Item(nl) => Some(nl.padding),
                        _ => None,
                    })
                    .unwrap_or(0);
                parent_item_padding + 4
            };
            let text = &self.buffer[node_line];
            let strip_graphemes = consume_indent_columns(text, target_cols);
            let chunk_start = (node_line.start() + strip_graphemes).min(node_line.end());
            (chunk_start, node_line.end())
        }
    }

    /// The roles over a code line in order, or `None` if the block's
    /// language isn't one we know.
    fn code_line_regions(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        code_line: (Grapheme, Grapheme), synthetic: bool,
    ) -> Option<Vec<(Role, (Grapheme, Grapheme))>> {
        let lang = Lang::from_info(&node_code_block.info)?;
        let key = (self.node_range(node), self.text_seq);
        let unit = self.syntax.unit_for(key, || {
            let (text, lines) = self.code_block_text(node, node_code_block, synthetic);
            CodeUnit { highlights: self.syntax.highlights(lang, text, &self.ctx), lines }
        });
        let idx = unit
            .lines
            .binary_search_by_key(&code_line.start(), |(start, _)| *start)
            .ok()?;
        let offset = unit.lines[idx].1;
        let start = self.offset_to_byte(code_line.start());
        let len = self.buffer[code_line].len();
        let spans = unit.highlights.as_ref()?.spans(offset..offset + len);
        let regions = roles_in(&spans, offset..offset + len)
            .into_iter()
            .map(|(role, r)| {
                // a token can end inside a grapheme, like a digit before a keycap's marks
                let r = (start + (r.start - offset), start + (r.end - offset));
                (role, self.range_to_char_ceil(r))
            })
            .collect();
        Some(regions)
    }

    /// A code block's code as one text, a line per code line, with where in
    /// it each code line starts.
    pub(crate) fn code_block_text(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock, synthetic: bool,
    ) -> (String, Vec<(Grapheme, usize)>) {
        let mut text = String::new();
        let mut lines = Vec::new();
        let first_line_idx = self.node_first_line_idx(node);
        let last_line_idx = self.node_last_line_idx(node);
        for line_idx in first_line_idx..=last_line_idx {
            let line = self.bounds.source_lines[line_idx];
            let fence = node_code_block.fenced
                && (line_idx == first_line_idx
                    || line_idx == last_line_idx
                        && self.is_closing_fence(node, node_code_block, line));
            if fence {
                continue;
            }
            let node_line = self.node_line(node, line);
            let code_line = self.code_line_range(node, node_code_block, node_line, synthetic);
            lines.push((code_line.start(), text.len()));
            text.push_str(&self.buffer[code_line]);
            text.push('\n');
        }
        (text, lines)
    }

    /// Build a `Layout` for one code-block line. Each highlighted
    /// region becomes its own `push_source` with a per-region color
    /// (mono font + colored). Without highlighting, the whole line
    /// goes in as one push_source with the default code format.
    pub(crate) fn layout_code_block_line(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme), synthetic: bool,
    ) -> Layout {
        let node_line = self.node_line(node, line);
        let code_line = self.code_line_range(node, node_code_block, node_line, synthetic);
        let code_line_text = &self.buffer[code_line];
        let mut layout = Layout::new(code_line);
        let regions = self.code_line_regions(node, node_code_block, code_line, synthetic);
        if let Some(regions) = regions {
            let text_format = self.text_format(node);
            for (role, region) in regions {
                if !region.is_empty() {
                    let format = self.syntax_format(role, text_format.clone());
                    layout.push_source(region, &self.buffer[region], format);
                }
            }
        } else if !code_line.is_empty() {
            layout.push_source(code_line, code_line_text, self.text_format(node));
        }
        layout
    }

    fn height_code_block_line(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme), synthetic: bool,
    ) -> f32 {
        self.code_row_layout(node, node_code_block, line, synthetic)
            .height
    }

    /// One line of code, laid out at [`Self::code_width`].
    fn code_row_layout(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme), synthetic: bool,
    ) -> WrapUnitLayout {
        let layout = self.layout_code_block_line(node, node_code_block, line, synthetic);
        self.compute_layout_from(layout, self.code_width(node), self.layout.row_height)
    }

    /// How wide a line of code may be laid out: in the editor, where a block
    /// scrolls, however long it is; elsewhere, as wide as the block.
    fn code_width(&self, node: &'ast AstNode<'ast>) -> f32 {
        if self.interactive { UNBOUNDED } else { self.width(node) - 2. * self.layout.block_padding }
    }

    // "The closing code fence may be indented up to three spaces, and may be
    // followed only by spaces, which are ignored."
    // https://github.github.com/gfm/#fenced-code-blocks

    pub(crate) fn is_closing_fence(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
        line: (Grapheme, Grapheme),
    ) -> bool {
        let NodeCodeBlock { fence_char, fence_length, .. } = node_code_block;
        let fence_char = *fence_char as char;

        let node_line = self.node_line(node, line);

        let text = &self.buffer[node_line];
        let mut chars = text.chars().peekable();

        // Skip up to 3 leading spaces
        for _ in 0..3 {
            if chars.peek() == Some(&' ') {
                chars.next();
            } else {
                break;
            }
        }

        // Skip exactly fence_length fence_char's
        for _ in 0..*fence_length {
            if chars.peek() == Some(&fence_char) {
                chars.next();
            } else {
                return false;
            }
        }

        // Skip any number of additional fence_char's
        loop {
            if chars.peek() == Some(&fence_char) {
                chars.next();
            } else {
                break;
            }
        }

        // Any additional characters may be spaces only
        chars.all(|c| c == ' ')
    }
}

/// A unit of code parsed, and where each of its lines starts in it: by a
/// code line's first grapheme, its byte in the unit.
pub struct CodeUnit {
    pub highlights: Option<Arc<Highlights>>,
    pub lines: Vec<(Grapheme, usize)>,
}

/// Highlights kept while they're used: parsed units by language and text,
/// so a unit reparses only when its code changes, and units by their range
/// and text version, so a unit's text is gathered once per edit.
#[derive(Clone, Default)]
pub struct SyntaxHighlightCache {
    highlights: KeptWhileUsed<(Lang, String), Option<Arc<Highlights>>>,
    units: KeptWhileUsed<((Grapheme, Grapheme), u64), Arc<CodeUnit>>,
}

/// Entries, each marked whether it was used since the last collection.
type KeptWhileUsed<K, V> = RefCell<HashMap<K, (V, bool)>>;

impl SyntaxHighlightCache {
    /// `text` parsed as `lang`; a new text reparses from the kept version
    /// it agrees with most, as an edited unit's last version does.
    pub fn highlights(&self, lang: Lang, text: String, ctx: &Context) -> Option<Arc<Highlights>> {
        let key = (lang, text);
        if let Some((highlights, used)) = self.highlights.borrow_mut().get_mut(&key) {
            *used = true;
            return highlights.clone();
        }
        let closest = self
            .highlights
            .borrow()
            .iter()
            .filter_map(|((kept_lang, kept), (highlights, _))| {
                let highlights = highlights.as_ref().filter(|_| *kept_lang == lang)?;
                Some((agreeing(kept, &key.1), highlights.clone()))
            })
            .max_by_key(|(agree, _)| *agree)
            .map(|(_, highlights)| highlights);
        let highlights = match closest {
            Some(closest) => closest.edited(key.1.clone()),
            None => lang.highlights(key.1.clone(), ctx),
        }
        .map(Arc::new);
        self.highlights
            .borrow_mut()
            .insert(key, (highlights.clone(), true));
        highlights
    }

    pub fn unit_for(
        &self, key: ((Grapheme, Grapheme), u64), unit: impl FnOnce() -> CodeUnit,
    ) -> Arc<CodeUnit> {
        if let Some((unit, used)) = self.units.borrow_mut().get_mut(&key) {
            *used = true;
            return unit.clone();
        }
        let unit = Arc::new(unit());
        self.units.borrow_mut().insert(key, (unit.clone(), true));
        unit
    }

    pub fn garbage_collect(&self) {
        self.highlights
            .borrow_mut()
            .retain(|_, (_, used)| std::mem::take(used));
        self.units
            .borrow_mut()
            .retain(|_, (_, used)| std::mem::take(used));
    }
}
