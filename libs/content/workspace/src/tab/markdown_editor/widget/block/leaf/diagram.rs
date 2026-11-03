//! A closed ```` ```mermaid ```` fence draws as its diagram. Collapsed, the
//! whole fence is one atom, like an image. With the caret inside, the source
//! shows as code with the last good render below it.

use comrak::nodes::{AstNode, NodeCodeBlock, NodeValue};
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2};
use epaint::RectShape;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;

use crate::style::phosphor;
use crate::tab::markdown_editor::input::Event;
use crate::tab::markdown_editor::row_scroll::RowScroller;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{
    EmbedKind, EmbedSpec, Layout, WrapUnitLayout,
};
use crate::tab::markdown_editor::{MdEdit, MdRender, TouchTarget, View};
use crate::tab::{ContextMenuTarget, ExtendedOutput as _};
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::diagram_cache::DiagramSize;
use crate::widgets::glyphon_label::{GlyphonLabel, ShapedLabel};

const PENDING_HEIGHT: f32 = 120.0;
/// How far a diagram shrinks to fit the note: its 16pt labels stay about
/// 11pt. A diagram still wider scrolls along its width, as a wide table does.
const MIN_SCALE: f32 = 0.7;
const ERROR_PAD: f32 = 12.0;
const RADIUS: f32 = 4.0;

/// What shows below a revealed diagram's source: its error, then the last
/// render that worked with that render's size.
struct Preview {
    error: Option<ShapedLabel>,
    image: Option<(String, Vec2)>,
}

