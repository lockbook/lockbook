use comrak::nodes::{AstNode, NodeWikiLink};
use egui::{OpenUrl, Pos2, Ui};
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::resolvers::LinkState;
use crate::resolvers::{EmbedResolver, LinkResolver};
use crate::tab::ExtendedOutput as _;
use crate::tab::markdown_editor::MdLabel;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast, E: EmbedResolver, L: LinkResolver> MdLabel<E, L> {
    pub fn span_wikilink(
        &self, node: &'ast AstNode<'ast>, wrap: &Wrap, range: (DocCharOffset, DocCharOffset),
    ) -> f32 {
        self.circumfix_span(node, wrap, range)
    }

    pub fn show_wikilink(
        &mut self, ui: &mut Ui, node: &'ast AstNode<'ast>, top_left: Pos2, wrap: &mut Wrap,
        node_wiki_link: &NodeWikiLink, range: (DocCharOffset, DocCharOffset),
    ) -> Response {
        let response = self.show_circumfix(ui, node, top_left, wrap, range);

        if response.hovered && self.inline_clickable(ui, node) {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            if self.link_resolver.wikilink_state(&node_wiki_link.url) == LinkState::Warning {
                if let Some(pos) = ui.ctx().pointer_hover_pos() {
                    egui::Area::new(ui.id().with("wikilink_warning"))
                        .order(egui::Order::Tooltip)
                        .fixed_pos(pos + egui::vec2(8.0, 16.0))
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.label("Some collaborators cannot access this link target");
                            });
                        });
                }
            }
        }
        if response.clicked {
            let cmd = ui.input(|i| i.modifiers.command);
            if let Some(id) = self.link_resolver.resolve_wikilink(&node_wiki_link.url) {
                ui.ctx().open_file(id, false);
            } else {
                ui.ctx()
                    .open_url(OpenUrl { url: node_wiki_link.url.clone(), new_tab: cmd });
            }
        }

        response
    }
}
