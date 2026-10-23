pub(crate) mod card;
mod hover;

use comrak::Arena;
use comrak::nodes::{AstNode, NodeValue};
use lb_rs::Uuid;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use lb_rs::model::text::operation_types::Operation;

use crate::file_cache::{FilesExt as _, ResolvedLink, split_fragment, wiki_fragment};
use crate::link_preview::{self, LinkMeta, Preview};
use crate::show::DocType;
use crate::style::{TypeRole, phosphor, phosphor_font_id};
use crate::tab::markdown_editor::TouchTarget;
use crate::tab::markdown_editor::fragment::slugify;
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::inline::image::image::image_url;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout, StyleInfo};
use crate::tab::markdown_editor::{MdEdit, MdRender};
use crate::tab::{ContextMenuTarget, ExtendedOutput as _, LinkSheet};
use crate::theme::palette_v2::ThemeExt as _;

pub use crate::resolvers::LinkState;
use crate::resolvers::rich_spans;
use crate::widgets::GlyphonLabel;

/// Width cap of a link's hover card.
const LINK_TIP_W: f32 = 300.0;

impl<'ast> MdRender {
    pub fn text_format_link(&self, parent: &AstNode<'_>, state: LinkState) -> Format {
        let parent_text_format = self.text_format(parent);
        let theme = self.ctx.get_lb_theme();
        let color = match state {
            LinkState::Normal => theme.fg().blue,
            LinkState::Warning { .. } => theme.fg().yellow,
            LinkState::Broken { .. } => theme.fg().red,
            LinkState::Placeholder { .. } => theme.fg().cyan,
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
        egui::Id::unique(("md_link", node_range))
    }

    /// Click-target id for a link rendered as a capsule preview; distinct from
    /// the plain-link salt so its embed-style tap handling doesn't collide.
    pub fn link_capsule_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::unique(("md_link_capsule", node_range))
    }

    /// Emit a link as a circumfix. For autolinks with a fetched title,
    /// swap the URL bytes for the title via `push_override`. Empty-text
    /// links (`[](url)`) are not autolinks and have nothing to show, so
    /// they render their raw source like other incomplete syntax.
    /// A `Sense::click` interaction scope routes clicks here when they
    /// act on the link (cmd held, read-only, or unrevealed touch);
    /// otherwise a desktop link gets a hover-only scope for its tip and
    /// clicks fall through to the editor for cursor placement.
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
            self.get_link_title(&url)
        } else {
            None
        };

        // Resolved autolink preview → a "chip" capsule (favicon + title, no
        // underline) that behaves like the block card / inline image: tap selects,
        // cmd/keyboard-tap opens (`handle_link_capsule_interactions`), interior
        // reveal only.
        if let Some(t) = title {
            // a file's name reads as an ordinary link; the pill is for web previews
            let internal = matches!(self.get_link_meta(&url), LinkMetaLookup::Internal(_));
            let pill_fmt = if internal {
                link_fmt.clone()
            } else {
                self.text_format_link_pill(parent, state.clone())
            };
            layout.interaction_open(Self::link_capsule_id_salt(node_range), egui::Sense::click());
            layout.style_open(StyleInfo {
                format: pill_fmt.clone(),
                source_range: node_range,
                chip: !internal,
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
        let sense = self.link_sense(clickable);
        if let Some(sense) = sense {
            layout.interaction_open(salt, sense);
        }
        self.layout_circumfix(layout, node, range, link_fmt.clone());
        if sense.is_some() {
            layout.interaction_close();
        }
    }

    /// A link's interaction scope: click when it opens on click, else hover
    /// on desktop so the pointer can still show its tip. None on touch, where
    /// taps on an unopenable link place the cursor.
    pub fn link_sense(&self, clickable: bool) -> Option<egui::Sense> {
        if clickable {
            Some(egui::Sense::click())
        } else if !self.touch_mode {
            Some(egui::Sense::hover())
        } else {
            None
        }
    }

    /// The hover card explaining a yellow or red link: what's wrong, then the
    /// fix in quieter ink; for any other web link, its preview card.
    /// `state` is worked out only under the pointer. `preview` is the link's
    /// URL and whether its text is that URL, for links that may preview.
    pub fn show_link_tip(
        &self, ctx: &egui::Context, response: &egui::Response, state: impl FnOnce() -> LinkState,
        preview: Option<(&str, bool)>,
    ) {
        if !response.contains_pointer() {
            // still called, so the tip's dwell timing sees the pointer leave
            crate::style::tip_card_placed(ctx, response, response.rect, LINK_TIP_W, |_| {});
            return;
        }
        let (LinkState::Warning { message }
        | LinkState::Broken { message }
        | LinkState::Placeholder { message }) = state()
        else {
            if let Some((url, shows_url)) = preview {
                self.show_link_preview(ctx, response, url, shows_url);
            }
            return;
        };
        let mut lines = message.lines();
        let Some(what) = lines.next() else { return };
        let fixes: Vec<&str> = lines.collect();
        crate::style::tip_card_placed(ctx, response, response.rect, LINK_TIP_W, |ui| {
            let t = ui.ctx().get_lb_theme();
            let line = |ui: &mut egui::Ui, text: &str, ink| {
                ui.add(
                    GlyphonLabel::new_rich(rich_spans(text), ink)
                        .font_size(TypeRole::Body.size())
                        .line_height(TypeRole::Body.line_height())
                        .max_width(LINK_TIP_W),
                );
            };
            line(ui, what, t.neutral_fg());
            for fix in fixes {
                line(ui, fix, t.neutral_fg_secondary());
            }
        });
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
    /// `frag_start` bounds the pass to `fragments[frag_start..]`: the main
    /// render passes `0`; the drag float passes its own fragment offset so
    /// only the floated capsule's favicon is painted (its selection tint is
    /// skipped — the card is already distinct).
    pub fn show_capsule_overlays(
        &mut self, ui: &mut egui::Ui, root: &'ast AstNode<'ast>, viewport: egui::Rect,
        frag_start: usize,
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
            let chip_rects: Vec<egui::Rect> = self.fragments[frag_start..]
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
            // a scrolled table shows only some of a row
            let row =
                egui::Rect::from_x_y_ranges(egui::Rangef::EVERYTHING, chip_rects[0].y_range());
            let showing = self.table_showing(row).unwrap_or(egui::Rect::NOTHING);
            let row_clip = ui.clip_rect();
            ui.set_clip_rect(row_clip.intersect(showing));
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
                    // non-advancing child ui — see the embed paint in
                    // `show_wrap_layout` (#4892)
                    let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(icon_rect));
                    self.embeds
                        .show(ui, &favicon_url, icon_rect, egui::CornerRadius::same(3));
                } else {
                    ui.painter().text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        phosphor::LINK,
                        phosphor_font_id(size),
                        self.ctx.get_lb_theme().neutral_fg_secondary(),
                    );
                }
            }
            ui.set_clip_rect(row_clip);

            if frag_start == 0 && !sel.is_empty() && sel.contains_range(&node_range, true, true) {
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
                for r in rows.iter().filter_map(|r| self.table_showing(*r)) {
                    ui.painter().rect_filled(r, 2.0, tint);
                }
            }
        }

        ui.set_clip_rect(entry_clip);
    }

    pub fn resolve_link(&self, url: &str) -> Option<ResolvedLink> {
        self.link_resolver.resolve_link(url)
    }

    /// Open `url` in-app for internal file links and in the browser for web
    /// URLs; an unresolved destination explains itself instead. `new_tab` is
    /// only for an explicit new-tab request (cmd-click / multi-open).
    pub fn open_resolved_link(&self, url: &str, ctx: &egui::Context, new_tab: bool) {
        match self.resolve_link(url) {
            Some(ResolvedLink::File(file_id)) => self.open_linked_file(url, file_id, ctx, new_tab),
            Some(ResolvedLink::External(target)) => {
                ctx.open_url(egui::OpenUrl { url: target, new_tab: true })
            }
            None => self.explain_unopenable(url, self.link_state_for_url(url), ctx),
        }
    }

    /// Opens a link's destination: a wiki title when `wikilink`, else a URL.
    pub fn open_dest(&self, dest: &str, wikilink: bool, ctx: &egui::Context) {
        match wikilink {
            true => self.open_wikilink(dest, ctx, false),
            false => self.open_resolved_link(dest, ctx, false),
        }
    }

    /// Opens the file a wikilink names, behind a confirmation when it means
    /// different files to different readers. A title nothing matches creates
    /// the note where the resolver has a place for it.
    pub fn open_wikilink(&self, title: &str, ctx: &egui::Context, new_tab: bool) {
        let Some(file_id) = self.resolve_wikilink(title) else {
            if let Some((parent, names)) = self.link_resolver.wikilink_placement(title) {
                return ctx.create_note(parent, names);
            }
            let shown = format!("[[{title}]]");
            return self.explain_unopenable(&shown, self.link_state_for_wikilink(title), ctx);
        };
        let heading = wiki_fragment(title).1;
        match self.link_resolver.wikilink_confirmation(title) {
            Some(reason) => self.ask_to_open(file_id, heading, new_tab, reason, ctx),
            None => ctx.open_link(file_id, heading.map(Into::into), new_tab),
        }
    }

    /// Opens the file `url` resolved to, behind a confirmation when the link
    /// means different files to different readers.
    fn open_linked_file(&self, url: &str, file_id: Uuid, ctx: &egui::Context, new_tab: bool) {
        let heading = split_fragment(url).1;
        match self.link_resolver.confirmation(url) {
            Some(reason) => self.ask_to_open(file_id, heading, new_tab, reason, ctx),
            None => ctx.open_link(file_id, heading.map(Into::into), new_tab),
        }
    }

    fn ask_to_open(
        &self, file_id: Uuid, heading: Option<&str>, new_tab: bool, reason: String,
        ctx: &egui::Context,
    ) {
        let shown = self.files.read().unwrap().path(file_id);
        let heading = heading.map(String::from);
        ctx.show_link_sheet(LinkSheet {
            open: Some((file_id, new_tab)),
            heading,
            shown,
            message: reason,
        });
    }

    /// Says why a link that names nothing the reader can see doesn't open.
    fn explain_unopenable(&self, shown: &str, state: LinkState, ctx: &egui::Context) {
        if let LinkState::Broken { message } = state {
            let sheet = LinkSheet { open: None, heading: None, shown: shown.into(), message };
            ctx.show_link_sheet(sheet);
        }
    }

    pub fn link_state_for_url(&self, url: &str) -> LinkState {
        self.link_resolver.link_state(url)
    }

    pub fn link_state_for_wikilink(&self, url: &str) -> LinkState {
        self.link_resolver.wikilink_state(url)
    }

    /// The external URL of file `id`, which means the file wherever it is
    /// pasted.
    pub fn external_url(&self, id: Uuid) -> String {
        let origin = self
            .open_origin
            .as_deref()
            .unwrap_or("https://app.lockbook.net");
        format!("{origin}/open/{id}")
    }

    /// What copying a link puts on the clipboard. A link to a file copies as
    /// the file's external URL, a heading kept as its slug: the one form that
    /// means the same place wherever it is pasted. Anything else copies as
    /// written.
    pub fn link_to_copy(&self, url: &str, wikilink: bool) -> String {
        let file = if wikilink {
            self.resolve_wikilink(url)
        } else {
            match self.resolve_link(url) {
                Some(ResolvedLink::File(id)) => Some(id),
                _ => None,
            }
        };
        let Some(id) = file else { return url.into() };
        let fragment = match wikilink {
            true => wiki_fragment(url).1.map(slugify),
            false => split_fragment(url).1.map(String::from),
        };
        match fragment.filter(|f| !f.is_empty()) {
            Some(fragment) => format!("{}#{fragment}", self.external_url(id)),
            None => self.external_url(id),
        }
    }

    /// Whether [`Self::selection_open_target`] is an image, for the platform
    /// edit menu's wording.
    pub fn selection_target_is_image(&mut self) -> bool {
        self.selection_target().is_some_and(|(_, _, image)| image)
    }

    /// What to copy ([`Self::link_to_copy`]) for the first link-like node
    /// whose source range intersects the current selection — gates and feeds
    /// the platform edit menu's "Open Link" / "Copy Link".
    pub fn selection_open_target(&mut self) -> Option<String> {
        let (url, wikilink, _) = self.selection_target()?;
        Some(self.link_to_copy(&url, wikilink))
    }

    /// The first link-like node whose source range intersects the selection:
    /// its destination, and whether it is a wikilink or an image.
    fn selection_target(&mut self) -> Option<(String, bool, bool)> {
        let arena = Arena::new();
        let root = self.reparse(&arena);
        let selection = self.buffer.current.selection;
        root.descendants().find_map(|node| {
            let is_image = image_url(node).is_some();
            let target = match &node.data.borrow().value {
                NodeValue::Link(l) => (l.url.clone(), false, false),
                NodeValue::Image(l) => (l.url.clone(), false, true),
                NodeValue::WikiLink(nwl) => (nwl.url.clone(), true, is_image),
                _ => return None,
            };
            (!target.0.is_empty() && self.node_range(node).intersects(&selection, true))
                .then_some(target)
        })
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

        let mut wikilinks = vec![];
        let mut linked = vec![];
        let mut urls = vec![];
        let mut broken = vec![];

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
                if self.resolve_wikilink(&url).is_some() {
                    wikilinks.push(url);
                } else {
                    broken.push((url, true));
                }
                continue;
            }

            match self.resolve_link(&url) {
                Some(ResolvedLink::File(id)) => linked.push((url, id)),
                Some(ResolvedLink::External(url)) => {
                    urls.push(egui::OpenUrl { url, new_tab: false });
                }
                None => broken.push((url, false)),
            }
        }

        let opening = wikilinks.len() + linked.len() + urls.len();
        if opening == 0 {
            // nothing opens: say why the first link doesn't
            match broken.first() {
                Some((title, true)) => self.open_wikilink(title, ctx, false),
                Some((url, false)) => self.open_resolved_link(url, ctx, false),
                None => {}
            }
            return;
        }
        let new_tab = opening > 1;
        for title in wikilinks {
            self.open_wikilink(&title, ctx, new_tab);
        }
        for (url, id) in linked {
            self.open_linked_file(&url, id, ctx, new_tab);
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

    /// Hover → the Warning/Broken tip, and `PointingHand` when a click
    /// opens; click → navigate in place. The producer's interaction scope
    /// is the gate: cmd held (desktop), read-only, or touch — where the
    /// click instead selects the link and pops the menu
    /// ([`MdEdit::handle_link_menu_taps`]).
    pub fn handle_link_interactions(&mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui) {
        let parent_base = ui.scope_id();
        for node in root.descendants() {
            let (url, is_wikilink, is_image) = match &node.data.borrow().value {
                NodeValue::WikiLink(nwl) => (nwl.url.clone(), true, false),
                NodeValue::Link(nl) => (nl.url.clone(), false, false),
                NodeValue::Image(nl) => (nl.url.clone(), false, true),
                _ => continue,
            };
            let id = parent_base.with(Self::link_interaction_id_salt(self.node_range(node)));

            let open = Event::OpenLink { url: url.clone(), wikilink: is_wikilink };
            let tap = if self.readonly {
                open.clone()
            } else {
                Event::Select { region: self.node_range(node).into() }
            };
            self.touch_consume_interaction(id, TouchTarget::Tap(tap));

            let Some(response) = self.interaction_responses.get(&id) else {
                continue;
            };

            if response.hovered() && response.sense.senses_click() {
                crate::style::link_cursor(ui.ctx());
            }
            let preview =
                (!is_wikilink && !is_image).then(|| (url.as_str(), self.link_is_auto(node, &url)));
            self.show_link_tip(
                ui.ctx(),
                response,
                || {
                    if is_wikilink {
                        self.link_state_for_wikilink(&url)
                    } else {
                        self.link_state_for_url(&url)
                    }
                },
                preview,
            );

            if response.clicked() && (self.readonly || !self.touch_mode) {
                self.render_events.push(open);
                return;
            }
        }
    }

    /// The text an autolink shows in place of its URL: a file's name, what a
    /// known site's URL says it is, or the page's title once fetched.
    fn get_link_title(&self, url: &str) -> Option<String> {
        match self.get_link_meta(url) {
            LinkMetaLookup::Internal(title) => Some(title),
            LinkMetaLookup::Loaded(meta) => Some(link_preview::label(url).unwrap_or(meta.title)),
            LinkMetaLookup::Loading | LinkMetaLookup::Unavailable => link_preview::label(url),
        }
    }

    /// Resolve a link's preview metadata. Internal links (lb:// / relative
    /// paths) resolve synchronously to a title from the file cache. External
    /// http(s) links are fetched asynchronously (gated by `contact_linked_sites`)
    /// and shared through [`crate::link_preview::LinkPreviews`]; `Unavailable`
    /// while a fetch failed or fetching is off.
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

        match self.previews.get(&resolved_url) {
            Preview::Loaded(meta) => LinkMetaLookup::Loaded(meta),
            Preview::Loading => LinkMetaLookup::Loading,
            Preview::Unavailable => LinkMetaLookup::Unavailable,
        }
    }

    /// Drop `url`'s cached metadata and fetch it fresh — the context menu's
    /// "Refresh Preview". Explicitly user-invoked, but still behind the
    /// contact-linked-sites opt-in (previews don't render without it anyway).
    pub fn refresh_link_meta(&self, url: &str) {
        if let Some(ResolvedLink::External(u)) = self.resolve_link(url) {
            if u.starts_with("http://") || u.starts_with("https://") {
                self.previews.refresh(&u);
            }
        }
    }

    /// Re-fetch preview metadata for every link intersecting the selection —
    /// the touch edit menu's "Refresh Preview".
    pub fn refresh_selection_previews(&mut self) {
        let arena = Arena::new();
        let root = self.reparse(&arena);
        let selection = self.buffer.current.selection;
        let mut urls = vec![];
        for node in root.descendants() {
            if let NodeValue::Link(l) = &node.data.borrow().value {
                if !l.url.is_empty() && self.node_range(node).intersects(&selection, true) {
                    urls.push(l.url.clone());
                }
            }
        }
        for url in urls {
            self.refresh_link_meta(&url);
        }
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

impl MdEdit {
    /// A link to this note's heading with `slug`: the external URL, which
    /// opens the same place wherever it is pasted.
    pub fn heading_link(&self, slug: &str) -> String {
        format!("{}#{slug}", self.renderer.external_url(self.file_id))
    }

    /// [`Self::heading_link`] for the heading on the caret's line, for the
    /// platform edit menu's "Copy Heading Link".
    pub fn selection_heading_link(&mut self) -> Option<String> {
        if self.renderer.plaintext {
            return None;
        }
        let arena = Arena::new();
        let root = self.renderer.reparse(&arena);
        let caret = self.renderer.buffer.current.selection.1;
        let slug = self.renderer.heading_slug_at(root, caret)?;
        Some(self.heading_link(&slug))
    }
}

/// A choice from the desktop link context menu ([`fill_link_menu`]).
#[derive(Clone, Copy)]
pub enum LinkMenuAction {
    Open,
    Copy,
    Edit,
    Refresh,
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

/// The link section of a desktop context menu; `editable` gates "Edit",
/// `refreshable` gates "Refresh Preview" (rendered previews only).
///
/// `map` wraps each [`LinkMenuAction`] so a host menu can mix link items with
/// Cut / Copy / Paste without a second entry list.
pub fn fill_link_menu<T>(
    e: &mut crate::style::context_menu::Entries<T>, is_image: bool, editable: bool,
    refreshable: bool, map: impl Fn(LinkMenuAction) -> T,
) {
    let (open, copy, edit) = if is_image {
        ("Open Image", "Copy URL", "Edit Image")
    } else {
        ("Open Link", "Copy Link", "Edit Link")
    };
    e.item(crate::style::phosphor::ARROW_SQUARE_OUT, open, map(LinkMenuAction::Open));
    e.item(crate::style::phosphor::COPY, copy, map(LinkMenuAction::Copy));
    if editable {
        e.item(crate::style::phosphor::PENCIL, edit, map(LinkMenuAction::Edit));
    }
    if refreshable {
        e.item(
            crate::style::phosphor::ARROWS_CLOCKWISE,
            "Refresh Preview",
            map(LinkMenuAction::Refresh),
        );
    }
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
            let is_image = image_url(node).is_some();
            let (url, is_wikilink, is_image) = match &node.data.borrow().value {
                NodeValue::WikiLink(nwl) => (nwl.url.clone(), true, is_image),
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
            NodeValue::WikiLink(wl) => {
                let open = if wl.embed { 3 } else { 2 };
                ((node_range.start() + open, node_range.end() - 2), false)
            }
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
        let parent_base = ui.scope_id();
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
