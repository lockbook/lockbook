use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use lb_rs::spawn;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use crate::file_cache::{FilesExt as _, ResolvedLink};
use crate::show::DocType;
use crate::tab::ExtendedOutput as _;
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::input;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{
    FontFamily, Format, Layout, StyleInfo,
};
use crate::theme::icons::Icon;
use crate::theme::palette_v2::ThemeExt as _;

/// Fetched preview metadata for an external link — title and (optionally)
/// the absolute URLs for a favicon and an OG/Twitter card thumbnail. Stored
/// in [`LayoutCache::link_meta`] keyed by the destination URL and persisted
/// to the workspace sidecar so reopened docs render previews on first paint.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkMeta {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon_url: Option<String>,
}

pub enum LinkMetaState {
    Loading,
    Loaded(LinkMeta),
    Failed,
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

    /// Capsule-pill format for a small-preview link. Combined with
    /// `StyleInfo { chip: true }` the wrap layout paints a rounded
    /// capsule that hugs the row's height — same mechanism as the fold
    /// `···` chip. No underline (the capsule background does the
    /// affordance work); link color stays so the title still reads as
    /// "link" rather than as a neutral pill.
    pub fn text_format_link_pill(&self, parent: &AstNode<'_>, state: LinkState) -> Format {
        let theme = self.ctx.get_lb_theme();
        Format {
            background: theme.neutral_bg_secondary(),
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
        let (size, sized_title) = self.link_preview_size(node);

        // Display modes:
        //   Small — capsule chip (same primitive as the fold `···` chip)
        //     spanning the bracket+url source. Suffix is stripped.
        //   Large — plain link-styled title; the discord/slack card
        //     above carries the visual weight, so no inline pill.
        //   None — fall through to circumfix (bracket text shown, syntax
        //     hidden when not revealed). Bare URLs render as-is; the
        //     preview popover handles converting them to `[title](url)`
        //     on demand, so the editor no longer auto-fetches titles for
        //     every autolink it sees.
        let sized_title =
            (!revealed && !trimmed.is_empty() && size != LinkPreviewSize::None).then(|| {
                // empty stripped title (e.g. `[|small](url)`) falls back
                // to the host so the pill isn't a dangling icon.
                if sized_title.is_empty() {
                    host_from_url(&url).unwrap_or_else(|| url.clone())
                } else {
                    sized_title.clone()
                }
            });
        match (sized_title, size) {
            (Some(t), LinkPreviewSize::Small) => {
                let pill_fmt = self.text_format_link_pill(parent, state.clone());
                layout.style_open(StyleInfo {
                    format: pill_fmt.clone(),
                    source_range: node_range,
                    chip: true,
                });
                // Leading em-space reserves a square at the chip's left
                // edge for the favicon overlay painted in
                // `show_paragraph`. Em-space (U+2003) is 1em wide so the
                // favicon area scales naturally with the chip's height.
                // The character displays as blank so a missing favicon
                // just leaves harmless whitespace.
                let with_favicon_slot = format!("\u{2003}{t}");
                layout.push_override(trimmed, &with_favicon_slot, pill_fmt);
                layout.style_close();
            }
            (Some(t), LinkPreviewSize::Large) => {
                layout.style_open(StyleInfo::new(link_fmt.clone(), node_range));
                layout.push_override(trimmed, &t, link_fmt.clone());
                layout.style_close();
            }
            (Some(_), LinkPreviewSize::None) | (None, _) => {
                self.layout_circumfix(layout, node, range, link_fmt.clone());
            }
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

    /// Find the link node whose source range contains the cursor, if any.
    /// Skips images (they have separate semantics) and skips wikilinks
    /// (no `|size` syntax). Returns enough metadata for the popover /
    /// hover chip to render and commit without re-walking the AST.
    pub fn link_under_cursor(&self, root: &'ast AstNode<'ast>) -> Option<LinkUnderCursor> {
        let sel = self.buffer.current.selection;
        if sel.0 != sel.1 {
            return None;
        }
        self.link_at_offset(root, sel.1)
    }

    /// Same as [`Self::link_under_cursor`] but for an arbitrary grapheme
    /// offset — used by the hover chip after mapping pointer position to
    /// a graphme via the wrap layout.
    pub fn link_at_offset(
        &self, root: &'ast AstNode<'ast>, offset: Grapheme,
    ) -> Option<LinkUnderCursor> {
        for node in root.descendants() {
            if !matches!(node.data.borrow().value, NodeValue::Link(_)) {
                continue;
            }
            let url = match &node.data.borrow().value {
                NodeValue::Link(l) => l.url.clone(),
                _ => continue,
            };
            let node_range = self.node_range(node);
            if !node_range.contains(offset, true, true) {
                continue;
            }
            let infix = self.infix_range(node)?;
            let bracket_text = self.buffer[infix].to_string();
            let (stripped, size) = parse_link_preview_size(&bracket_text);
            let is_autolink = self.link_is_auto(node, &url);
            return Some(LinkUnderCursor {
                node_range,
                infix_range: infix,
                url,
                is_autolink,
                current_size: size,
                stripped_title: stripped.to_string(),
            });
        }
        None
    }

    /// Mutate a link's `|size` suffix to `new_size`. Queues a single
    /// `Event::Replace` over the bracket text — splicing in the stripped
    /// title plus the new suffix (or no suffix for `None`). Designed as
    /// the entry point for any UI that drives size changes: the popover,
    /// the hover chip, a slash command, etc. No-op for image nodes (their
    /// `|WxH` suffix has different semantics) and for autolinks (the
    /// link's "bracket text" is the URL itself — converting bare→sized
    /// requires the fetched title and is handled by a separate path).
    pub fn set_link_preview_size(&mut self, node: &'ast AstNode<'ast>, new_size: LinkPreviewSize) {
        if matches!(node.data.borrow().value, NodeValue::Image(_)) {
            return;
        }
        let url = node_link_url(node);
        if self.link_is_auto(node, &url) {
            return;
        }
        let Some(range) = self.infix_range(node) else {
            return;
        };
        let (stripped, _current) = parse_link_preview_size(&self.buffer[range]);
        let suffix = match new_size {
            LinkPreviewSize::None => "",
            LinkPreviewSize::Small => "|small",
            LinkPreviewSize::Large => "|large",
        };
        let new_text = format!("{stripped}{suffix}");
        self.render_events.push(input::Event::Replace {
            region: range.into(),
            text: new_text,
            advance_cursor: false,
        });
    }

    /// Parse the trailing `|small` / `|large` off the link's bracket text.
    /// Returns the requested size and the title with the suffix removed.
    /// Always returns `None`/empty for image nodes — images carry their
    /// own `|WxH` size suffix and shouldn't double-interpret as a link
    /// preview.
    pub fn link_preview_size(&self, node: &'ast AstNode<'ast>) -> (LinkPreviewSize, String) {
        if matches!(node.data.borrow().value, NodeValue::Image(_)) {
            return (LinkPreviewSize::None, String::new());
        }
        let Some(range) = self.infix_range(node) else {
            return (LinkPreviewSize::None, String::new());
        };
        let (stripped, size) = parse_link_preview_size(&self.buffer[range]);
        (size, stripped.to_string())
    }

    /// Overlay favicons on every Small-preview pill within `paragraph`.
    /// Run after all paragraph lines have painted, so chip fragments
    /// are populated in `self.fragments`. For each sized=Small Link
    /// node: warm the favicon URL (kicks off the fetch on first
    /// encounter, no-op once cached), find the leftmost chip fragment
    /// covering its source range, and — only if the favicon texture is
    /// already loaded — paint a centered square into the leading
    /// em-space that `layout_link` reserved at the chip's left edge.
    /// Skipping the paint while loading avoids the embed resolver's
    /// 48pt placeholder overflowing a tiny favicon rect.
    pub fn show_small_link_favicons(&mut self, ui: &mut egui::Ui, paragraph: &'ast AstNode<'ast>) {
        for descendant in paragraph.descendants() {
            let url = match &descendant.data.borrow().value {
                NodeValue::Link(l) => l.url.clone(),
                _ => continue,
            };
            let (size, _) = self.link_preview_size(descendant);
            if size != LinkPreviewSize::Small {
                continue;
            }
            let favicon_url = match self.get_link_meta(&url) {
                LinkMetaLookup::External(Some(m)) => m.favicon_url,
                _ => None,
            };
            let Some(favicon_url) = favicon_url else { continue };
            self.embeds.warm(&favicon_url);
            if !self.embeds.is_loaded(&favicon_url) {
                continue;
            }
            let node_range = self.node_range(descendant);
            let first_chip_rect = self
                .fragments
                .iter()
                .find(|f| {
                    f.source_range.intersects(&node_range, true)
                        && f.style_stack.last().is_some_and(|s| s.chip)
                })
                .map(|f| f.rect);
            let Some(chip_rect) = first_chip_rect else { continue };
            // Square favicon shifted inward so it doesn't nuzzle the
            // capsule's left edge — the icon_inset matches the chip's
            // leading pad (`CHIP_SIDE_PAD * row_height`), so the favicon
            // sits at the same x as a glyph would. Combined with the
            // leading em-space (1em advance), the title still starts
            // ~0.5em to the right of the favicon's right edge.
            let icon_size = chip_rect.height() * 0.7;
            let icon_inset = chip_rect.height() * 0.3;
            let icon_rect = egui::Rect::from_min_size(
                egui::Pos2::new(
                    chip_rect.min.x + icon_inset,
                    chip_rect.center().y - icon_size * 0.5,
                ),
                egui::Vec2::splat(icon_size),
            );
            // Dark-mode contrast tile. Dark monochrome marks like
            // GitHub's render onto transparent backgrounds — against the
            // chip's dark-gray pill they vanish. A near-white tile drawn
            // *behind* the favicon shows through every transparent pixel
            // and lets dark marks read, while opaque favicons (X, YT)
            // simply hide it. Generalizes across sites because it
            // doesn't depend on the icon's actual colors. Skipped in
            // light mode since the chip's neutral bg is already light
            // enough that dark marks read fine on top of it.
            if self.dark_mode {
                ui.painter()
                    .rect_filled(icon_rect, 3.0, egui::Color32::from_gray(235));
            }
            self.embeds.show(ui, &favicon_url, icon_rect);
        }
    }

    /// Fixed-height card for a large link preview. Matches the visual
    /// budget of Discord/Slack unfurls: thumbnail row + title row +
    /// description row. Width is the paragraph width; the thumbnail
    /// occupies the left, text the right. Loading/failed thumbnails
    /// don't change height — we always reserve the full card height so
    /// the layout doesn't reflow as previews settle.
    pub fn large_link_card_height(&self, node: &'ast AstNode<'ast>) -> f32 {
        let _ = node; // height is currently fixed; node reserved for future per-link sizing
        let row = self.layout.row_height;
        let padding = self.layout.inline_padding;
        // thumbnail | title + description on the right
        let body = (row * 5.0).max(80.0);
        body + padding * 2.0
    }

    /// Render a Discord/Slack-style preview card for a large link. Uses
    /// the existing `embeds` resolver to fetch the thumbnail texture
    /// (same path as image embeds), falling back to a placeholder
    /// rectangle while the fetch is pending. Title + description come
    /// from the in-memory [`LinkMetaState`] populated by `get_link_meta`.
    pub fn show_large_link_card(
        &mut self, ui: &mut egui::Ui, node: &'ast AstNode<'ast>, top_left: egui::Pos2, url: &str,
    ) {
        let width = self.width(node);
        let height = self.large_link_card_height(node);
        let rect = egui::Rect::from_min_size(top_left, egui::Vec2::new(width, height));
        let theme = self.ctx.get_lb_theme();

        ui.painter().rect_stroke(
            rect,
            6.0,
            egui::Stroke { width: 1.0, color: theme.neutral_bg_tertiary() },
            egui::epaint::StrokeKind::Inside,
        );

        // Layout: thumbnail on the left, text on the right. The thumb
        // slot's *width* matches the texture's natural aspect at full
        // body height — so a 2:1 og:image (GitHub) fills the slot
        // edge-to-edge, and a 16:9 og:image (YouTube) gets a narrower
        // slot that it also fills edge-to-edge. Capped at 2:1 (and at
        // half the card width on narrow viewports); textures wider than
        // the cap vertically letterbox into the capped slot. Card body
        // is height minus 2×padding.
        let pad = self.layout.inline_padding;
        let body = rect.shrink(pad);
        let thumb_height = body.height();
        let max_thumb_width = (thumb_height * 2.0).min(body.width() * 0.5);

        let meta = match self.get_link_meta(url) {
            LinkMetaLookup::External(Some(m)) => Some(m),
            _ => None,
        };
        let thumb_url = meta
            .as_ref()
            .and_then(|m| m.thumbnail_url.as_ref())
            .cloned();

        // Texture's natural aspect, defaulting to a square when the size
        // isn't known yet (first-ever paint of an uncached URL). Once
        // the image cache settles the next paint picks up the real
        // aspect and the slot re-sizes once; image_dims is persisted so
        // reopens are stable from frame zero.
        let ppp = self.ctx.pixels_per_point();
        let natural = thumb_url
            .as_ref()
            .map(|u| self.embeds.size(u) / ppp)
            .unwrap_or(egui::Vec2::splat(thumb_height));
        let aspect = if natural.y > 0.0 { natural.x / natural.y } else { 1.0 };
        let thumb_width = (thumb_height * aspect).min(max_thumb_width);
        let thumb_rect =
            egui::Rect::from_min_size(body.min, egui::Vec2::new(thumb_width, thumb_height));
        let text_rect =
            egui::Rect::from_min_max(egui::Pos2::new(thumb_rect.max.x + pad, body.min.y), body.max);

        if let Some(thumb_url) = thumb_url {
            self.embeds.warm(&thumb_url);
            // For texture aspect within the cap, fitted == thumb_rect
            // exactly — no padding. For over-wide textures (aspect >
            // 2:1) `scale` is bound by width and the image vertically
            // letterboxes; the backdrop fills the leftover band.
            let scale = if natural.x > 0.0 && natural.y > 0.0 {
                (thumb_rect.width() / natural.x).min(thumb_rect.height() / natural.y)
            } else {
                1.0
            };
            let fitted_size = natural * scale;
            let fits_exactly = (fitted_size.x - thumb_rect.width()).abs() < 0.5
                && (fitted_size.y - thumb_rect.height()).abs() < 0.5;
            if !fits_exactly {
                ui.painter()
                    .rect_filled(thumb_rect, 4.0, theme.neutral_bg_tertiary());
            }
            let fitted = egui::Rect::from_center_size(thumb_rect.center(), fitted_size);
            self.embeds.show(ui, &thumb_url, fitted);
        } else {
            ui.painter()
                .rect_filled(thumb_rect, 4.0, theme.neutral_bg_tertiary());
            ui.painter().text(
                thumb_rect.center(),
                egui::Align2::CENTER_CENTER,
                Icon::IMAGE.icon,
                egui::FontId { size: thumb_height * 0.4, family: egui::FontFamily::Monospace },
                theme.neutral_fg_secondary(),
            );
        }

        let (title, description) = match meta.as_ref() {
            Some(m) => (m.title.clone(), m.description.clone().unwrap_or_default()),
            None => (host_from_url(url).unwrap_or_else(|| url.to_string()), String::new()),
        };
        let row = self.layout.row_height;
        let title_font = egui::FontId::proportional(row);
        let desc_font = egui::FontId::proportional(row * 0.85);

        // Title: wrap to the text column width, capped at 2 rows.
        // `painter().text` doesn't wrap; build a galley instead so long
        // page titles ("YouTube — full nine-word video title …") line-
        // break inside the card instead of overflowing right.
        let title_galley = {
            let mut job = egui::text::LayoutJob::single_section(
                title,
                egui::TextFormat {
                    font_id: title_font.clone(),
                    color: theme.fg().blue,
                    ..Default::default()
                },
            );
            job.wrap.max_width = text_rect.width();
            job.wrap.max_rows = 2;
            ui.fonts(|f| f.layout_job(job))
        };
        let title_height = title_galley.size().y;
        ui.painter()
            .galley(text_rect.min, title_galley, theme.fg().blue);

        if !description.is_empty() {
            // Description fills whatever vertical space remains under the
            // wrapped title. `max_rows` derived from that space keeps the
            // card height fixed; the long-form description gets cleanly
            // truncated rather than spilling beyond the card.
            let desc_top = text_rect.min.y + title_height + 4.0;
            let remaining = text_rect.max.y - desc_top;
            let desc_line_height = row * 0.85 * 1.2;
            let max_rows = (remaining / desc_line_height).floor().max(0.0) as usize;
            if max_rows > 0 {
                let mut job = egui::text::LayoutJob::single_section(
                    description,
                    egui::TextFormat {
                        font_id: desc_font,
                        color: theme.neutral_fg_secondary(),
                        ..Default::default()
                    },
                );
                job.wrap.max_width = text_rect.width();
                job.wrap.max_rows = max_rows;
                let galley = ui.fonts(|f| f.layout_job(job));
                ui.painter().galley(
                    egui::Pos2::new(text_rect.min.x, desc_top),
                    galley,
                    theme.neutral_fg_secondary(),
                );
            }
        }
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

    /// Looks up — or kicks off a fetch for — preview metadata (title,
    /// favicon, thumbnail) for `url`. For internal links the title comes
    /// from the file cache synchronously and there's nothing further to
    /// fetch. For external http/https links the cache entry transitions
    /// from `Loading` → `Loaded`/`Failed` asynchronously; callers see
    /// `External(None)` until the fetch completes (then the next frame
    /// gets `External(Some(meta))`).
    pub fn get_link_meta(&self, url: &str) -> LinkMetaLookup {
        let Some(resolved) = self.resolve_link(url) else {
            return LinkMetaLookup::Unsupported;
        };

        let resolved_url = match resolved {
            ResolvedLink::File(id) => {
                let guard = self.files.read().unwrap();
                let Some(file) = guard.get_by_id(id) else {
                    return LinkMetaLookup::Unsupported;
                };
                let title = DocType::from_name(&file.name).display_name(&file.name);
                return LinkMetaLookup::Internal(title.to_string());
            }
            ResolvedLink::External(url)
                if url.starts_with("http://") || url.starts_with("https://") =>
            {
                url
            }
            ResolvedLink::External(_) => return LinkMetaLookup::Unsupported,
        };

        let arc = match self
            .layout_cache
            .link_meta
            .borrow_mut()
            .entry(resolved_url.clone())
        {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let arc = Arc::new(Mutex::new(LinkMetaState::Loading));
                e.insert(arc.clone());
                let client = self.client.clone();
                let ctx = self.ctx.clone();
                let meta_state = arc.clone();
                let link_seq = self.layout_cache.link_seq.clone();
                let ws_seq = self.ws_seq.clone();
                let persistence = self.persistence.clone();
                spawn!({
                    const CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
                    const GOOGLEBOT: &str =
                        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

                    #[cfg(not(target_arch = "wasm32"))]
                    let mut html = fetch_html(&client, &resolved_url, CHROME);
                    #[cfg(target_arch = "wasm32")]
                    let mut html = fetch_html(&client, &resolved_url, CHROME).await;

                    // some sites (e.g. Twitter/X) only serve static content to known crawlers
                    if html
                        .as_deref()
                        .ok()
                        .and_then(|h| extract_link_meta(h, &resolved_url))
                        .is_none()
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

                    let meta = html.ok().and_then(|h| extract_link_meta(&h, &resolved_url));
                    if let (Some(meta), Some(persistence)) = (meta.as_ref(), persistence.as_ref()) {
                        persistence.merge_link_meta(std::collections::HashMap::from([(
                            resolved_url.clone(),
                            meta.clone(),
                        )]));
                    }
                    *meta_state.lock().unwrap() = meta
                        .map(LinkMetaState::Loaded)
                        .unwrap_or(LinkMetaState::Failed);
                    link_seq.store(ws_seq.fetch_add(1, Ordering::Relaxed), Ordering::Relaxed);
                    ctx.request_repaint();
                });
                arc
            }
        };

        let state = arc.lock().unwrap();
        match &*state {
            LinkMetaState::Loaded(m) => LinkMetaLookup::External(Some(m.clone())),
            LinkMetaState::Loading | LinkMetaState::Failed => LinkMetaLookup::External(None),
        }
    }
}

/// Result of [`MdRender::get_link_meta`]. Distinguishes internal links
/// (resolved synchronously from the file cache, no fetch) from external
/// links (async) and from unsupported URL schemes.
pub enum LinkMetaLookup {
    /// File link resolved to a display title via the file cache.
    Internal(String),
    /// External http/https link. `Some` once the async fetch settles to
    /// `Loaded`; `None` while loading or after a failure.
    External(Option<LinkMeta>),
    /// URL didn't resolve, or used a non-http(s) scheme we don't preview.
    Unsupported,
}

/// Snapshot of the link node currently under the cursor — produced by
/// [`MdRender::link_under_cursor`] and consumed by the preview popover.
/// All ranges are pre-resolved grapheme ranges (no AST needed downstream).
#[derive(Clone)]
pub struct LinkUnderCursor {
    /// Whole link source range — includes `[`, the bracket text, and `](url)`.
    pub node_range: (Grapheme, Grapheme),
    /// Bracket-text range for bracketed links; for autolinks this equals
    /// the URL token range (since bracket = URL).
    pub infix_range: (Grapheme, Grapheme),
    /// Destination URL (from the comrak Link node, not from source).
    pub url: String,
    /// True when bracket text exactly matches the URL — a bare/autolink.
    /// Commit needs to wrap instead of splice in this case.
    pub is_autolink: bool,
    /// `|small`/`|large` state currently parsed from bracket text.
    pub current_size: LinkPreviewSize,
    /// Bracket text with any `|small`/`|large` suffix stripped — the
    /// title to keep when changing size, or to seed an autolink wrap.
    pub stripped_title: String,
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

/// Inline link preview "size" set via a trailing `|small` / `|large` on
/// the link's bracket text — e.g. `[Title|small](url)`. Mirrors Obsidian
/// image dims (`![alt|WxH](url)`): the modifier lives after the last `|`
/// of the link text, and an unknown / missing modifier leaves the bar as
/// literal title text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LinkPreviewSize {
    #[default]
    None,
    Small,
    Large,
}

/// Best-effort host extraction for falling back when an explicit title is
/// empty — `[|small](https://x.com/foo)` shows `x.com` rather than a
/// dangling pill marker.
fn host_from_url(url: &str) -> Option<String> {
    url::Url::parse(url).ok()?.host_str().map(|h| h.to_string())
}

/// Strip the trailing `|small` / `|large` off a link's bracket text. Returns
/// the title to display and the requested size. Only the segment after the
/// last `|` counts — anything else (no bar, unknown keyword, whitespace)
/// yields `None` and leaves the input untouched.
pub fn parse_link_preview_size(text: &str) -> (&str, LinkPreviewSize) {
    let Some(bar) = text.rfind('|') else {
        return (text, LinkPreviewSize::None);
    };
    let size = match &text[bar + 1..] {
        "small" => LinkPreviewSize::Small,
        "large" => LinkPreviewSize::Large,
        _ => return (text, LinkPreviewSize::None),
    };
    (&text[..bar], size)
}

fn extract_link_meta(html: &str, base_url: &str) -> Option<LinkMeta> {
    let doc = Html::parse_document(html);
    let title = extract_title(&doc)?;
    let description = extract_description(&doc);
    let thumbnail_url = extract_thumbnail_url(&doc).and_then(|u| resolve_url(base_url, &u));
    let favicon_url = extract_favicon_url(&doc, base_url);
    Some(LinkMeta { title, description, thumbnail_url, favicon_url })
}

fn extract_title(doc: &Html) -> Option<String> {
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
    doc.select(&meta_sel)
        .find_map(|e| e.value().attr("content"))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

fn extract_description(doc: &Html) -> Option<String> {
    let sel = Selector::parse(
        "meta[property='og:description'], meta[name='twitter:description'], meta[name='description']",
    )
    .ok()?;
    doc.select(&sel)
        .find_map(|e| e.value().attr("content"))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

fn extract_thumbnail_url(doc: &Html) -> Option<String> {
    let sel = Selector::parse("meta[property='og:image'], meta[name='twitter:image']").ok()?;
    doc.select(&sel)
        .find_map(|e| e.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// First `<link rel*="icon">` href, resolved against `base_url`. Falls
/// back to the page origin's `/favicon.ico` so sites that omit the link
/// tag still get a favicon (the request may 404; the image cache treats
/// that as `Failed` and the small preview drops the icon).
fn extract_favicon_url(doc: &Html, base_url: &str) -> Option<String> {
    let sel = Selector::parse("link[rel~='icon']").ok()?;
    let from_link = doc
        .select(&sel)
        .find_map(|e| e.value().attr("href"))
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .and_then(|h| resolve_url(base_url, &h));
    if from_link.is_some() {
        return from_link;
    }
    let origin = url::Url::parse(base_url).ok()?;
    let fallback = origin.join("/favicon.ico").ok()?;
    Some(fallback.to_string())
}

fn resolve_url(base: &str, href: &str) -> Option<String> {
    let base = url::Url::parse(base).ok()?;
    let joined = base.join(href).ok()?;
    Some(joined.to_string())
}

#[cfg(test)]
mod tests {
    use super::{LinkPreviewSize, parse_link_preview_size};

    #[test]
    fn link_preview_size_suffix() {
        let p = parse_link_preview_size;
        // recognized sizes strip the suffix
        assert_eq!(p("Title|small"), ("Title", LinkPreviewSize::Small));
        assert_eq!(p("Title|large"), ("Title", LinkPreviewSize::Large));
        // empty title still parses
        assert_eq!(p("|small"), ("", LinkPreviewSize::Small));
        // last `|` wins — earlier bars are part of the title
        assert_eq!(p("a|b|small"), ("a|b", LinkPreviewSize::Small));
        // bare title — no bar
        assert_eq!(p("Title"), ("Title", LinkPreviewSize::None));
        // unknown keyword keeps the bar as literal title
        assert_eq!(p("Title|foo"), ("Title|foo", LinkPreviewSize::None));
        // case-sensitive — uppercase isn't recognized
        assert_eq!(p("Title|SMALL"), ("Title|SMALL", LinkPreviewSize::None));
        // whitespace breaks the match
        assert_eq!(p("Title|small "), ("Title|small ", LinkPreviewSize::None));
        assert_eq!(p("Title| small"), ("Title| small", LinkPreviewSize::None));
        // empty input
        assert_eq!(p(""), ("", LinkPreviewSize::None));
    }
}
