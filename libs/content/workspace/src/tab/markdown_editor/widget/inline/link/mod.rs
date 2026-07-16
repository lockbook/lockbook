pub(crate) mod card;
pub(crate) mod meta;

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use lb_rs::spawn;
use std::collections::hash_map::Entry;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use lb_rs::model::text::operation_types::Operation;

use crate::egress::{FetchError, fetch_html};
use crate::file_cache::{FilesExt as _, ResolvedLink};
use crate::show::DocType;
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::inline::link::meta::{
    LinkMeta, LinkMetaState, extract_link_meta, is_junk_meta,
};
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout, StyleInfo};
use crate::tab::markdown_editor::{MdEdit, MdRender};
use crate::tab::{ContextMenuTarget, ExtendedOutput as _};
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

    /// Capsule "chip" format for a resolved autolink preview: the link colour on
    /// a subtle surface, no underline (the pill is the affordance). Paired with
    /// `StyleInfo { chip: true }`, the wrap layout paints the rounded capsule.
    pub fn text_format_link_pill(&self, parent: &AstNode<'_>, state: LinkState) -> Format {
        Format {
            background: self.ctx.get_lb_theme().neutral_bg_secondary(),
            underline: false,
            ..self.text_format_link(parent, state)
        }
    }

    fn link_is_auto(&self, node: &'ast AstNode<'ast>, url: &str) -> bool {
        self.infix_range(node)
            .is_some_and(|r| &self.buffer[r] == url)
    }

    /// Whether this link should render as a block **card** (its own row) rather
    /// than an inline link. The trigger is positional so the source stays clean,
    /// portable markdown: a **bare autolink** (`https://x.com`, not `[label](url)`
    /// and not the delimited `<url>` form) that is **alone on its own line**
    /// (within a paragraph, bounded by line breaks or the paragraph edges) and
    /// **not inside a container block** (list item / quote / alert / table /
    /// footnote).
    pub fn link_renders_as_card(&self, node: &'ast AstNode<'ast>) -> bool {
        let url = node_link_url(node);
        if url.is_empty() || !self.link_is_auto(node, &url) {
            return false; // labeled `[text](url)` and non-links never card
        }
        // `<url>` (delimited autolink) opts out — only bare URLs card.
        if self.buffer[self.node_range(node)].starts_with('<') {
            return false;
        }

        let Some(parent) = node.parent() else {
            return false;
        };
        if !matches!(parent.data.borrow().value, NodeValue::Paragraph) {
            return false;
        }
        // must be the only meaningful inline on its line — scan out to a line
        // break (or the paragraph edge) on each side, allowing only blank text.
        if !alone_on_line(node.previous_sibling(), |n| n.previous_sibling())
            || !alone_on_line(node.next_sibling(), |n| n.next_sibling())
        {
            return false;
        }

        // not inside a container block (Document itself doesn't count)
        let mut ancestor = parent.parent();
        while let Some(a) = ancestor {
            let value = &a.data.borrow().value;
            if !matches!(value, NodeValue::Document) && value.is_container_block() {
                return false;
            }
            ancestor = a.parent();
        }
        true
    }

    /// Shared by producer + consumer so `ui.id().with(salt)` resolves
    /// to the same id on both sides.
    pub fn link_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::new(("md_link", node_range))
    }

    /// Click-target id for a link rendered as a capsule preview; distinct from
    /// the plain-link salt so its embed-style tap handling doesn't collide.
    pub fn link_capsule_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::new(("md_link_capsule", node_range))
    }

    /// Emit a link as a circumfix. For autolinks with a fetched title,
    /// swap the URL bytes for the title via `push_override`. Empty-text
    /// links (`[](url)`) are not autolinks and have nothing to show, so
    /// they render their raw source like other incomplete syntax.
    /// A `Sense::click` interaction scope routes clicks here when they
    /// act on the link (cmd held, read-only, or unrevealed touch);
    /// otherwise no scope is opened and clicks fall through to the
    /// editor for cursor placement.
    pub fn layout_link(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node);
        let url = node_link_url(node);
        let is_auto = self.link_is_auto(node, &url);
        let parent = node.parent().unwrap();
        let state = self.link_state_for_url(&url);
        let link_fmt = self.text_format_link(parent, state.clone());
        // Interior-only: a cursor inside reveals raw markdown, but bordering or
        // selecting the link keeps its rendered (preview / capsule) form.
        let revealed = self.range_revealed_interior(node_range);

        // Block link-preview card on its own row. Interior-only reveal (like an
        // image): bordering keeps the card; a cursor inside falls through to the
        // editable inline link.
        let single_line = range.contains_range(&node_range, true, true);
        if single_line && !revealed && self.layout_link_card(layout, node, &url) {
            return;
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

        // Resolved autolink preview → a "chip" capsule (favicon + title, no
        // underline) that behaves like the block card / inline image: tap selects,
        // cmd/keyboard-tap opens (`handle_link_capsule_interactions`), interior
        // reveal only.
        if let Some(t) = title {
            let pill_fmt = self.text_format_link_pill(parent, state.clone());
            layout.interaction_open(Self::link_capsule_id_salt(node_range), egui::Sense::click());
            layout.style_open(StyleInfo {
                format: pill_fmt.clone(),
                source_range: node_range,
                chip: true,
            });
            // Leading em-space reserves the favicon slot (painted in
            // `show_capsule_overlays`); it's in the title's source range, so the
            // selection covers the favicon like text — no separate embed. The
            // WORD JOINER removes the break opportunity after the em space
            // (UAX#14 LB11) so the slot wraps with the title's first word
            // instead of orphaning at the end of a line.
            let text = if self.capsule_favicon_url(&url).is_some() {
                format!("\u{2003}\u{2060}{t}")
            } else {
                t
            };
            layout.push_override(trimmed, &text, pill_fmt);
            layout.style_close();
            layout.interaction_close();
            return;
        }

        // Plain inline link (labeled, or an autolink whose title hasn't
        // resolved): cmd-click opens (desktop editor); read-only views have
        // no cursor to place, so a plain click opens; touch taps select the
        // link and pop the menu — unless revealed, when taps place the
        // cursor in the raw source.
        let clickable = self.readonly
            || self.ctx.input(|i| i.modifiers.command)
            || (self.touch_mode && !revealed);
        let salt = Self::link_interaction_id_salt(node_range);
        if clickable {
            layout.interaction_open(salt, egui::Sense::click());
        }
        self.layout_circumfix(layout, node, range, link_fmt.clone());
        if clickable {
            layout.interaction_close();
        }
    }

    /// Favicon URL for a capsule preview's external link when previews are on
    /// (not gated on load) — reserves the leading em-space slot in the title.
    /// [`Self::show_link_favicons`] paints the icon over that slot once loaded.
    pub(crate) fn capsule_favicon_url(&self, url: &str) -> Option<String> {
        if !self.contact_linked_sites {
            return None;
        }
        match self.get_link_meta(url) {
            LinkMetaLookup::Loaded(meta) => meta.favicon_url,
            _ => None,
        }
    }

    /// Per-capsule overlays painted after content, before the selection/cursor
    /// pass: the favicon over the title's em-space slot, then a selection tint on
    /// top of a selected capsule (the opaque pill hides the under-text rect —
    /// like images/cards).
    pub fn show_capsule_overlays(
        &mut self, ui: &mut egui::Ui, root: &'ast AstNode<'ast>, viewport: egui::Rect,
    ) {
        // Clip favicon + tint to the editor viewport: this runs before
        // `post_render` installs its overlay clip, so a chip scrolled past the
        // top edge would otherwise bleed the favicon over the tab bar/toolbar.
        let entry_clip = ui.clip_rect();
        ui.set_clip_rect(viewport.intersect(entry_clip));

        let sel = self
            .in_progress_selection
            .unwrap_or(self.buffer.current.selection);
        for node in root.descendants() {
            let url = match &node.data.borrow().value {
                NodeValue::Link(l) => l.url.clone(),
                _ => continue,
            };
            let node_range = self.node_range(node);
            let chip_rects: Vec<egui::Rect> = self
                .fragments
                .iter()
                .filter(|f| {
                    f.source_range.intersects(&node_range, true)
                        && f.style_stack.last().is_some_and(|s| s.chip)
                })
                .map(|f| f.rect)
                .collect();
            if chip_rects.is_empty() {
                continue; // not rendered as a capsule
            }

            // Favicon over the leading em-space slot; a neutral link glyph
            // holds the slot while the texture loads — or forever, if it
            // never does — so the reserved space always reads as an icon.
            if let Some(favicon_url) = self.capsule_favicon_url(&url) {
                let chip = chip_rects[0];
                let size = chip.height() * 0.7;
                let inset = chip.height() * 0.3;
                let icon_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(chip.min.x + inset, chip.center().y - size * 0.5),
                    egui::Vec2::splat(size),
                );
                self.embeds.prefetch(&favicon_url);
                if self.embeds.is_loaded(&favicon_url) {
                    // Dark mode: a near-white tile behind the icon so dark
                    // monochrome marks (e.g. GitHub) read against the dark pill;
                    // opaque favicons simply hide it.
                    if self.dark_mode {
                        ui.painter()
                            .rect_filled(icon_rect, 3.0, egui::Color32::from_gray(235));
                    }
                    self.embeds
                        .show(ui, &favicon_url, icon_rect, egui::CornerRadius::same(3));
                } else {
                    ui.painter().text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        Icon::LINK.icon,
                        egui::FontId::monospace(size),
                        self.ctx.get_lb_theme().neutral_fg_secondary(),
                    );
                }
            }

            if !sel.is_empty() && sel.contains_range(&node_range, true, true) {
                let theme = self.ctx.get_lb_theme();
                let accent = theme.bg().get_color(theme.prefs().primary);
                let tint =
                    egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 90);
                // Merge fragments into one rect per row so the selection reads as
                // a single area (like a card), not a strip of small rects.
                let mut rows: Vec<egui::Rect> = Vec::new();
                for r in &chip_rects {
                    match rows.iter_mut().find(|q| (q.top() - r.top()).abs() < 0.5) {
                        Some(q) => *q = q.union(*r),
                        None => rows.push(*r),
                    }
                }
                for r in &rows {
                    ui.painter().rect_filled(*r, 2.0, tint);
                }
            }
        }

        ui.set_clip_rect(entry_clip);
    }

    pub fn resolve_link(&self, url: &str) -> Option<ResolvedLink> {
        self.link_resolver.resolve_link(url)
    }

    /// Open `url` in a new tab, navigating in-app for internal file links and
    /// to the browser otherwise. Shared by link and image interaction handlers.
    pub fn open_resolved_link(&self, url: &str, ctx: &egui::Context) {
        match self.resolve_link(url) {
            Some(ResolvedLink::File(file_id)) => ctx.open_file(file_id, true),
            Some(ResolvedLink::External(target)) => {
                ctx.open_url(egui::OpenUrl { url: target, new_tab: true })
            }
            None => ctx.open_url(egui::OpenUrl { url: url.into(), new_tab: true }),
        }
    }

    pub fn link_state_for_url(&self, url: &str) -> LinkState {
        self.link_resolver.link_state(url)
    }

    pub fn link_state_for_wikilink(&self, url: &str) -> LinkState {
        self.link_resolver.wikilink_state(url)
    }

    /// URL (or wikilink target) of the first link-like node whose source range
    /// intersects the current selection — gates and feeds the platform edit
    /// menu's "Open Link" / "Copy Link".
    pub fn selection_open_target(&mut self) -> Option<String> {
        let arena = Arena::new();
        let root = self.reparse(&arena);
        let selection = self.buffer.current.selection;
        for node in root.descendants() {
            let url = match &node.data.borrow().value {
                NodeValue::Link(l) | NodeValue::Image(l) => l.url.clone(),
                NodeValue::WikiLink(nwl) => nwl.url.clone(),
                _ => continue,
            };
            if !url.is_empty() && self.node_range(node).intersects(&selection, true) {
                return Some(url);
            }
        }
        None
    }

    /// Open every link/image in the current selection (reparses first); the iOS
    /// edit-menu "Open Link" action shares this with desktop Cmd+Enter.
    pub fn open_selection_links(&mut self) {
        let arena = Arena::new();
        let root = self.reparse(&arena);
        self.open_links_in_selection(root, &self.ctx);
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
    /// in a new tab. The producer's interaction scope is the gate: cmd
    /// held (desktop), read-only, or touch — where the click instead
    /// selects the link and pops the menu ([`MdEdit::handle_link_menu_taps`]).
    pub fn handle_link_interactions(&mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui) {
        let parent_base = ui.id();
        for node in root.descendants() {
            let (url, is_wikilink) = match &node.data.borrow().value {
                NodeValue::WikiLink(nwl) => (nwl.url.clone(), true),
                NodeValue::Link(nl) | NodeValue::Image(nl) => (nl.url.clone(), false),
                _ => continue,
            };
            let id = parent_base.with(Self::link_interaction_id_salt(self.node_range(node)));

            // iOS routes touches through `touch_consuming_rects` —
            // without these entries a tap on the open-link button would
            // place the cursor instead of reaching the click handler below.
            self.touch_consume_interaction(id);

            let Some(response) = self.interaction_responses.get(&id) else {
                continue;
            };

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

            if response.clicked() && (self.readonly || !self.touch_mode) {
                if is_wikilink {
                    if let Some(file_id) = self.resolve_wikilink(&url) {
                        ui.ctx().open_file(file_id, true);
                    }
                } else {
                    self.open_resolved_link(&url, ui.ctx());
                }
                return;
            }
        }
    }

    /// The title an autolink swaps in for its URL — a thin view over
    /// [`Self::get_link_meta`].
    fn get_link_title(&self, url: &str) -> DestinationTitle {
        match self.get_link_meta(url) {
            LinkMetaLookup::Internal(title) => DestinationTitle::Ready(title),
            LinkMetaLookup::Loaded(meta) => DestinationTitle::Ready(meta.title),
            LinkMetaLookup::Loading | LinkMetaLookup::Unavailable => DestinationTitle::Absent,
        }
    }

    /// Resolve a link's preview metadata. Internal links (lb:// / relative
    /// paths) resolve synchronously to a title from the file cache. External
    /// http(s) links are fetched asynchronously (gated by `contact_linked_sites`)
    /// and cached in [`LayoutCache::link_meta`]; `External(None)` until a fetch
    /// completes, the site is uncacheable, or fetching is off.
    pub fn get_link_meta(&self, url: &str) -> LinkMetaLookup {
        let Some(resolved) = self.resolve_link(url) else {
            return LinkMetaLookup::Unavailable;
        };

        let resolved_url = match resolved {
            ResolvedLink::File(id) => {
                let guard = self.files.read().unwrap();
                let Some(file) = guard.get_by_id(id) else {
                    return LinkMetaLookup::Unavailable;
                };
                let title = DocType::from_name(&file.name).display_name(&file.name);
                return LinkMetaLookup::Internal(title.to_string());
            }
            ResolvedLink::External(url)
                if url.starts_with("http://") || url.starts_with("https://") =>
            {
                url
            }
            ResolvedLink::External(_) => return LinkMetaLookup::Unavailable,
        };

        let now = web_time::Instant::now();
        let arc = match self
            .layout_cache
            .link_meta
            .borrow_mut()
            .entry(resolved_url.clone())
        {
            Entry::Occupied(mut e) => {
                // A failure whose next-retry timestamp has passed is eligible
                // again (still behind the fetch opt-in).
                let retry_attempts = match &*e.get().lock().unwrap() {
                    LinkMetaState::Failed { retry_at: Some(t), attempts } if *t <= now => {
                        Some(*attempts)
                    }
                    _ => None,
                };
                match retry_attempts {
                    Some(attempts) if self.contact_linked_sites => {
                        let arc = Arc::new(Mutex::new(LinkMetaState::Loading));
                        e.insert(arc.clone());
                        self.spawn_meta_fetch(resolved_url, arc.clone(), attempts);
                        arc
                    }
                    _ => e.get().clone(),
                }
            }
            Entry::Vacant(e) => {
                // Opt-out: don't contact the site unless preview fetching is on.
                // (Occupied/cached metadata above still displays — no new request.)
                if !self.contact_linked_sites {
                    return LinkMetaLookup::Unavailable;
                }
                let arc = Arc::new(Mutex::new(LinkMetaState::Loading));
                e.insert(arc.clone());
                self.spawn_meta_fetch(resolved_url, arc.clone(), 0);
                arc
            }
        };

        let state = arc.lock().unwrap();
        match &*state {
            LinkMetaState::Loaded(meta) => LinkMetaLookup::Loaded(meta.clone()),
            LinkMetaState::Loading => LinkMetaLookup::Loading,
            LinkMetaState::Failed { retry_at: Some(until), .. } if *until > now => {
                // Wake a frame when the cooldown lapses — an idle editor never
                // repaints, so without this the retry waits for user input.
                self.ctx.request_repaint_after(*until - now);
                LinkMetaLookup::Unavailable
            }
            LinkMetaState::Failed { .. } => LinkMetaLookup::Unavailable,
        }
    }

    /// Fetch + scrape `resolved_url`'s metadata into `meta_state` off-thread,
    /// then bump `link_seq` and request a repaint. `attempts` counts prior
    /// consecutive failures — it drives the transient-failure backoff.
    fn spawn_meta_fetch(
        &self, resolved_url: String, meta_state: Arc<Mutex<LinkMetaState>>, attempts: u32,
    ) {
        /// Self-assigned retry-after for network-level failures; doubles per
        /// consecutive failure up to the cap.
        const TRANSIENT_RETRY_BASE: std::time::Duration = std::time::Duration::from_secs(60);
        const TRANSIENT_RETRY_CAP: std::time::Duration = std::time::Duration::from_secs(15 * 60);
        /// Bot walls outlive short retries — a site's posture changes slowly.
        const BLOCKED_RETRY: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

        let client = self.client.clone();
        let ctx = self.ctx.clone();
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

            // Some sites (e.g. Twitter/X) only serve static content to known
            // crawlers, and bot walls often whitelist them — but never burn a
            // second request on a host that's rate-limiting us.
            let good = |h: &str| {
                extract_link_meta(h, &resolved_url)
                    .is_some_and(|m| !is_junk_meta(h, &m, &resolved_url))
            };
            if !matches!(html, Err(FetchError::RateLimited { .. }))
                && !html.as_deref().is_ok_and(good)
            {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    html = fetch_html(&client, &resolved_url, GOOGLEBOT);
                }
                #[cfg(target_arch = "wasm32")]
                {
                    html = fetch_html(&client, &resolved_url, GOOGLEBOT).await;
                }
            }

            let now = web_time::Instant::now();
            *meta_state.lock().unwrap() = match html {
                Ok(h) => match extract_link_meta(&h, &resolved_url) {
                    Some(meta) if !is_junk_meta(&h, &meta, &resolved_url) => {
                        LinkMetaState::Loaded(meta)
                    }
                    // Junk describes the wall, not the page — treat like a
                    // bot wall rather than rendering it.
                    Some(_) => LinkMetaState::Failed {
                        retry_at: Some(now + BLOCKED_RETRY),
                        attempts: attempts + 1,
                    },
                    None => LinkMetaState::Failed { retry_at: None, attempts: attempts + 1 },
                },
                Err(FetchError::RateLimited { until }) => {
                    LinkMetaState::Failed { retry_at: Some(until), attempts: attempts + 1 }
                }
                Err(FetchError::Blocked(_)) => LinkMetaState::Failed {
                    retry_at: Some(now + BLOCKED_RETRY),
                    attempts: attempts + 1,
                },
                Err(FetchError::Transient(_)) => {
                    let backoff =
                        (TRANSIENT_RETRY_BASE * 2u32.pow(attempts.min(4))).min(TRANSIENT_RETRY_CAP);
                    LinkMetaState::Failed { retry_at: Some(now + backoff), attempts: attempts + 1 }
                }
                Err(FetchError::Permanent(_)) => {
                    LinkMetaState::Failed { retry_at: None, attempts: attempts + 1 }
                }
            };
            link_seq.store(ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
            ctx.request_repaint();
        });
    }

}

