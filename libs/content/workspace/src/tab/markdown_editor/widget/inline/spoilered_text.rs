use comrak::nodes::{AstNode, NodeValue};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};
use crate::theme::palette_v2::ThemeExt as _;
use lb_rs::model::text::offset_types::Grapheme;

impl<'ast> MdRender {
    pub fn text_format_spoilered_text(&self, node: &AstNode<'_>, parent: &AstNode<'_>) -> Format {
        let parent_text_format = self.text_format(parent);
        let theme = self.ctx.get_lb_theme();
        let bg = theme.neutral_bg_secondary();
        // Reveal when the user tapped this spoiler OR the cursor /
        // selection sits on it (so editing within reveals automatically).
        // `sourcepos_to_range` matches `node_range` for spoilers
        // (no node_range hacks apply to this NodeValue).
        let node_range = self.sourcepos_to_range(node.data.borrow().sourcepos);
        let revealed = self
            .revealed_spoilers
            .contains(&Self::spoiler_interaction_id_salt(node))
            || self.range_revealed(node_range, true);
        Format {
            background: bg,
            // Hidden state: glyphs blend into the chip. Glyphon caches
            // by colour, so changing this also reshapes the buffer.
            color: if revealed { parent_text_format.color } else { bg },
            spoiler: true,
            ..parent_text_format
        }
    }

    /// Sourcepos-keyed so the salt is reachable from `text_format`
    /// dispatch (which doesn't carry the `'ast` lifetime needed by
    /// `node_range`).
    pub fn spoiler_interaction_id_salt(node: &AstNode<'_>) -> egui::Id {
        let sp = node.data.borrow().sourcepos;
        egui::Id::new(("md_spoiler", sp.start.line, sp.start.column, sp.end.line, sp.end.column))
    }

    pub fn layout_spoilered_text(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let parent = node.parent().unwrap();
        let salt = Self::spoiler_interaction_id_salt(node);
        let fmt = self.text_format_spoilered_text(node, parent);

        layout.interaction_open(salt, egui::Sense::click());
        self.layout_circumfix(layout, node, range, fmt);
        layout.interaction_close();
    }

    /// Toggle `revealed_spoilers` when a spoiler chip is clicked.
    /// Must run after `interact_fragments`.
    pub fn handle_spoiler_interactions(&mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui) {
        let parent_base = ui.id();
        for node in root.descendants() {
            if !matches!(node.data.borrow().value, NodeValue::SpoileredText) {
                continue;
            }
            let salt = Self::spoiler_interaction_id_salt(node);
            let id = parent_base.with(salt);
            let Some(response) = self.interaction_responses.get(&id) else {
                continue;
            };

            // iOS routes touches through `touch_consuming_rects` —
            // without this entry a tap on a spoiler would place the
            // cursor instead of reaching the toggle handler below.
            self.touch_consuming_rects.push(response.rect);

            if response.hovered() {
                ui.ctx()
                    .output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }
            if response.clicked() && !self.revealed_spoilers.insert(salt) {
                self.revealed_spoilers.remove(&salt);
            }
        }
    }
}
