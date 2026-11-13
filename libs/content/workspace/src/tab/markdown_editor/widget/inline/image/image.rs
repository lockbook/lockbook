use std::f32;

use comrak::nodes::{AstNode, NodeValue};
use egui::{self, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, IntoRangeExt as _, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;

use crate::file_cache::{ResolvedLink, wiki_fragment};
use crate::resolvers::image_embed::{wiki_embed_key, wiki_embed_title};
use crate::style::ThemeExt;
use crate::tab::image_viewer::is_supported_image_fmt;
use crate::tab::markdown_editor::TouchTarget;
use crate::tab::markdown_editor::input::{Advance, Bound, Event, Increment, Location, Region};
use crate::tab::markdown_editor::widget::block::drag::TouchReorder;
use crate::tab::markdown_editor::widget::block::leaf::diagram::Picture;
use crate::tab::markdown_editor::widget::inline::link::hover::CARD_RADIUS;
use crate::tab::markdown_editor::widget::inline::link::{LinkMenuAction, fill_link_menu};
use crate::tab::markdown_editor::widget::inline::tab_embed::tab_embed_title;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::FragmentContent;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{EmbedKind, EmbedSpec, Layout};
use crate::tab::markdown_editor::{MdEdit, MdRender};
use crate::tab::{ContextMenuTarget, ExtendedOutput as _};

/// What an image node shows, as embeds take it: a markdown image's
/// destination, or the [`wiki_embed_key`] of a wiki embed whose title names
/// an image (`![[photo.png]]`). Other wiki embeds are links.
pub fn image_url(node: &AstNode<'_>) -> Option<String> {
    match &node.data.borrow().value {
        NodeValue::Image(link) => Some(link.url.clone()),
        NodeValue::WikiLink(wl) if wl.embed && names_image(&wl.url) => {
            Some(wiki_embed_key(&wl.url))
        }
        _ => None,
    }
}

/// Whether a wiki title names an image (or a drawing) by its extension.
fn names_image(title: &str) -> bool {
    let file = wiki_fragment(title).0;
    let ext = file.rsplit_once('.').map_or("", |(_, ext)| ext.trim());
    is_supported_image_fmt(ext) || ext.eq_ignore_ascii_case("svg")
}

/// An image a drag is moving, from grab until release.
#[derive(Clone, Debug)]
pub struct ImageDrag {
    pub image: (Grapheme, Grapheme),
    /// The buffer's sequence `image` is right for; edits since move it along.
    pub seq: usize,
    /// What it floats as, from the grab: its url, its kind and its size,
    /// so it draws where the finger holds it with its source scrolled away.
    pub url: String,
    pub kind: EmbedKind,
    pub size: Vec2,
    /// The live pointer, or where it last was once a finger lifts.
    pub pointer: egui::Pos2,
    /// Pointer less the image's top left at the grab, so the floating
    /// image keeps the grab point under the pointer.
    pub grab_offset: Vec2,
    /// Let go with its move queued: it floats where it lands until the move
    /// applies, so no frame draws it back where it was.
    pub dropped: bool,
    /// Where it lands if let go now, as this frame found it.
    pub target: Option<Grapheme>,
}

impl MdRender {
    /// Prefetch textures under `node` so the image cache does not drop them.
    /// Used for off-screen neighbors about to scroll into view. Covers
    /// images ([`image_url`]) and link-preview thumbnails/favicons.
    pub fn warm_images<'a>(&self, node: &'a comrak::nodes::AstNode<'a>) {
        for descendant in node.descendants() {
            if let Some(url) = image_url(descendant) {
                self.embeds.prefetch(&url);
                continue;
            }
            let NodeValue::Link(link) = &descendant.data.borrow().value else { continue };
            let resolved = match self.resolve_link(&link.url) {
                Some(ResolvedLink::External(url)) => url,
                _ => continue,
            };
            let Some(meta) = self.previews.loaded(&resolved) else {
                continue;
            };
            if let Some(url) = meta.thumbnail_url.as_deref() {
                self.embeds.prefetch(url);
            }
            if let Some(url) = meta.favicon_url.as_deref() {
                self.embeds.prefetch(url);
            }
        }
    }
}

