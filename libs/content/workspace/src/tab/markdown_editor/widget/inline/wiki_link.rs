use comrak::nodes::AstNode;
use lb_rs::Uuid;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::resolvers::link::LinkState;
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Layout;
use crate::theme::icons::Icon;

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
        let fmt = self.text_format_link(parent, state.clone());
        let cmd = self.readonly || self.ctx.input(|i| i.modifiers.command);
        let salt = Self::link_interaction_id_salt(node_range);
        if cmd {
            layout.interaction_open(salt, egui::Sense::click());
        }
        self.layout_circumfix(layout, node, range, fmt);
        if cmd {
            layout.interaction_close();
        }

        let broken = matches!(state, LinkState::Broken { .. });
        if self.touch_mode && !broken && range.contains_inclusive(node_range.end()) {
            let anchor = (node_range.end(), node_range.end());
            let parent_fmt = self.text_format(parent);
            layout.push_override(anchor, " ", parent_fmt);
            layout.interaction_open(salt, egui::Sense::click());
            layout.push_override(
                anchor,
                Icon::OPEN_IN_NEW.icon,
                self.text_format_link_button(parent, state),
            );
            layout.interaction_close();
        }
    }
}
