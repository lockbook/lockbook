use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use lb_rs::spawn;
use scraper::{Html, Selector};
use std::collections::hash_map::Entry;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use crate::file_cache::{FilesExt as _, ResolvedLink};
use crate::show::DocType;
use crate::tab::ExtendedOutput as _;
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::block::TitleState;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};
use crate::theme::palette_v2::ThemeExt as _;

enum DestinationTitle {
    Ready(String),
    Absent,
}

pub use crate::resolvers::LinkState;

impl<'ast> MdRender {
    pub fn text_format_link(&self, parent: &AstNode<'_>, state: LinkState) -> Format {
        let parent_text_format = self.text_format(parent);
        let theme = self.ctx.get_lb_theme();
        let color = match state {
            LinkState::Normal => theme.fg().blue,
            LinkState::Warning { .. } => theme.fg().yellow,
            LinkState::Broken { .. } => theme.fg().red,
        };
        Format { color, underline: true, ..parent_text_format }
    }

    fn link_is_auto(&self, node: &'ast AstNode<'ast>, url: &str) -> bool {
        self.infix_range(node)
            .is_some_and(|r| &self.buffer[r] == url)
    }

    /// Emit a link as a normal circumfix (children styled with link
    /// format, prefix/postfix syntax revealed by cursor). For empty
    /// or auto links with a fetched title, swap the URL bytes for
    /// the fetched title via `push_override`. Sense routing (click
    /// to open, touch-mode "open in new tab" button) lives in the
    /// fragment paint / hit-test layer, not in walker emit.
    pub fn layout_link(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node);
        let url = node_link_url(node);
        let is_auto = self.link_is_auto(node, &url);
        let parent = node.parent().unwrap();
        let link_fmt = self.text_format_link(parent, self.link_state_for_url(&url));
        let revealed = self.range_revealed(node_range, is_auto);

        // Empty link with title fetched: replace URL bytes with title.
        if (node.children().next().is_none() || is_auto) && !revealed {
            let trimmed = node_range.trim(&range);
            if !trimmed.is_empty() {
                if let DestinationTitle::Ready(t) = self.get_link_title(&url) {
                    layout.push_override(trimmed, &t, link_fmt);
                    return;
                }
            }
        }
        // Otherwise: emit as a circumfix with link format.
        self.layout_circumfix(layout, node, range, link_fmt);
    }

    pub fn resolve_link(&self, url: &str) -> Option<ResolvedLink> {
        self.link_resolver.resolve_link(url)
    }

    pub fn link_state_for_url(&self, url: &str) -> LinkState {
        self.link_resolver.link_state(url)
    }

    pub fn link_state_for_wikilink(&self, url: &str) -> LinkState {
        self.link_resolver.wikilink_state(url)
    }

    pub fn open_links_in_selection(&self, root: &'ast AstNode<'ast>, ctx: &egui::Context) {
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

    // Resolves the display title for a link with empty text.
    // Internal links (lb:// or relative paths) resolve synchronously from the file cache.
    // External http/https links are fetched asynchronously; returns Absent until
    // the fetch completes (caller renders the original URL text in that case).
    fn get_link_title(&self, url: &str) -> DestinationTitle {
        let Some(resolved) = self.resolve_link(url) else {
            return DestinationTitle::Absent;
        };

        let resolved_url = match resolved {
            ResolvedLink::File(id) => {
                let guard = self.files.read().unwrap();
                let Some(file) = guard.get_by_id(id) else {
                    return DestinationTitle::Absent;
                };
                let title = DocType::from_name(&file.name).display_name(&file.name);
                return DestinationTitle::Ready(title.to_string());
            }
            ResolvedLink::External(url)
                if url.starts_with("http://") || url.starts_with("https://") =>
            {
                url
            }
            ResolvedLink::External(_) => return DestinationTitle::Absent,
        };

        let arc = match self
            .layout_cache
            .link_titles
            .borrow_mut()
            .entry(resolved_url.clone())
        {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let arc = Arc::new(Mutex::new(TitleState::Loading));
                e.insert(arc.clone());
                let client = self.client.clone();
                let ctx = self.ctx.clone();
                let title_state = arc.clone();
                let link_seq = self.layout_cache.link_seq.clone();
                let ws_seq = self.ws_seq.clone();
                spawn!({
                    const CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
                    const GOOGLEBOT: &str =
                        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

                    #[cfg(not(target_arch = "wasm32"))]
                    let mut html = fetch_html(&client, &resolved_url, CHROME);
                    #[cfg(target_arch = "wasm32")]
                    let mut html = fetch_html(&client, &resolved_url, CHROME).await;

                    // some sites (e.g. Twitter/X) only serve static content to known crawlers
                    if html.as_deref().ok().and_then(extract_html_title).is_none() {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            html = fetch_html(&client, &resolved_url, GOOGLEBOT);
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            html = fetch_html(&client, &resolved_url, GOOGLEBOT).await;
                        }
                    }

                    *title_state.lock().unwrap() = html
                        .ok()
                        .and_then(|h| extract_html_title(&h))
                        .map(TitleState::Loaded)
                        .unwrap_or(TitleState::Failed);
                    link_seq.store(ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
                    ctx.request_repaint();
                });
                arc
            }
        };

        let state = arc.lock().unwrap();
        match &*state {
            TitleState::Loaded(t) => DestinationTitle::Ready(t.clone()),
            TitleState::Loading | TitleState::Failed => DestinationTitle::Absent,
        }
    }
}

fn node_link_url(node: &AstNode<'_>) -> String {
    use comrak::nodes::NodeValue;
    match &node.data.borrow().value {
        NodeValue::Link(link) => link.url.clone(),
        _ => String::new(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_html(
    client: &crate::tab::markdown_editor::HttpClient, url: &str, user_agent: &str,
) -> Result<String, String> {
    client
        .get(url)
        .header("User-Agent", user_agent)
        .send()
        .and_then(|r| r.text())
        .map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
async fn fetch_html(
    client: &crate::tab::markdown_editor::HttpClient, url: &str, user_agent: &str,
) -> Result<String, String> {
    client
        .get(url)
        .header("User-Agent", user_agent)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

fn extract_html_title(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);

    let title_sel = Selector::parse("title").ok()?;
    let title = doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>());
    if let Some(t) = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
    {
        return Some(t);
    }

    // static / server rendered properties designed to support this use case for JS pages
    let meta_sel = Selector::parse("meta[property='og:title'], meta[name='twitter:title']").ok()?;
    let title = doc
        .select(&meta_sel)
        .find_map(|e| e.value().attr("content"))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())?;
    Some(title)
}