impl<'ast> MdRender {
    pub fn image_size(&self, texture_size: Vec2, width: f32, requested: ImageDims) -> Vec2 {
        // Constrain the image to fit the renderer with a margin of breathing
        // room. Clamp to non-negative — when the renderer is too small to
        // satisfy the margin (initial frames before viewport is known,
        // zero-height containers), the image collapses to 0 rather than
        // letting negative dimensions corrupt the surrounding block layout.
        let image_max_size = (Vec2::new(self.width, self.viewport_height)
            - Vec2::splat(self.layout.margin))
        .max(Vec2::ZERO);

        // Texture dims are device pixels; the layout works in logical points.
        // Convert so a Retina screenshot (ppp 2) isn't shown at 2x real size.
        let natural = texture_size / self.ctx.pixels_per_point();
        let aspect = if natural.x > 0.0 { natural.y / natural.x } else { 1.0 };

        // Obsidian-style explicit dims (`![alt|WxH](url)`) are logical points,
        // not device pixels. They set the box — upscaling past natural size is
        // allowed — but still shrink to fit the viewport, preserving the
        // requested aspect ratio. Width-only keeps the natural aspect.
        if requested.width.is_some() || requested.height.is_some() {
            let target = match (requested.width, requested.height) {
                (Some(w), Some(h)) => Vec2::new(w, h),
                (Some(w), None) => Vec2::new(w, w * aspect),
                (None, Some(h)) if aspect > 0.0 => Vec2::new(h / aspect, h),
                (None, Some(h)) => Vec2::new(h, h),
                (None, None) => unreachable!(),
            };
            let scale = (image_max_size.x / target.x)
                .min(image_max_size.y / target.y)
                .min(1.0);
            return (target * scale).max(Vec2::ZERO);
        }

        // A degenerate texture (a decode with no size yet, a failed one)
        // collapses rather than dividing by zero into NaN geometry.
        if natural.x <= 0.0 || natural.y <= 0.0 {
            return Vec2::ZERO;
        }

        // only shrink images, never stretch beyond their natural size
        let width = width.min(natural.x).min(image_max_size.x);
        let height = (natural.y * width / natural.x).min(image_max_size.y);

        // if height was the binding constraint, recompute width to preserve aspect ratio
        let width = natural.x * height / natural.y;

        Vec2::new(width, height)
    }

    /// Obsidian-style explicit dimensions for an image, parsed from the
    /// trailing `|W` / `|WxH` of its alt text (`![alt|100x100](url)`) or of
    /// a wiki embed's label (`![[photo.png|100x100]]`).
    pub fn image_dims(&self, node: &'ast AstNode<'ast>) -> ImageDims {
        let wiki = matches!(node.data.borrow().value, NodeValue::WikiLink(_));
        match self.infix_range(node) {
            Some(range) if wiki => parse_image_dims(&format!("|{}", &self.buffer[range])),
            Some(range) => parse_image_dims(&self.buffer[range]),
            None => ImageDims::default(),
        }
    }

    /// Logical-point size an image node ([`image_url`]) will render at,
    /// constrained by its block's width and any Obsidian `|WxH` hint. `None`
    /// if `node` isn't an image, has no leaf-block ancestor, or collapses to
    /// zero on the first frame.
    pub fn image_logical_size(&self, node: &'ast AstNode<'ast>) -> Option<Vec2> {
        let url = image_url(node)?;
        let block_ancestor = node
            .ancestors()
            .skip(1)
            .find(|a| a.data.borrow().value.is_leaf_block())?;
        let width = self.embed_room(block_ancestor);
        let in_cell = matches!(block_ancestor.data.borrow().value, NodeValue::TableCell);
        let texture_size = self.embeds.size(&url);
        let mut size = self.image_size(texture_size, width, self.image_dims(node));
        // a size asked for by name fits the viewport; in a cell it fits the cell
        if in_cell && size.x > width {
            size *= width / size.x;
        }
        (size.x > 0.0 && size.y > 0.0).then_some(size)
    }

    /// Collapsed → emit one `InlineItem::Embed` covering the syntax.
    /// Revealed / `disable_images` / multi-line / unsizable → fall
    /// through to `layout_link` so the source bytes render for editing.
    pub fn layout_image(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
    ) {
        let node_range = self.node_range(node);
        let single_line = range.contains_range(&node_range, true, true);
        let revealed = self.range_revealed_interior(node_range);
        let collapsed_size = (!self.disable_images && !revealed && single_line)
            .then(|| self.image_logical_size(node))
            .flatten();
        let Some(size) = collapsed_size else {
            match node.data.borrow().value {
                NodeValue::WikiLink(_) => self.layout_wikilink(layout, node, range),
                _ => self.layout_link(layout, node, range),
            }
            return;
        };
        let Some(url) = image_url(node) else { return };
        // Always interactive: a plain tap selects, a cmd/keyboard-hidden tap
        // opens, and in a note a mouse drags it to another place.
        layout.interaction_open(Self::image_interaction_id_salt(node_range), self.embed_sense());
        // sized against the text it sits in: a heading's, else the body's
        let block = node
            .ancestors()
            .skip(1)
            .find(|a| a.data.borrow().value.is_leaf_block());
        let row_height = block.map_or(self.layout.row_height, |b| self.row_height(b));
        let baseline = self.body_baseline(row_height);
        let (ascent, descent) = small_image_metrics(size.y, row_height, baseline);
        layout.push_embed(EmbedSpec {
            advance: size.x,
            ascent,
            descent,
            source_range: node_range,
            url,
            kind: EmbedKind::Image,
        });
        layout.interaction_close();
    }

    /// How wide an embed in `block`, its leaf block, can be: the block's
    /// width, or in a table cell a column's widest, its padding aside,
    /// which the cell's width can't set while it comes from what the cell
    /// holds.
    pub fn embed_room(&self, block: &'ast AstNode<'ast>) -> f32 {
        match block.data.borrow().value {
            NodeValue::TableCell => self.layout.table_column_max - 2.0 * self.layout.block_padding,
            _ => self.width(block),
        }
    }

    /// How an embed takes the pointer: a click, and where the note is
    /// editable, a mouse drag that moves it (touch lifts with a long press).
    pub fn embed_sense(&self) -> egui::Sense {
        if self.readonly || self.touch_mode {
            egui::Sense::click()
        } else {
            egui::Sense::click_and_drag()
        }
    }

    /// Interaction-response salt for a rendered image; distinct from
    /// [`Self::link_interaction_id_salt`] so the collapsed-image and
    /// revealed-link handlers don't collide.
    pub fn image_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::unique(("md_image", node_range))
    }

    /// The source range of what an inline drag lifts at `pos`: the image
    /// drawn there, or a note embed held by its title, its inside being the
    /// embedded note's.
    pub fn image_at(&self, pos: egui::Pos2) -> Option<(Grapheme, Grapheme)> {
        if let Some((salt, _)) = self.fragment_at_pos(pos).and_then(|f| f.interaction) {
            let mut images = self.bounds.images.iter().copied();
            if let Some(image) = images.find(|&r| Self::image_interaction_id_salt(r) == salt) {
                return Some(image);
            }
        }
        self.fragments.iter().find_map(|f| {
            let FragmentContent::Embed { kind: EmbedKind::Tab, url } = &f.content else {
                return None;
            };
            let band = self.tab_embed_band(url, f.rect)?;
            band.contains(pos).then_some(f.source_range)
        })
    }

    /// Where the image or embed at `image` was last drawn, its url and kind.
    pub fn drawn_image(
        &self, image: (Grapheme, Grapheme),
    ) -> Option<(egui::Rect, &str, EmbedKind)> {
        self.fragments.iter().find_map(|f| match &f.content {
            FragmentContent::Embed { url, kind } if f.source_range == image => {
                Some((f.rect, url.as_str(), *kind))
            }
            _ => None,
        })
    }

    /// A drag of `image` grabbed at `pointer`.
    pub fn image_drag(&self, image: (Grapheme, Grapheme), pointer: egui::Pos2) -> ImageDrag {
        let (rect, url, kind) = self
            .drawn_image(image)
            .map_or((egui::Rect::NOTHING, "", EmbedKind::Image), |(rect, url, kind)| {
                (rect, url, kind)
            });
        let grab_offset = if rect.is_positive() { pointer - rect.left_top() } else { Vec2::ZERO };
        let (url, size) =
            (url.to_string(), if rect.is_positive() { rect.size() } else { Vec2::ZERO });
        let seq = self.buffer.current.seq;
        ImageDrag {
            image,
            seq,
            url,
            kind,
            size,
            pointer,
            grab_offset,
            dropped: false,
            target: None,
        }
    }

    /// Recompute [`super::super::super::bounds::Bounds::images`] — the source
    /// range of every inline image. Images are empty when they render raw
    /// (disabled), since then the source is plain editable text. Depends on
    /// text only.
    pub fn calc_atom_bounds<'a>(&mut self, root: &'a AstNode<'a>) {
        let mut images = Vec::new();
        for node in root.descendants() {
            if image_url(node).is_some() && !self.disable_images {
                images.push(self.node_range(node));
            }
        }
        images.sort_unstable();
        self.bounds.images = images;
        self.calc_diagram_bounds(root);
        self.calc_math_bounds(root);
    }
}