/// Result of a [`MdRender::get_link_meta`] lookup.
pub enum LinkMetaLookup {
    /// Internal file link — a title from the file cache (never a card/capsule).
    Internal(String),
    /// Fetched external metadata.
    Loaded(LinkMeta),
    /// External fetch in progress — a card holds a skeleton until it resolves.
    Loading,
    /// No metadata and none coming (fetch failed, fetching off, or
    /// unresolvable) — degrade to a plain link rather than a stuck skeleton.
    Unavailable,
}

fn node_link_url(node: &AstNode<'_>) -> String {
    use comrak::nodes::NodeValue;
    match &node.data.borrow().value {
        NodeValue::Link(link) | NodeValue::Image(link) => link.url.clone(),
        _ => String::new(),
    }
}

/// Walking out from a link's neighbor via `next`, is the link alone on its line?
/// A line break (or running out of siblings — the paragraph edge) ends the line;
/// blank text is skipped; anything else means the link shares its line.
fn alone_on_line<'a>(
    mut sib: Option<&'a AstNode<'a>>, next: impl Fn(&'a AstNode<'a>) -> Option<&'a AstNode<'a>>,
) -> bool {
    while let Some(s) = sib {
        match &s.data.borrow().value {
            NodeValue::SoftBreak | NodeValue::LineBreak => return true,
            NodeValue::Text(t) if t.trim().is_empty() => {}
            _ => return false,
        }
        sib = next(s);
    }
    true
}

