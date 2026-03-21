use comrak::nodes::{AstNode, NodeWikiLink};
use egui::{OpenUrl, Pos2, Ui};
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::file_cache::FilesExt as _;
use crate::tab::markdown_editor::Editor;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Wrap};
use crate::theme::palette_v2::ThemeExt as _;

impl Editor {
    pub fn resolve_link(&self, url: &str) -> Option<String> {
        let guard = self.files.read().unwrap();
        let cache = guard.as_ref()?;
        let from_id = cache.files.get_by_id(self.file_id)?.parent;
        cache.files.resolve_link(url, from_id)
    }

    pub fn resolve_wikilink(&self, title: &str) -> Option<String> {
        let guard = self.files.read().unwrap();
        let cache = guard.as_ref()?;
        let from_id = cache.files.get_by_id(self.file_id)?.parent;
        cache.files.resolve_wikilink(title, from_id)
    }
}

impl<'ast> Editor {
    pub fn text_format_wiki_link(&self, parent: &AstNode<'_>, url: &str) -> Format {
        let base = self.text_format_link(parent);
        if self.resolve_wikilink(url).is_none() {
            let theme = self.ctx.get_lb_theme();
            Format { color: theme.fg().red, ..base }
        } else {
            base
        }
    }

    pub fn text_format_internal_link(&self, parent: &AstNode<'_>, url: &str) -> Format {
        let base = self.text_format_link(parent);
        if self.resolve_link(url).is_none() {
            let theme = self.ctx.get_lb_theme();
            Format { color: theme.fg().red, ..base }
        } else {
            base
        }
    }

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
            let url = self
                .resolve_wikilink(&node_wiki_link.url)
                .unwrap_or_else(|| node_wiki_link.url.clone());
            ui.ctx().open_url(OpenUrl { url, new_tab: cmd });
        }

        response
    }
}