impl<'ast> MdEdit {
    /// The gesture that *opens* an embed rather than selecting it: a cmd-click
    /// (desktop). Touch taps select and pop the context menu, whose "Open
    /// Link" action opens, except in an editor that never takes focus (an
    /// embedded note), which has no menu of its own. Shared by every embed kind.
    pub fn embed_tap_opens(&self, ui: &egui::Ui) -> bool {
        !self.takes_focus
            || !self.renderer.touch_mode
                && (self.renderer.readonly || ui.ctx().input(|i| i.modifiers.command))
    }

    /// Shared tap handling for one embed fragment (inline image, link card, or
    /// capsule): when `open`, a click opens `url`, else it selects the node
    /// and (touch) pops the edit menu as an `Atom` target, so the platform
    /// offers "Edit" (`Event::EnterAtom`). Desktop right-click shows the link
    /// menu ([`fill_link_menu`]). Registers the fragment rects as a touch
    /// target, whose tap selects the node or (read-only) opens; `salt` identifies
    /// the fragment's `Sense::click` scope. No-op if the embed wasn't rendered
    /// this frame. The per-kind handlers differ only in node lookup.
    #[allow(clippy::too_many_arguments)]
    pub fn handle_embed_tap(
        &mut self, root: &'ast AstNode<'ast>, node: &'ast AstNode<'ast>, ui: &egui::Ui,
        id: egui::Id, ops: &mut Vec<Operation>, url: &str, salt: egui::Id, open: bool,
    ) {
        let response = match self
            .renderer
            .interaction_responses
            .get(&ui.scope_id().with(salt))
        {
            Some(r) => r.clone(), // clone to release the `renderer` borrow
            None => return,
        };

        let node_range = self.renderer.node_range(node);
        // a wiki link's key holds its title
        let (dest, wikilink) = match wiki_embed_title(url) {
            Some(title) => (title, true),
            None => (url, false),
        };
        // an image hidden past the note's scope shows on a tap; shown, the
        // picture says nothing
        let is_image = image_url(node).is_some();
        let withheld = is_image && self.renderer.embeds.withheld(url);
        let tap = if withheld {
            Event::LoadImage { url: url.to_string() }
        } else if self.renderer.readonly {
            Event::OpenLink { url: dest.to_string(), wikilink }
        } else {
            Event::Select { region: node_range.into() }
        };
        self.renderer
            .touch_consume_interaction(ui.scope_id().with(salt), TouchTarget::Tap(tap));

        if (open || withheld) && response.hovered() {
            crate::style::link_cursor(ui.ctx());
        }
        if !is_image || !self.renderer.embeds.is_loaded(url) {
            let state = || match wikilink {
                true => self.renderer.link_state_for_wikilink(dest),
                false => self.renderer.link_state_for_url(dest),
            };
            // a block card or a tab embed already shows everything its preview would
            let card = !is_image
                && (self.renderer.link_renders_as_card(node) || tab_embed_title(node).is_some());
            let preview = (!is_image && !card).then_some((dest, false, wikilink));
            self.renderer
                .show_link_tip(ui.ctx(), &response, state, preview);
        }
        if withheld && response.clicked() {
            self.renderer.embeds.load_anyway(url);
            return;
        }
        if response.clicked() {
            if open {
                self.renderer.open_dest(dest, wikilink, ui.ctx());
            } else {
                ui.memory_mut(|m| m.request_focus(id));
                let region = Region::BetweenLocations {
                    start: Location::Grapheme(node_range.start()),
                    end: Location::Grapheme(node_range.end()),
                };
                self.calc_operations(ui.ctx(), root, Event::Select { region }, ops);
                if self.renderer.touch_mode {
                    ui.ctx()
                        .set_context_menu(response.rect.center_top(), ContextMenuTarget::Atom);
                }
            }
        }

        if !self.renderer.touch_mode {
            let editable = !self.renderer.readonly;
            // cards/capsules render fetched previews; images don't
            let refreshable = !is_image
                && !wikilink
                && self.renderer.contact_linked_sites
                && matches!(self.renderer.resolve_link(url), Some(ResolvedLink::External(_)));
            let t = ui.ctx().get_lb_theme();
            let action = crate::style::context_menu::show(&response, &t, |e| {
                fill_link_menu(e, is_image, editable, refreshable, |a| a);
            });
            match action {
                Some(LinkMenuAction::Open) => self.renderer.open_dest(dest, wikilink, ui.ctx()),
                Some(LinkMenuAction::Copy) => ui
                    .ctx()
                    .copy_text(self.renderer.link_to_copy(dest, wikilink)),
                Some(LinkMenuAction::CopyImage) => {
                    self.renderer.copy_picture(Picture::Image(url.to_string()))
                }
                Some(LinkMenuAction::Refresh) => self.renderer.refresh_link_meta(url),
                Some(LinkMenuAction::Edit) => {
                    let (select, force_reveal) = self.atom_edit_selection(node);
                    if force_reveal {
                        self.renderer.entered_atom = Some(node_range);
                    }
                    ui.memory_mut(|m| m.request_focus(id));
                    let region = Region::BetweenLocations {
                        start: Location::Grapheme(select.start()),
                        end: Location::Grapheme(select.end()),
                    };
                    self.calc_operations(ui.ctx(), root, Event::Select { region }, ops);
                }
                None => {}
            }
        }
    }

    /// Open or select an inline image clicked this frame (see [`Self::handle_embed_tap`]).
    pub fn handle_image_interactions(
        &mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui, id: egui::Id, ops: &mut Vec<Operation>,
    ) {
        let open = self.embed_tap_opens(ui);
        for node in root.descendants() {
            let Some(url) = image_url(node) else { continue };
            let node_range = self.renderer.node_range(node);
            let salt = MdRender::image_interaction_id_salt(node_range);
            let grabbed = self
                .renderer
                .interaction_responses
                .get(&ui.scope_id().with(salt))
                .is_some_and(|r| r.drag_started_by(egui::PointerButton::Primary));
            if grabbed {
                let pointer = ui.input(|i| i.pointer.press_origin()).unwrap_or_default();
                self.in_progress_image_drag = Some(self.renderer.image_drag(node_range, pointer));
                crate::style::context_menu::close(ui.ctx());
            }
            self.handle_embed_tap(root, node, ui, id, ops, &url, salt, open);
        }
        if self.renderer.readonly || self.renderer.touch_mode {
            return;
        }
        // a note embed, by its title, as a mouse drags an image
        let r = &self.renderer;
        let held: Vec<((Grapheme, Grapheme), egui::Rect)> = r
            .fragments
            .iter()
            .filter_map(|f| match &f.content {
                FragmentContent::Embed { kind: EmbedKind::Tab, url } => {
                    Some((f.source_range, r.tab_embed_band(url, f.rect)?))
                }
                _ => None,
            })
            .collect();
        for (range, band) in held {
            let resp = ui.interact(
                band,
                ui.scope_id().with(("md_embed_band", range)),
                egui::Sense::drag(),
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            if resp.drag_started_by(egui::PointerButton::Primary) {
                let pointer = ui.input(|i| i.pointer.press_origin()).unwrap_or_default();
                self.in_progress_image_drag = Some(self.renderer.image_drag(range, pointer));
            }
        }
    }

    /// Where an inline embed dragged from `image` would land with the
    /// pointer at `pos`: the caret a click there places. Where it can't
    /// land, inside a word, a code span, a link or a block of code, it goes
    /// to that thing's nearer edge. `None` where the drop moves nothing or
    /// `pos` is outside the note's `viewport`.
    fn image_drop_offset(
        &self, root: &'ast AstNode<'ast>, image: (Grapheme, Grapheme), pos: egui::Pos2,
        viewport: egui::Rect,
    ) -> Option<Grapheme> {
        if !viewport.contains(pos) {
            return None;
        }
        let to = self
            .location_to_char_offset(Location::Pos(pos))
            .into_range();
        let to = self.renderer.snap_selection_out_of_fold_tags(to);
        let to = self.renderer.snap_selection_into_table_cells(to).start();
        let r = &self.renderer;
        // a span's edges, the nearer first
        let edges = |(start, end): (Grapheme, Grapheme)| {
            if to.0 - start.0 <= end.0 - to.0 { [start, end] } else { [end, start] }
        };
        // inside these the source shows, or the url takes the image in; a
        // bare url, as comrak reads one, runs on to the next space
        let raw = |n: &'ast AstNode<'ast>| {
            matches!(
                n.data.borrow().value,
                NodeValue::Code(_)
                    | NodeValue::Math(_)
                    | NodeValue::HtmlInline(_)
                    | NodeValue::Link(_)
                    | NodeValue::WikiLink(_)
                    | NodeValue::Image(_)
                    | NodeValue::FootnoteReference(_)
            )
        };
        let spans: Vec<((Grapheme, Grapheme), bool)> = root
            .descendants()
            .filter(|&n| raw(n))
            .map(|n| {
                let range = r.node_range(n);
                let bare = matches!(n.data.borrow().value, NodeValue::Link(_))
                    && !r.buffer[range].starts_with(['[', '<']);
                let line = r
                    .bounds
                    .source_lines
                    .iter()
                    .find(|l| l.contains(range.end(), true, true));
                let mut end = range.end();
                while bare
                    && line.is_some_and(|l| end < l.end())
                    && !r.buffer[(end, end + 1)].chars().all(char::is_whitespace)
                {
                    end += 1;
                }
                ((range.start(), end), bare)
            })
            .collect();
        // a backslash before it would escape its `!`
        let lands = |to: Grapheme| {
            let inline = r
                .bounds
                .inline_paragraphs
                .iter()
                .any(|p| p.contains_inclusive(to));
            let escaped = to.0 > 0 && &r.buffer[(to - 1, to)] == "\\";
            let after_url = spans.iter().any(|&(span, bare)| bare && span.end() == to);
            inline && !escaped && !after_url && !image.contains_inclusive(to)
        };
        let span = spans
            .iter()
            .find(|&&(span, bare)| {
                span.start() < to && (to < span.end() || bare && to == span.end())
            })
            .map(|&(span, _)| span);
        // nor a word
        let word = || {
            let &line = r
                .bounds
                .source_lines
                .iter()
                .find(|l| l.contains(to, true, true))?;
            let letter = |g: Grapheme| r.buffer[(g, g + 1)].chars().all(char::is_alphanumeric);
            let inside = to > line.start() && to < line.end() && letter(to - 1) && letter(to);
            let (mut start, mut end) = (to, to);
            while inside && start > line.start() && letter(start - 1) {
                start -= 1;
            }
            while inside && end < line.end() && letter(end) {
                end += 1;
            }
            inside.then_some((start, end))
        };
        let candidates = match span.or_else(word) {
            Some(span) => edges(span).to_vec(),
            None => vec![to],
        };
        if let Some(to) = candidates.into_iter().find(|&to| lands(to)) {
            return Some(to);
        }
        // in a block of code: the line before it or after it, nearer first
        let lines = &r.bounds.source_lines;
        let block = root
            .descendants()
            .filter(|n| {
                let value = &n.data.borrow().value;
                value.is_leaf_block() && !value.contains_inlines()
            })
            .map(|n| r.node_range(n))
            .find(|range| range.contains(to, true, true))?;
        let first = lines
            .iter()
            .position(|l| l.contains(block.start(), true, true))?;
        let last = lines
            .iter()
            .rposition(|l| l.contains(block.end(), true, true))?;
        let before = first.checked_sub(1).map(|i| lines[i].end());
        let after = lines.get(last + 1).map(|l| l.start());
        let sides = if to.0 - block.start().0 <= block.end().0 - to.0 {
            [before, after]
        } else {
            [after, before]
        };
        sides.into_iter().flatten().find(|&to| lands(to))
    }

    /// Follows an image drag: the grabbing hand, edge scroll, and on release
    /// the move to where the pointer let go. A mouse drag lives while the
    /// button is down; a touch drag while its long press is armed. `root`
    /// is this frame's parse.
    pub(crate) fn handle_image_drag(&mut self, ui: &mut egui::Ui, root: &'ast AstNode<'ast>) {
        let Some(mut drag) = self
            .in_progress_image_drag
            .clone()
            .filter(|drag| !drag.dropped)
        else {
            return;
        };
        // the text may have changed under it, a sync or a key: the image's
        // range follows, or the drag ends if the image itself was edited
        let mut image = drag.image;
        if !self.renderer.buffer.transform_range(drag.seq, &mut image) {
            self.in_progress_image_drag = None;
            return;
        }
        (drag.image, drag.seq) = (image, self.renderer.buffer.current.seq);
        let live = if self.renderer.touch_mode {
            matches!(self.touch_reorder, TouchReorder::Armed { .. })
        } else {
            ui.input(|i| i.pointer.primary_down())
        };
        // a lifted finger's last position stands: touch-up nulls egui's
        // pointer, and a platform-driven drop has already set it
        if live || !self.renderer.touch_mode {
            if let Some(pointer) = self.drag_pointer(ui) {
                drag.pointer = pointer;
            }
        }
        // rising above the finger or pointer as it moves down (`drag_lift`)
        let aim = drag.pointer - egui::vec2(0.0, self.drag_lift(drag.grab_offset.y));
        drag.target = self.image_drop_offset(root, drag.image, aim, ui.clip_rect());
        if live {
            self.in_progress_image_drag = Some(drag);
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
            self.auto_scroll_to_drag_pointer(ui, true);
        } else {
            self.in_progress_image_drag = None;
            if let Some(to) = drag.target {
                let range = drag.image;
                self.event.internal_events.push(Event::Move { range, to });
                self.in_progress_image_drag = Some(ImageDrag { dropped: true, ..drag });
            }
        }
        ui.ctx().request_repaint();
    }

    /// A dragged image: an empty slot where it was, the caret where it would
    /// land, and the image itself under the pointer.
    pub(crate) fn draw_image_drag(&mut self, ui: &mut egui::Ui) {
        let Some(drag) = self.in_progress_image_drag.clone() else { return };
        let theme = self.renderer.ctx.get_lb_theme();
        if let Some(to) = drag.target {
            // the drop line, standing in the text: unlike the caret, it has its dot
            if let Some([top, bot]) = self.cursor_line(to) {
                let start = egui::pos2(top.x, top.y - 4.0);
                crate::style::paint_drop_line(
                    ui.painter(),
                    start,
                    egui::pos2(top.x, bot.y),
                    &theme,
                );
            }
        }
        let (url, kind) = (drag.url.as_str(), drag.kind);
        let corner = match kind {
            EmbedKind::Tab => CARD_RADIUS.corner(),
            _ => egui::CornerRadius::same(2),
        };
        // the empty slot, as far as its source shows: a scrolled table hides it
        let src = self.renderer.drawn_image(drag.image).map(|(src, ..)| src);
        if let Some(slot) = src.and_then(|src| self.renderer.row_showing(drag.image, src)) {
            ui.painter().rect_filled(slot, corner, theme.neutral_bg());
            crate::style::paint_inset_shadow(ui.painter(), slot, corner, &theme);
        }
        let card = egui::Rect::from_min_size(drag.pointer - drag.grab_offset, drag.size);
        let float_ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(card));
        float_ui.multiply_opacity(crate::style::DRAG_OPACITY);
        float_ui
            .painter()
            .add(crate::style::overlay_shadow(&theme).as_shape(card, corner));
        match kind {
            EmbedKind::Tab => {
                float_ui
                    .painter()
                    .rect_filled(card, corner, theme.neutral_bg());
                // the floating copy only paints
                self.renderer.painting_drag_float = true;
                self.renderer
                    .paint_tab_embed(float_ui, url, card, drag.image.start());
                self.renderer.painting_drag_float = false;
            }
            _ => self.renderer.embeds.show(float_ui, url, card, corner),
        }
    }

    /// If the selection exactly covers a collapsed image (i.e. a tap-selected
    /// atom), select its URL ([`Self::atom_edit_selection`]) — the selection
    /// endpoints inside the syntax reveal the source, and the likeliest edit
    /// becomes type-to-replace. Touch platforms trigger this from the edit
    /// menu's "Edit" action — with no arrow keys, there's otherwise no way to
    /// move the cursor into the syntax. True if handled (op pushed).
    pub fn enter_at_image(
        &self, root: &'ast AstNode<'ast>, operations: &mut Vec<Operation>,
    ) -> bool {
        let selection = self.renderer.buffer.current.selection;
        for node in root.descendants() {
            if image_url(node).is_none() {
                continue;
            }
            let img = self.renderer.node_range(node);
            if selection != img || self.renderer.range_revealed_interior(img) {
                continue;
            }
            operations.push(Operation::Select(self.atom_edit_selection(node).0));
            return true;
        }
        false
    }

    /// Backspace/forward-delete against a collapsed image selects it rather
    /// than erasing a character of its source; a second press then deletes the
    /// selection. Mirrors [`Self::delete_at_fold`]'s non-destructive edit. A
    /// note embed showing its tab, a diagram and a divider are such atoms too.
    /// True if handled (ops pushed).
    pub fn delete_at_image(
        &self, root: &'ast AstNode<'ast>, region: Region, operations: &mut Vec<Operation>,
    ) -> bool {
        let Region::SelectionOrAdvance {
            advance: Advance::Next(Bound::Word) | Advance::By(Increment::Char),
            backwards,
        } = region
        else {
            return false;
        };
        // selection must be empty
        let Some(offset) = self.renderer.selection_offset() else {
            return false;
        };
        let r = &self.renderer;
        let last = r.buffer.current.segs.last_cursor_position();
        let newline = |at: Grapheme| at < last && r.buffer[(at, at + 1)] == *"\n";
        let rules = root
            .descendants()
            .filter(|node| matches!(node.data.borrow().value, NodeValue::ThematicBreak))
            .map(|node| r.node_line(node, r.node_first_line(node)));
        // diagrams and dividers are blocks: the newline beside one is against it too
        let blocks = r.collapsed_diagrams().chain(rules);
        let diagrams =
            blocks.map(|d| (d, newline(d.end()), d.start() > 0 && newline(d.start() - 1)));
        let images = r.bounds.images.iter().copied();
        let embeds = root
            .descendants()
            .filter(|&node| r.tab_embed_height(node).is_some())
            .map(|node| r.node_range(node));
        let inline = images.chain(embeds).map(|img| (img, false, false));
        for (img, newline_after, newline_before) in inline.chain(diagrams) {
            if r.range_revealed_interior(img) {
                continue;
            }
            let against = if backwards {
                offset == img.end() || newline_after && offset == img.end() + 1
            } else {
                offset == img.start() || newline_before && offset + 1 == img.start()
            };
            if against {
                operations.push(Operation::Select(img));
                return true;
            }
        }
        false
    }
}

