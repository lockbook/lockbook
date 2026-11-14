use comrak::nodes::{AstNode, NodeValue};
use egui::{Color32, Pos2, Rect, Sense, Ui, pos2, vec2};
use epaint::RectShape;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;

use crate::style::overlay_scroll::{BAR_INSET, BAR_WIDTH};
use crate::style::phosphor;
use crate::tab::markdown_editor::input::Event;
use crate::tab::markdown_editor::row_scroll::RowScroller;
use crate::tab::markdown_editor::widget::block::leaf::diagram::Picture;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{
    EmbedKind, EmbedSpec, Format, Layout,
};
use crate::tab::markdown_editor::{MdEdit, MdRender, TouchTarget};
use crate::tab::{ContextMenuTarget, Destination, ExtendedOutput as _};
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::glyphon_label::{GlyphonLabel, ShapedLabel};
use crate::widgets::math::{Formula, MathSize, Typeset};

impl<'ast> MdRender {
    pub fn text_format_math(&self, parent: &AstNode<'_>) -> Format {
        self.text_format_code(parent)
    }

    /// The box `node` draws as on `line`: its formula sized to the text it
    /// sits in, and in a table cell scaled down to fit like an image. `None`
    /// shows its source: while the caret is inside it, as an image reveals,
    /// while it renders, when it spans lines, and when it doesn't typeset.
    pub fn math_size(
        &self, node: &'ast AstNode<'ast>, line: (Grapheme, Grapheme),
    ) -> Option<MathSize> {
        let range = self.node_range(node);
        if !line.contains_range(&range, true, true) || self.range_revealed_interior(range) {
            return None;
        }
        let block = self.math_block(node);
        let cell = block.filter(|b| matches!(b.data.borrow().value, NodeValue::TableCell));
        match self.math_typeset(node)? {
            Typeset::Ready(mut size) => {
                let room = cell.map_or(size.size.x, |b| self.embed_room(b));
                if size.size.x > room {
                    let scale = room / size.size.x;
                    size.size *= scale;
                    size.baseline *= scale;
                } else if cell.is_none() && block.is_some_and(|b| size.size.x > self.width(b)) {
                    // wider than its row, it scrolls with its bar below the glyphs
                    size.size.y += BAR_WIDTH + 2.0 * BAR_INSET;
                }
                Some(size)
            }
            _ => None,
        }
    }

    /// Where `node`'s render stands, sized to the text it sits in.
    pub(crate) fn math_typeset(&self, node: &'ast AstNode<'ast>) -> Option<Typeset> {
        let formula = Self::formula(node)?;
        let block = self.math_block(node);
        let row_height = block.map_or(self.layout.row_height, |b| self.row_height(b));
        let color = self.ctx.get_lb_theme().neutral_fg();
        Some(self.math.size(&formula, row_height, color))
    }

    pub(crate) fn formula(node: &'ast AstNode<'ast>) -> Option<Formula> {
        let NodeValue::Math(math) = &node.data.borrow().value else { return None };
        Some(Formula { latex: math.literal.clone(), display: math.display_math })
    }

    /// The errors below paragraph `node`, as a diagram's show below its
    /// source: one for each display formula on it that doesn't typeset.
    fn math_errors(&self, node: &'ast AstNode<'ast>) -> Vec<ShapedLabel> {
        let width = self.width(node).max(1.0);
        node.descendants()
            .filter(|n| matches!(&n.data.borrow().value, NodeValue::Math(m) if m.display_math))
            .filter_map(|n| match self.math_typeset(n)? {
                Typeset::Failed { message, .. } => Some(message),
                _ => None,
            })
            .map(|message| {
                let detail = message.strip_prefix("error: ").unwrap_or(&message);
                let text = format!("Math error: {detail}");
                GlyphonLabel::new(&text, self.ctx.get_lb_theme().danger())
                    .font_size(14.0)
                    .max_width(width)
                    .build(&self.ctx)
            })
            .collect()
    }

    pub fn height_math_errors(&self, node: &'ast AstNode<'ast>) -> f32 {
        let gap = self.layout.block_padding;
        self.math_errors(node).iter().map(|l| gap + l.size.y).sum()
    }

    pub fn show_math_errors(&mut self, ui: &Ui, node: &'ast AstNode<'ast>, mut top_left: Pos2) {
        for label in self.math_errors(node) {
            top_left.y += self.layout.block_padding;
            let rect = Rect::from_min_size(top_left, label.size);
            self.text_areas
                .push(label.text_area(rect, &self.ctx, ui.clip_rect()));
            top_left.y += rect.height();
        }
    }

    /// The leaf block a formula sits in: a heading's text, else the body's.
    fn math_block(&self, node: &'ast AstNode<'ast>) -> Option<&'ast AstNode<'ast>> {
        node.ancestors()
            .skip(1)
            .find(|a| a.data.borrow().value.is_leaf_block())
    }

    /// A formula on its baseline; display math across its row, centered.
    pub fn layout_math(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let Some(math) = self.math_size(node, range) else {
            // its source, underlined when it doesn't typeset
            let source = self.node_range(node);
            if range.contains_range(&source, true, true)
                && matches!(self.math_typeset(node), Some(Typeset::Failed { .. }))
            {
                layout.push_error(source);
            }
            return self.layout_code(layout, node, range);
        };
        let display = matches!(&node.data.borrow().value, NodeValue::Math(m) if m.display_math);
        // past the room its row has, a formula scrolls in a window that wide,
        // and display math spans its row; a table cell's width comes from
        // what it holds, which `math_size` already fits it in
        let row = self
            .math_block(node)
            .filter(|b| !matches!(b.data.borrow().value, NodeValue::TableCell))
            .map(|b| (self.width(b), matches!(b.data.borrow().value, NodeValue::Paragraph)));
        let advance = match row {
            Some((room, true)) if display => room,
            Some((room, _)) => math.size.x.min(room),
            None => math.size.x,
        };
        let source_range = self.node_range(node);
        layout.interaction_open(Self::math_interaction_id_salt(source_range), Sense::click());
        layout.push_embed(EmbedSpec {
            advance,
            ascent: math.baseline,
            descent: math.size.y - math.baseline,
            source_range,
            url: format!("{:016x}", math.key),
            kind: EmbedKind::Math,
        });
        layout.interaction_close();
    }

    pub fn math_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::unique(("md_math", node_range))
    }

    /// A formula in its `window`: scaled down with a table cell's room,
    /// centered across a display row, and scrolled along its width when it
    /// is wider than its row.
    pub fn paint_math(
        &mut self, ui: &mut Ui, key: &str, range: (Grapheme, Grapheme), window: Rect,
    ) {
        let Ok(key) = u64::from_str_radix(key, 16) else { return };
        let color = self.ctx.get_lb_theme().neutral_fg();
        let Some(natural) = self.math.natural_size(key) else { return };
        let size = natural * (window.height() / natural.y).min(1.);
        let width_px = (size.x * ui.ctx().pixels_per_point()).round() as u32;
        let Some(texture) = self.math.texture(key, color, width_px) else { return };
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let ordinal = self.bounds.math.iter().position(|m| *m == range);
        let Some(ordinal) = ordinal.filter(|_| size.x > window.width() + 0.5) else {
            let at = Rect::from_center_size(window.center(), size);
            ui.painter()
                .add(RectShape::filled(at, 0.0, Color32::WHITE).with_texture(texture, uv));
            return;
        };
        let key = RowScroller::Math(ordinal);
        let clip = window.intersect(ui.clip_rect());
        let offset = if self.painting_drag_float {
            self.row_scrolled(key)
        } else {
            self.row_scrollers.push((clip, key));
            self.row_scroll(ui, key, window, size.x, |_, _, _| None)
        };
        let at = Rect::from_min_size(window.min - vec2(offset, 0.0), size);
        ui.painter()
            .with_clip_rect(clip)
            .add(RectShape::filled(at, 0.0, Color32::WHITE).with_texture(texture, uv));
        let bg = self.ctx.get_lb_theme().neutral_bg();
        let bands = [(window.top(), window.bottom(), bg)];
        self.row_scroll_chrome(ui, key, window, clip, size.x, &bands);
    }

    /// Recompute [`crate::tab::markdown_editor::bounds::Bounds::math`].
    /// Depends on text only.
    pub fn calc_math_bounds<'a>(&mut self, root: &'a AstNode<'a>) {
        let mut math: Vec<_> = root
            .descendants()
            .filter(|node| matches!(node.data.borrow().value, NodeValue::Math(_)))
            .map(|node| self.node_range(node))
            .collect();
        math.sort_unstable();
        // scroll offsets go by a formula's place among the formulas
        if math.len() != self.bounds.math.len() {
            self.row_scrolls
                .retain(|key, _| !matches!(key, RowScroller::Math(_)));
        }
        self.bounds.math = math;
    }
}

