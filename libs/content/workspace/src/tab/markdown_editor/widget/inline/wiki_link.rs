use comrak::nodes::{AstNode, NodeValue, NodeWikiLink};
use egui::{OpenUrl, Pos2, Ui};
use lb_rs::Uuid;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _};

use crate::file_cache::FileCache;
use crate::tab::core_get_by_relative_path;
use crate::tab::core_get_relative_path;
use crate::tab::markdown_editor::Editor;
use crate::tab::ExtendedOutput as _;
use crate::tab::markdown_editor::widget::inline::Response;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Wrap};
use crate::theme::palette_v2::ThemeExt as _;

impl Editor {
    pub fn is_broken_internal_link(&self, url: &str) -> bool {
        if url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("mailto:")
            || url.starts_with('#')
        {
            return false;
        }
        if let Some(&cached) = self.layout_cache.broken_links.borrow().get(url) {
            return cached;
        }
        let from_id = self
            .core
            .get_file_by_id(self.file_id)
            .map(|f| f.parent)
            .unwrap_or(self.file_id);
        let broken = core_get_by_relative_path(&self.core, from_id, url).is_err();
        self.layout_cache
            .broken_links
            .borrow_mut()
            .insert(url.to_string(), broken);
        broken
    }

    pub fn is_wikilink_broken(&self, url: &str) -> bool {
        self.resolve_wikilink(url).is_none()
    }

    pub fn resolve_wikilink(&self, url: &str) -> Option<Uuid> {
        let from_id = self
            .core
            .get_file_by_id(self.file_id)
            .map(|f| f.parent)
            .unwrap_or(self.file_id);

        // Disambiguation paths contain a slash. Re-add .md and resolve directly
        // from the parent folder — the same origin used when the path was built.
        if url.contains('/') {
            let with_ext =
                if url.ends_with(".md") { url.to_string() } else { format!("{}.md", url) };
            if let Ok(file) = core_get_by_relative_path(&self.core, from_id, &with_ext) {
                return Some(file.id);
            }
        }

        let guard = self.files.read().unwrap();
        let FileCache { files, .. } = guard.as_ref()?;

        let title_bare = url
            .rsplit('/')
            .next()
            .unwrap_or(url)
            .trim_end_matches(".md");

        let candidates: Vec<_> = files
            .iter()
            .filter(|f| f.is_document())
            .filter(|f| {
                f.name
                    .trim_end_matches(".md")
                    .eq_ignore_ascii_case(title_bare)
            })
            .collect();

        match candidates.len() {
            0 => None,
            1 => Some(candidates[0].id),
            _ => candidates
                .iter()
                .min_by_key(|f| {
                    core_get_relative_path(&self.core, from_id, f.id)
                        .matches("../")
                        .count()
                })
                .map(|f| f.id),
        }
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

            if !url.starts_with("http://")
                && !url.starts_with("https://")
                && !url.starts_with("mailto:")
                && !url.starts_with('#')
            {
                let from_id = self
                    .core
                    .get_file_by_id(self.file_id)
                    .map(|f| f.parent)
                    .unwrap_or(self.file_id);
                if let Ok(file) = core_get_by_relative_path(&self.core, from_id, &url) {
                    file_ids.push(file.id);
                    continue;
                }
            }

            urls.push(egui::OpenUrl { url: url.to_string(), new_tab: false });
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

impl<'ast> Editor {
    pub fn text_format_wiki_link(&self, parent: &AstNode<'_>, url: &str) -> Format {
        let base = self.text_format_link(parent);
        if self.is_wikilink_broken(url) {
            let theme = self.ctx.get_lb_theme();
            Format { color: theme.fg().red, ..base }
        } else {
            base
        }
    }

    pub fn text_format_internal_link(&self, parent: &AstNode<'_>, url: &str) -> Format {
        let base = self.text_format_link(parent);
        if self.is_broken_internal_link(url) {
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