/// A choice from the desktop link context menu ([`link_menu_buttons`]).
#[derive(Clone, Copy)]
pub enum LinkMenuAction {
    Open,
    Copy,
    Edit,
}

/// What a desktop context menu over a link-like node acts on.
#[derive(Clone)]
pub struct LinkMenuTarget {
    pub url: String,
    pub is_wikilink: bool,
    pub is_image: bool,
    pub node_range: (Grapheme, Grapheme),
    /// See [`MdEdit::atom_edit_selection`].
    pub select: (Grapheme, Grapheme),
    pub force_reveal: bool,
}

/// The link section of a desktop context menu; `editable` gates "Edit".
pub fn link_menu_buttons(
    ui: &mut egui::Ui, is_image: bool, editable: bool,
) -> Option<LinkMenuAction> {
    let mut action = None;
    let (open, copy, edit) = if is_image {
        ("Open Image", "Copy URL", "Edit Image")
    } else {
        ("Open Link", "Copy Link", "Edit Link")
    };
    if ui.button(open).clicked() {
        action = Some(LinkMenuAction::Open);
        ui.close();
    }
    if ui.button(copy).clicked() {
        action = Some(LinkMenuAction::Copy);
        ui.close();
    }
    if editable && ui.button(edit).clicked() {
        action = Some(LinkMenuAction::Edit);
        ui.close();
    }
    action
}