impl<'ast> MdRender {
    pub fn is_diagram(node_code_block: &NodeCodeBlock) -> bool {
        node_code_block.fenced
            && node_code_block
                .info
                .split_whitespace()
                .next()
                .is_some_and(|lang| lang.eq_ignore_ascii_case("mermaid"))
    }

    /// A diagram fence with content and a closing fence. An unclosed fence
    /// runs to the end of the note, so it stays code.
    pub fn diagram_eligible(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> bool {
        if !Self::is_diagram(node_code_block) || self.view == View::Source {
            return false;
        }
        let first = self.node_first_line_idx(node);
        let last = self.node_last_line_idx(node);
        last > first + 1
            && self.is_closing_fence(node, node_code_block, self.bounds.source_lines[last])
    }

    pub fn diagram_collapsed(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> bool {
        self.diagram_eligible(node, node_code_block)
            && !self.range_revealed_interior(self.node_range(node))
    }

    /// The fence's content lines, without fence indent or container prefixes.
    pub fn diagram_source(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> String {
        let first = self.node_first_line_idx(node);
        let last = self.node_last_line_idx(node);
        let lines: Vec<&str> = (first + 1..last)
            .map(|idx| {
                let node_line = self.node_line(node, self.bounds.source_lines[idx]);
                let code = self.code_line_range(node, node_code_block, node_line, false);
                &self.buffer[code]
            })
            .collect();
        lines.join("\n")
    }

    /// The diagram's box: its natural size, shrunk to fit `width` but no
    /// smaller than [`MIN_SCALE`] when `floored`.
    fn diagram_box(&self, source: &str, width: f32, floored: bool) -> Vec2 {
        match self.diagrams.size(source) {
            DiagramSize::Ready(natural) => {
                let fit = width / natural.x;
                let scale = if floored { fit.max(MIN_SCALE) } else { fit }.min(1.0);
                (natural * scale).max(Vec2::splat(1.0))
            }
            DiagramSize::Pending => vec2(width, PENDING_HEIGHT),
            DiagramSize::Failed(message) => {
                let label = self.diagram_error_label(&message, width - 2.0 * ERROR_PAD);
                vec2(width, label.size.y + 2.0 * ERROR_PAD)
            }
        }
    }

    fn diagram_error_label(&self, message: &str, width: f32) -> ShapedLabel {
        let text = format!("Diagram error: {}", message_detail(message));
        GlyphonLabel::new(&text, self.ctx.get_lb_theme().danger())
            .font_size(14.0)
            .max_width(width.max(1.0))
            .build(&self.ctx)
    }

    pub fn diagram_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::unique(("md_diagram", node_range))
    }

    /// The collapsed diagram as one embed over the whole fence, and the
    /// embed's width: the window a diagram wider than the note scrolls in.
    fn diagram_layout(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> (WrapUnitLayout, f32) {
        let range = self.node_range(node);
        let width = self.width(node);
        let source = self.diagram_source(node, node_code_block);
        let size = self.diagram_box(&source, width, true);
        let advance = size.x.min(width);
        let mut layout = Layout::new(range);
        layout.interaction_open(Self::diagram_interaction_id_salt(range), Sense::click());
        layout.push_embed(EmbedSpec {
            advance,
            ascent: size.y,
            descent: 0.0,
            source_range: range,
            url: source,
            kind: EmbedKind::Diagram,
        });
        layout.interaction_close();
        (self.compute_layout_from(layout, width, self.layout.row_height), advance)
    }

    pub fn height_diagram(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> f32 {
        self.diagram_layout(node, node_code_block).0.height
    }

    pub fn show_diagram(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2,
        node_code_block: &NodeCodeBlock,
    ) {
        let (layout, box_width) = self.diagram_layout(node, node_code_block);
        let inset = ((self.width(node) - box_width) / 2.0).max(0.0);
        self.show_wrap_layout(ui, top_left + vec2(inset, 0.0), &layout);
        let first_line = self.bounds.source_lines[self.node_first_line_idx(node)];
        self.show_block_line_prefixes(ui, node, first_line, top_left, self.layout.row_height);
    }

    /// A collapsed diagram in its `window`, scrolled along its width when it
    /// is wider than the note.
    pub fn paint_diagram_embed(
        &mut self, ui: &mut Ui, source: &str, range: (Grapheme, Grapheme), window: Rect,
    ) {
        let full = self.diagram_box(source, window.width(), true);
        let ordinal = self.bounds.diagrams.iter().position(|d| *d == range);
        let Some(ordinal) = ordinal.filter(|_| full.x > window.width() + 0.5) else {
            return self.paint_diagram(ui, source, window);
        };
        let key = RowScroller::Diagram(ordinal);
        let clip = window.intersect(ui.clip_rect());
        let offset = if self.painting_drag_float {
            self.row_scrolled(key)
        } else {
            self.row_scrollers.push((clip, key));
            self.row_scroll(ui, key, window, full.x, |_, _, _| None)
        };

        let mut clipped = ui.new_child(UiBuilder::new().max_rect(window));
        clipped.set_clip_rect(clip);
        let rect = Rect::from_min_size(window.min - vec2(offset, 0.0), full);
        self.paint_diagram(&mut clipped, source, rect);
        let bg = self.ctx.get_lb_theme().neutral_bg();
        let bands = [(window.top(), window.bottom(), bg)];
        self.row_scroll_chrome(ui, key, window, clip, full.x, &bands);
    }

    pub fn paint_diagram(&mut self, ui: &mut Ui, source: &str, rect: Rect) {
        let theme = self.ctx.get_lb_theme();
        let painter = ui.painter();
        let width_px = (rect.width() * self.ctx.pixels_per_point()).round() as u32;
        match self.diagrams.size(source) {
            DiagramSize::Ready(_) => match self.diagrams.texture(source, width_px) {
                Some(texture) => {
                    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                    painter.add(
                        RectShape::filled(rect, 0.0, Color32::WHITE).with_texture(texture, uv),
                    );
                }
                None => {
                    painter.rect_filled(rect, RADIUS, theme.neutral_bg_secondary());
                }
            },
            DiagramSize::Pending => {
                painter.rect_filled(rect, RADIUS, theme.neutral_bg_secondary());
            }
            DiagramSize::Failed(message) => {
                painter.rect_filled(rect, RADIUS, theme.neutral_bg_secondary());
                painter.rect_stroke(
                    rect,
                    RADIUS,
                    Stroke::new(1.0_f32, theme.neutral_bg_tertiary()),
                    StrokeKind::Inside,
                );
                let label = self.diagram_error_label(&message, rect.width() - 2.0 * ERROR_PAD);
                let at = Rect::from_min_size(rect.min + Vec2::splat(ERROR_PAD), label.size);
                self.text_areas
                    .push(label.text_area(at, &self.ctx, ui.clip_rect()));
            }
        }
    }

    fn diagram_preview(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> Option<Preview> {
        if !self.diagram_eligible(node, node_code_block) {
            return None;
        }
        let source = self.diagram_source(node, node_code_block);
        let width = self.width(node);
        let start = self.node_range(node).start();
        let mut previews = self.diagram_previews.borrow_mut();
        let (error, shown) = match self.diagrams.size(&source) {
            DiagramSize::Ready(_) => {
                previews.insert(start, source.clone());
                (None, Some(source))
            }
            DiagramSize::Pending => (None, previews.get(&start).cloned()),
            DiagramSize::Failed(message) => {
                (Some(self.diagram_error_label(&message, width)), previews.get(&start).cloned())
            }
        };
        let image = shown.and_then(|shown| match self.diagrams.size(&shown) {
            DiagramSize::Ready(_) => {
                let size = self.diagram_box(&shown, width, false);
                Some((shown, size))
            }
            _ => None,
        });
        Some(Preview { error, image })
    }

    pub fn height_diagram_preview(
        &self, node: &'ast AstNode<'ast>, node_code_block: &NodeCodeBlock,
    ) -> f32 {
        let Some(Preview { error, image }) = self.diagram_preview(node, node_code_block) else {
            return 0.0;
        };
        let gap = self.layout.block_padding;
        error.map_or(0.0, |l| gap + l.size.y) + image.map_or(0.0, |(_, size)| gap + size.y)
    }

    pub fn show_diagram_preview(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2,
        node_code_block: &NodeCodeBlock,
    ) {
        let Some(Preview { error, image }) = self.diagram_preview(node, node_code_block) else {
            return;
        };
        let gap = self.layout.block_padding;
        let width = self.width(node);
        if let Some(label) = error {
            top_left.y += gap;
            let rect = Rect::from_min_size(top_left, label.size);
            self.text_areas
                .push(label.text_area(rect, &self.ctx, ui.clip_rect()));
            top_left.y += rect.height();
        }
        if let Some((source, size)) = image {
            top_left.y += gap;
            let inset = ((width - size.x) / 2.0).max(0.0);
            let rect = Rect::from_min_size(top_left + vec2(inset, 0.0), size);
            self.paint_diagram(ui, &source, rect);
        }
    }

    /// Recompute [`super::super::super::super::bounds::Bounds::diagrams`].
    /// Depends on text only.
    pub fn calc_diagram_bounds<'a>(&mut self, root: &'a AstNode<'a>) {
        let mut diagrams = Vec::new();
        for node in root.descendants() {
            if let NodeValue::CodeBlock(node_code_block) = &node.data.borrow().value {
                if self.diagram_eligible(node, node_code_block) {
                    diagrams.push(self.node_range(node));
                }
            }
        }
        diagrams.sort_unstable();
        // scroll offsets go by a diagram's place among the diagrams
        if diagrams.len() != self.bounds.diagrams.len() {
            self.row_scrolls
                .retain(|key, _| !matches!(key, RowScroller::Diagram(_)));
        }
        self.diagram_previews
            .borrow_mut()
            .retain(|start, _| diagrams.iter().any(|d| d.start() == *start));
        self.bounds.diagrams = diagrams;
    }

    /// Diagrams showing as pictures, which select and delete as a unit.
    pub fn collapsed_diagrams(&self) -> impl Iterator<Item = (Grapheme, Grapheme)> + '_ {
        self.bounds
            .diagrams
            .iter()
            .copied()
            .filter(|range| !self.range_revealed_interior(*range))
    }
}

/// merman's message without its `Diagram parse error (kind): ` lead.
fn message_detail(message: &str) -> &str {
    match message.strip_prefix("Diagram parse error") {
        Some(rest) => rest.split_once("): ").map_or(message, |(_, detail)| detail),
        None => message,
    }
}

/// A picture in a note: a collapsed image or diagram.
pub enum Picture {
    Image(String),
    Diagram(String),
}

impl<'ast> MdRender {
    /// The collapsed image or diagram the selection exactly covers.
    pub fn selected_picture(&self, root: &'ast AstNode<'ast>) -> Option<Picture> {
        let selection = self.buffer.current.selection;
        if selection.is_empty() || self.disable_images {
            return None;
        }
        root.descendants().find_map(|node| {
            if self.node_range(node) != selection {
                return None;
            }
            match &node.data.borrow().value {
                NodeValue::CodeBlock(ncb) if self.diagram_collapsed(node, ncb) => {
                    Some(Picture::Diagram(self.diagram_source(node, ncb)))
                }
                NodeValue::Image(link) if !self.range_revealed_interior(selection) => {
                    Some(Picture::Image(link.url.clone()))
                }
                _ => None,
            }
        })
    }

    /// Whether the platform edit menu offers "Open": the selection is a
    /// collapsed diagram.
    pub fn selection_has_diagram(&mut self) -> bool {
        let arena = comrak::Arena::new();
        let root = self.reparse(&arena);
        matches!(self.selected_picture(root), Some(Picture::Diagram(_)))
    }

    /// Whether the platform edit menu offers "Copy Image".
    pub fn selection_has_picture(&mut self) -> bool {
        let arena = comrak::Arena::new();
        let root = self.reparse(&arena);
        self.selected_picture(root).is_some()
    }

    /// Puts `picture`'s pixels on the clipboard, now or once an image loads.
    pub fn copy_picture(&self, picture: Picture) {
        match picture {
            Picture::Diagram(source) => {
                if let Some((image, _)) = self.diagrams.image(&source) {
                    self.ctx.copy_image(image);
                }
            }
            Picture::Image(url) => self.embeds.copy_image(&url, &self.ctx),
        }
    }
}

#[derive(Clone)]
enum DiagramMenu {
    Open,
    CopyImage,
    Copy,
    Edit,
}

impl<'ast> MdEdit {
    /// Escape in a diagram's source selects the diagram, which folds it back
    /// into its picture: the way out of what Edit opened.
    pub fn escape_diagram_source(
        &mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui, id: egui::Id, ops: &mut Vec<Operation>,
    ) {
        if self.emoji_completions.active
            || self.link_completions.active
            || !ui.memory(|m| m.has_focus(id))
        {
            return;
        }
        let selection = self.renderer.buffer.current.selection;
        let diagram = self.renderer.bounds.diagrams.iter().copied().find(|d| {
            d.contains_range(&selection, true, true) && self.renderer.range_revealed_interior(*d)
        });
        let Some(diagram) = diagram else { return };
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            let select = Event::Select { region: diagram.into() };
            self.calc_operations(ui.ctx(), root, select, ops);
        }
    }

    /// A tap selects a collapsed diagram (and, on touch, pops the atom menu);
    /// desktop right-click shows Copy Image / Copy / Edit.
    pub fn handle_diagram_interactions(
        &mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui, id: egui::Id, ops: &mut Vec<Operation>,
    ) {
        for node in root.descendants() {
            let NodeValue::CodeBlock(ncb) = &node.data.borrow().value else {
                continue;
            };
            if !self.renderer.diagram_collapsed(node, ncb) {
                continue;
            }
            let range = self.renderer.node_range(node);
            let salt = MdRender::diagram_interaction_id_salt(range);
            let Some(response) = self
                .renderer
                .interaction_responses
                .get(&ui.scope_id().with(salt))
            else {
                continue;
            };
            let response = response.clone();
            let select = Event::Select { region: range.into() };
            self.renderer.touch_consume_interaction(
                ui.scope_id().with(salt),
                TouchTarget::Tap(select.clone()),
            );
            if response.double_clicked() {
                ui.ctx()
                    .open_diagram(self.renderer.diagram_source(node, ncb));
            } else if response.clicked() {
                ui.memory_mut(|m| m.request_focus(id));
                self.calc_operations(ui.ctx(), root, select, ops);
                if self.renderer.touch_mode {
                    ui.ctx()
                        .set_context_menu(response.rect.center_top(), ContextMenuTarget::Atom);
                }
            }
            if self.renderer.touch_mode {
                continue;
            }
            let editable = !self.renderer.readonly;
            let theme = ui.ctx().get_lb_theme();
            let action = crate::style::context_menu::show(&response, &theme, |e| {
                e.item(phosphor::ARROW_SQUARE_OUT, "Open", DiagramMenu::Open);
                e.item(phosphor::IMAGE, "Copy Image", DiagramMenu::CopyImage);
                e.item(phosphor::COPY, "Copy", DiagramMenu::Copy);
                if editable {
                    e.item(phosphor::PENCIL, "Edit", DiagramMenu::Edit);
                }
            });
            match action {
                Some(DiagramMenu::Open) => {
                    ui.ctx()
                        .open_diagram(self.renderer.diagram_source(node, ncb));
                }
                Some(DiagramMenu::CopyImage) => {
                    let source = self.renderer.diagram_source(node, ncb);
                    self.renderer.copy_picture(Picture::Diagram(source));
                }
                Some(DiagramMenu::Copy) => ui.ctx().copy_text(self.renderer.buffer[range].into()),
                Some(DiagramMenu::Edit) => {
                    ui.memory_mut(|m| m.request_focus(id));
                    let caret = self.diagram_edit_caret(node);
                    self.calc_operations(
                        ui.ctx(),
                        root,
                        Event::Select { region: caret.into() },
                        ops,
                    );
                }
                None => {}
            }
        }
    }

    /// The end of a diagram's last source line: where Edit puts the caret.
    fn diagram_edit_caret(&self, node: &'ast AstNode<'ast>) -> (Grapheme, Grapheme) {
        let r = &self.renderer;
        let last_content = r.bounds.source_lines[r.node_last_line_idx(node) - 1];
        (last_content.end(), last_content.end())
    }

    /// If the selection exactly covers a collapsed diagram, put the caret in
    /// its source, revealing it. True if handled (op pushed).
    pub fn enter_at_diagram(
        &self, root: &'ast AstNode<'ast>, operations: &mut Vec<Operation>,
    ) -> bool {
        let selection = self.renderer.buffer.current.selection;
        let Some(node) = root.descendants().find(|node| {
            matches!(&node.data.borrow().value, NodeValue::CodeBlock(ncb)
                if self.renderer.node_range(node) == selection
                    && self.renderer.diagram_collapsed(node, ncb))
        }) else {
            return false;
        };
        operations.push(Operation::Select(self.diagram_edit_caret(node)));
        true
    }
}