#[derive(Clone)]
enum MathMenu {
    Open,
    CopyImage,
    Copy,
    Edit,
}

impl<'ast> MdEdit {
    /// A tap anywhere on a formula selects it, as one on a diagram does, and
    /// on touch pops the edit menu, whose Edit reveals its source
    /// ([`Self::enter_at_math`]); a cmd-click opens it, as any click does
    /// read-only, and on desktop right-click shows the menu.
    pub fn handle_math_interactions(
        &mut self, root: &'ast AstNode<'ast>, ui: &Ui, id: egui::Id, ops: &mut Vec<Operation>,
    ) {
        let open = self.embed_tap_opens(ui);
        for node in root.descendants() {
            let Some(formula) = MdRender::formula(node) else { continue };
            let range = self.renderer.node_range(node);
            let salt = MdRender::math_interaction_id_salt(range);
            let Some(response) = self
                .renderer
                .interaction_responses
                .get(&ui.scope_id().with(salt))
                .cloned()
            else {
                continue;
            };
            let select = Event::Select { region: range.into() };
            self.renderer.touch_consume_interaction(
                ui.scope_id().with(salt),
                TouchTarget::Tap(select.clone()),
            );
            if open && response.hovered() {
                crate::style::link_cursor(ui.ctx());
            }
            if response.clicked() && open {
                ui.ctx().open_picture(Destination::Formula(formula.clone()));
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
                e.item(phosphor::ARROW_SQUARE_OUT, "Open", MathMenu::Open);
                e.item(phosphor::IMAGE, "Copy Image", MathMenu::CopyImage);
                e.item(phosphor::COPY, "Copy", MathMenu::Copy);
                if editable {
                    e.item(phosphor::PENCIL, "Edit Formula", MathMenu::Edit);
                }
            });
            match action {
                Some(MathMenu::Open) => ui.ctx().open_picture(Destination::Formula(formula)),
                Some(MathMenu::CopyImage) => self.renderer.copy_picture(Picture::Formula(formula)),
                Some(MathMenu::Copy) => ui.ctx().copy_text(self.renderer.buffer[range].into()),
                Some(MathMenu::Edit) => {
                    ui.memory_mut(|m| m.request_focus(id));
                    let caret = self.math_edit_caret(node);
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

    /// `Event::EnterAtom` on a selected formula puts the caret in its source,
    /// revealing it. True if handled.
    pub fn enter_at_math(
        &self, root: &'ast AstNode<'ast>, operations: &mut Vec<Operation>,
    ) -> bool {
        let selection = self.renderer.buffer.current.selection;
        let Some(node) = root.descendants().find(|node| {
            matches!(node.data.borrow().value, NodeValue::Math(_))
                && self.renderer.node_range(node) == selection
                && !self.renderer.range_revealed_interior(selection)
        }) else {
            return false;
        };
        operations.push(Operation::Select(self.math_edit_caret(node)));
        true
    }

    /// The end of a formula's source, inside its closing delimiter.
    fn math_edit_caret(&self, node: &'ast AstNode<'ast>) -> (Grapheme, Grapheme) {
        let NodeValue::Math(math) = &node.data.borrow().value else { unreachable!() };
        // `$$…$$` and `$`…`$` close with two graphemes, `$…$` with one
        let close = if math.display_math || !math.dollar_math { 2 } else { 1 };
        let end = self.renderer.node_range(node).end() - close;
        (end, end)
    }
}
