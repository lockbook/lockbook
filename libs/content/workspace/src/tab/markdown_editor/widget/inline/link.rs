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
use crate::tab::markdown_editor::widget::utils::wrap_layout::{
    FontFamily, Format, Layout, StyleInfo,
};
use crate::theme::icons::Icon;
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

    /// `Icon` glyph (no underline) used for the touch-mode "open link"
    /// affordance appended after each link. Coloured to match the link's
    /// state so the button doesn't read as healthy-blue on a warning link.
    pub fn text_format_link_button(&self, parent: &AstNode<'_>, state: LinkState) -> Format {
        Format {
            family: FontFamily::Icons,
            underline: false,
            ..self.text_format_link(parent, state)
        }
    }

    fn link_is_auto(&self, node: &'ast AstNode<'ast>, url: &str) -> bool {
        self.infix_range(node)
            .is_some_and(|r| &self.buffer[r] == url)
    }

    /// Shared by producer + consumer so `ui.id().with(salt)` resolves
    /// to the same id on both sides.
    pub fn link_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::new(("md_link", node_range))
    }

    /// Emit a link as a circumfix. For autolinks with a fetched title,
    /// swap the URL bytes for the title via `push_override`. Empty-text
    /// links (`[](url)`) are not autolinks and have nothing to show, so
    /// they render their raw source like other incomplete syntax.
    /// With cmd held, opens a `Sense::click` interaction scope so egui
    /// z-order routes cmd-click here; without cmd no scope is opened
    /// and clicks fall through to the editor.
    pub fn layout_link(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node);
        let url = node_link_url(node);
        let is_auto = self.link_is_auto(node, &url);
        let parent = node.parent().unwrap();
        let state = self.link_state_for_url(&url);
        let link_fmt = self.text_format_link(parent, state.clone());
        let revealed = self.range_revealed(node_range, is_auto);

        let cmd = self.ctx.input(|i| i.modifiers.command);
        let salt = Self::link_interaction_id_salt(node_range);
        if cmd {
            layout.interaction_open(salt, egui::Sense::click());
        }

        let trimmed = node_range.trim(&range);
        let title = if is_auto && !revealed && !trimmed.is_empty() {
            match self.get_link_title(&url) {
                DestinationTitle::Ready(t) => Some(t),
                DestinationTitle::Absent => None,
            }
        } else {
            None
        };
        match title {
            Some(t) => {
                layout.style_open(StyleInfo::new(link_fmt.clone(), node_range));
                layout.push_override(trimmed, &t, link_fmt.clone());
                layout.style_close();
            }
            None => self.layout_circumfix(layout, node, range, link_fmt.clone()),
        }

        if cmd {
            layout.interaction_close();
        }

        // Touch-mode open-link affordance: tap the trailing icon to open
        // the link (no cmd modifier on mobile). Only emit on the row that
        // contains the link's end. Broken links have nothing to open, so
        // they get no button.
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

    /// Hover → `PointingHand` + Warning/Broken tooltip; click → open
    /// in a new tab. The producer only opens an interaction scope when
    /// cmd is held (desktop) or for the trailing open-link affordance
    /// (touch); the response's presence is the gate.
    pub fn handle_link_interactions(&mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui) {
        let parent_base = ui.id();
        for node in root.descendants() {
            let (url, is_wikilink) = match &node.data.borrow().value {
                NodeValue::WikiLink(nwl) => (nwl.url.clone(), true),
                NodeValue::Link(nl) | NodeValue::Image(nl) => (nl.url.clone(), false),
                _ => continue,
            };
            let id = parent_base.with(Self::link_interaction_id_salt(self.node_range(node)));
            let Some(response) = self.interaction_responses.get(&id) else {
                continue;
            };

            // iOS routes touches through `touch_consuming_rects` —
            // without this entry a tap on the open-link button would
            // place the cursor instead of reaching the click handler below.
            self.touch_consuming_rects.push(response.rect);

            if response.hovered() {
                ui.ctx()
                    .output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);

                let state = if is_wikilink {
                    self.link_state_for_wikilink(&url)
                } else {
                    self.link_state_for_url(&url)
                };
                if let LinkState::Warning { message } | LinkState::Broken { message } = &state {
                    if let Some(pos) = ui.ctx().pointer_hover_pos() {
                        egui::Area::new(id.with("link_warning"))
                            .order(egui::Order::Tooltip)
                            .fixed_pos(pos + egui::vec2(8.0, 16.0))
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    ui.label(message);
                                });
                            });
                    }
                }
            }

            if response.clicked() {
                if is_wikilink {
                    if let Some(file_id) = self.resolve_wikilink(&url) {
                        ui.ctx().open_file(file_id, true);
                    }
                } else {
                    match self.resolve_link(&url) {
                        Some(ResolvedLink::File(file_id)) => ui.ctx().open_file(file_id, true),
                        Some(ResolvedLink::External(target)) => ui
                            .ctx()
                            .open_url(egui::OpenUrl { url: target, new_tab: true }),
                        None => ui
                            .ctx()
                            .open_url(egui::OpenUrl { url: url.clone(), new_tab: true }),
                    }
                }
                return;
            }
        }
    }

    // Resolves the display title an autolink swaps in for its URL.
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
        NodeValue::Link(link) | NodeValue::Image(link) => link.url.clone(),
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