impl<'ast> MdEdit {
    /// The link-like node under `pos` for a desktop context menu, resolved
    /// through the fragment actually painted there — no offset snapping
    /// from empty space beside a link.
    pub fn link_target_at_pos(
        &self, root: &'ast AstNode<'ast>, pos: egui::Pos2,
    ) -> Option<LinkMenuTarget> {
        let frag = self.renderer.fragment_at_pos(pos)?;
        let offset = frag.source_range.start();
        for node in root.descendants() {
            let (url, is_wikilink, is_image) = match &node.data.borrow().value {
                NodeValue::WikiLink(nwl) => (nwl.url.clone(), true, false),
                NodeValue::Image(l) => (l.url.clone(), false, true),
                NodeValue::Link(l) => (l.url.clone(), false, false),
                _ => continue,
            };
            let node_range = self.renderer.node_range(node);
            if url.is_empty() || offset < node_range.start() || offset >= node_range.end() {
                continue;
            }
            let (select, force_reveal) = self.atom_edit_selection(node);
            return Some(LinkMenuTarget {
                url,
                is_wikilink,
                is_image,
                node_range,
                select,
                force_reveal,
            });
        }
        None
    }

    /// The selection that edits a link-like node's hidden part: an image's
    /// or labeled link's destination, a wikilink's interior, a bare
    /// autolink's whole URL. `true` if acting on it should force-reveal via
    /// `entered_atom` — a bare autolink *is* its URL, so there is no
    /// interior sub-range whose selection would reveal the source.
    pub fn atom_edit_selection(&self, node: &'ast AstNode<'ast>) -> ((Grapheme, Grapheme), bool) {
        let node_range = self.renderer.node_range(node);
        let is_image = matches!(node.data.borrow().value, NodeValue::Image(_));
        match &node.data.borrow().value {
            NodeValue::WikiLink(_) => ((node_range.start() + 2, node_range.end() - 2), false),
            NodeValue::Link(_) | NodeValue::Image(_) => {
                let url = node_link_url(node);
                if !is_image && !url.is_empty() && self.renderer.link_is_auto(node, &url) {
                    (node_range, true)
                } else {
                    // `[title](url)` / `![alt](url)` — the postfix is
                    // `](url)`; without a label there are no children (no
                    // postfix) and the url starts after the opening syntax.
                    let open_len = if is_image { 4 } else { 3 };
                    let url_range = match self.renderer.postfix_range(node) {
                        Some(postfix) => (postfix.start() + 2, postfix.end() - 1),
                        None => (node_range.start() + open_len, node_range.end() - 1),
                    };
                    if url_range.0 <= url_range.1 {
                        (url_range, false)
                    } else {
                        // degenerate: interior caret
                        let caret = node_range.start() + (open_len - 2);
                        ((caret, caret), false)
                    }
                }
            }
            _ => (node_range, false),
        }
    }

    /// Touch tap on a plain rendered link or wikilink: select the link and
    /// pop the atom menu ("Open Link" / "Edit"), mirroring embed taps
    /// ([`Self::handle_embed_tap`]). Applied via `ops` so the selection is
    /// current when the platform builds the menu.
    pub fn handle_link_menu_taps(
        &mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui, id: egui::Id, ops: &mut Vec<Operation>,
    ) {
        if !self.renderer.touch_mode || self.renderer.readonly {
            return;
        }
        let parent_base = ui.id();
        for node in root.descendants() {
            match &node.data.borrow().value {
                NodeValue::Link(_) | NodeValue::WikiLink(_) => {}
                _ => continue,
            }
            let node_range = self.renderer.node_range(node);
            let salt = MdRender::link_interaction_id_salt(node_range);
            let response = match self
                .renderer
                .interaction_responses
                .get(&parent_base.with(salt))
            {
                Some(r) => r.clone(), // clone to release the `renderer` borrow
                None => continue,
            };
            if response.clicked() {
                ui.memory_mut(|m| m.request_focus(id));
                let region = Region::BetweenLocations {
                    start: Location::Grapheme(node_range.start()),
                    end: Location::Grapheme(node_range.end()),
                };
                self.calc_operations(ui.ctx(), root, Event::Select { region }, ops);
                ui.ctx()
                    .set_context_menu(response.rect.center_top(), ContextMenuTarget::Atom);
                return;
            }
        }
    }
}
