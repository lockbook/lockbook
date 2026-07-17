use comrak::nodes::AstNode;
use lb_rs::Uuid;
use lb_rs::model::text::offset_types::Grapheme;

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;

impl<'ast> MdRender {
    pub fn resolve_wikilink(&self, url: &str) -> Option<Uuid> {
        self.link_resolver.resolve_wikilink(url)
    }

    pub fn layout_wikilink(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let url = match &node.data.borrow().value {
            comrak::nodes::NodeValue::WikiLink(nwl) => nwl.url.clone(),
            _ => String::new(),
        };
        let parent = node.parent().unwrap();
        let node_range = self.node_range(node);
        let state = self.link_state_for_wikilink(&url);
        let fmt = self.text_format_link(parent, state);
        // Read-only views have no cursor to place, so a plain click follows
        // the link; the editor requires cmd, and touch taps select the link
        // and pop the menu — unless revealed, when taps place the cursor.
        let revealed = self.range_revealed_interior(node_range);
        let clickable = self.readonly
            || self.ctx.input(|i| i.modifiers.command)
            || (self.touch_mode && !revealed);
        let salt = Self::link_interaction_id_salt(node_range);
        if clickable {
            layout.interaction_open(salt, egui::Sense::click());
        }
        self.layout_circumfix(layout, node, range, fmt);
        if clickable {
            layout.interaction_close();
        }
    }
}