/// Explicit image dimensions in logical points; `None` means "use the
/// natural / fit-to-width size for that axis."
#[derive(Clone, Copy, Default)]
pub struct ImageDims {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

/// An image's `(ascent, descent)` about the text baseline, `baseline` of the
/// row down from its top: one up to two text rows tall centers on the text,
/// growing its row evenly; a taller one stands on the baseline.
pub fn small_image_metrics(height: f32, row_height: f32, baseline: f32) -> (f32, f32) {
    if height > 2.0 * row_height {
        return (height, 0.0);
    }
    let ascent = (baseline - 0.5) * row_height + height / 2.0;
    (ascent, height - ascent)
}

/// Parse Obsidian's `|W` / `|WxH` size suffix off an image's alt text. Only a
/// trailing segment of positive integers (optionally split by a single `x`)
/// counts — anything else leaves a literal `|` in the alt and yields no dims.
fn parse_image_dims(alt: &str) -> ImageDims {
    let Some(bar) = alt.rfind('|') else {
        return ImageDims::default();
    };
    let mut parts = alt[bar + 1..].split('x');
    let w = parts.next().unwrap_or("");
    let h = parts.next();
    if parts.next().is_some() {
        return ImageDims::default(); // more than one `x`
    }

    let dim = |s: &str| -> Option<f32> {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<u32>().ok())
            .flatten()
            .filter(|n| *n > 0)
            .map(|n| n as f32)
    };

