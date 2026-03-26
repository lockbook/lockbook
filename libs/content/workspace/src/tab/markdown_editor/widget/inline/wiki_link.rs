use comrak::nodes::{AstNode, NodeValue, NodeWikiLink};
use egui::{OpenUrl, Pos2, Ui};
use lb_rs::Uuid;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::file_cache::{FilesExt as _, ResolvedLink};
use crate::tab::markdown_editor::Editor;
use crate::tab::ExtendedOutput as _;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::Wrap;

impl<'ast> Editor {

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
        }
        if response.clicked {
            let cmd = ui.input(|i| i.modifiers.command);
            if let Some(id) = self.resolve_wikilink(&node_wiki_link.url) {
                ui.ctx().open_file(id, false);
            } else {
                ui.ctx()
                    .open_url(OpenUrl { url: node_wiki_link.url.clone(), new_tab: cmd });
            }
        }

        response
    }
}

impl Editor {
    pub fn resolve_wikilink(&self, url: &str) -> Option<Uuid> {
        let guard = self.files.read().unwrap();
        let cache = guard.as_ref()?;
        let from_id = cache.files.get_by_id(self.file_id)?.parent;
        cache.files.resolve_wikilink(url, from_id)
    }

    pub fn open_links_in_selection<'ast>(
        &self, root: &'ast AstNode<'ast>, ctx: &egui::Context,
    ) {
        let selection = self.buffer.current.selection;

        let mut file_ids = vec![];
        let mut urls = vec![];

        for node in root.descendants() {
            let node_range = self.node_range(node);
            if !node_range.intersects(&selection, true) {
                continue;
            }

            let (url, is_wikilink) = {
                let data = node.data.borrow();
                match &data.value {
                    NodeValue::WikiLink(nwl) => (nwl.url.clone(), true),
                    NodeValue::Link(nl) => (nl.url.clone(), false),
                    NodeValue::Image(ni) => (ni.url.clone(), false),
                    _ => continue,
                }
            };

            if is_wikilink {
                if let Some(id) = self.resolve_wikilink(&url) {
                    file_ids.push(id);
                }
                continue;
            }

            match self.resolve_link(&url) {
                Some(ResolvedLink::File(id)) => file_ids.push(id),
                Some(ResolvedLink::External(url)) => {
                    urls.push(egui::OpenUrl { url, new_tab: false });
                }
                None => {
                    urls.push(egui::OpenUrl { url, new_tab: false });
                }
            }
        }

        let new_tab = file_ids.len() + urls.len() > 1;
        for id in file_ids {
            ctx.open_file(id, new_tab);
        }
        if new_tab {
            for url in &mut urls {
                url.new_tab = true;
            }
        }
        for url in urls {
            ctx.open_url(url);
        }
    }
}
