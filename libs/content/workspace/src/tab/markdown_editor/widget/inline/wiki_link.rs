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
        let fmt = self.text_format_link(node.parent().unwrap(), self.link_state_for_wikilink(&url));
        let cmd = self.ctx.input(|i| i.modifiers.command);
        if cmd {
            let salt = Self::link_interaction_id_salt(self.node_range(node));
            layout.interaction_open(salt, egui::Sense::CLICK);
        }
        self.layout_circumfix(layout, node, range, fmt);
        if cmd {
            layout.interaction_close();
        }
    }
}