    match h {
        None => match dim(w) {
            Some(width) => ImageDims { width: Some(width), height: None },
            None => ImageDims::default(),
        },
        // `WxH`: both must be valid, else the `|…` isn't a size spec
        Some(h) => match (dim(w), dim(h)) {
            (Some(width), Some(height)) => ImageDims { width: Some(width), height: Some(height) },
            _ => ImageDims::default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::parse_image_dims;

    #[test]
    fn obsidian_image_dims() {
        let wh = |s: &str| {
            let d = parse_image_dims(s);
            (d.width, d.height)
        };
        // valid specs
        assert_eq!(wh("alt|100x40"), (Some(100.0), Some(40.0)));
        assert_eq!(wh("alt|100"), (Some(100.0), None));
        assert_eq!(wh("|100x40"), (Some(100.0), Some(40.0))); // empty alt
        assert_eq!(wh("a|b|100"), (Some(100.0), None)); // only the last `|` segment
        // not a size: the `|` stays part of the alt
        assert_eq!(wh("alt"), (None, None));
        assert_eq!(wh("alt|caption"), (None, None));
        assert_eq!(wh("alt|100x"), (None, None)); // missing height
        assert_eq!(wh("alt|100xx40"), (None, None)); // two `x`
        assert_eq!(wh("alt|100X40"), (None, None)); // uppercase X isn't the separator
        assert_eq!(wh("alt|0"), (None, None)); // zero rejected
        assert_eq!(wh("alt|100x0"), (None, None)); // zero dim rejected
    }
}
