use std::f32;

use comrak::nodes::{AstNode, NodeValue};
use egui::{self, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use lb_rs::model::text::operation_types::Operation;

use crate::file_cache::{ResolvedLink, wiki_fragment};
use crate::resolvers::image_embed::{wiki_embed_key, wiki_embed_title};
use crate::style::ThemeExt;
use crate::tab::image_viewer::is_supported_image_fmt;
use crate::tab::markdown_editor::TouchTarget;
use crate::tab::markdown_editor::input::{Advance, Bound, Event, Increment, Location, Region};
use crate::tab::markdown_editor::widget::block::leaf::diagram::Picture;
use crate::tab::markdown_editor::widget::inline::link::{LinkMenuAction, fill_link_menu};
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
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
        let mut width = self.width(block_ancestor);
        let in_cell = matches!(block_ancestor.data.borrow().value, NodeValue::TableCell);
        if in_cell {
            width = (width - 2.0 * self.layout.block_padding).max(0.0);
        }
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
        // Always interactive: a plain tap selects, a cmd/keyboard-hidden tap opens.
        layout.interaction_open(Self::image_interaction_id_salt(node_range), egui::Sense::click());
        let (ascent, descent) = small_image_metrics(size.y, self.row_height(node));
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

    /// Interaction-response salt for a rendered image; distinct from
    /// [`Self::link_interaction_id_salt`] so the collapsed-image and
    /// revealed-link handlers don't collide.
    pub fn image_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::unique(("md_image", node_range))
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
    }
}

impl<'ast> MdEdit {
    /// The gesture that *opens* an embed rather than selecting it: a cmd-click
    /// (desktop). Touch taps always select and pop the context menu, whose
    /// "Open Link" action opens. Shared by every embed kind.
    pub fn embed_tap_opens(&self, ui: &egui::Ui) -> bool {
        !self.renderer.touch_mode && ui.ctx().input(|i| i.modifiers.command)
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
        // a wiki embed's key holds its title
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
            // a block card already shows everything its preview would
            let card = !is_image && self.renderer.link_renders_as_card(node);
            let preview = (!is_image && !wikilink && !card).then_some((url, false));
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
            let refreshable = !is_image && self.renderer.contact_linked_sites;
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
            self.handle_embed_tap(root, node, ui, id, ops, &url, salt, open);
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
    /// selection. Mirrors [`Self::delete_at_fold`]'s non-destructive edit. True
    /// if handled (ops pushed).
    pub fn delete_at_image(&self, region: Region, operations: &mut Vec<Operation>) -> bool {
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
        // a diagram is a block: the newline beside it is against it too
        let diagrams = r
            .collapsed_diagrams()
            .map(|d| (d, newline(d.end()), d.start() > 0 && newline(d.start() - 1)));
        let images = r.bounds.images.iter().map(|&img| (img, false, false));
        for (img, newline_after, newline_before) in images.chain(diagrams) {
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

/// An image's `(ascent, descent)` about the text baseline: one up to two
/// text rows tall centers on the text, growing its row evenly; a taller one
/// stands on the baseline.
pub fn small_image_metrics(height: f32, row_height: f32) -> (f32, f32) {
    if height > 2.0 * row_height {
        return (height, 0.0);
    }
    let ascent = 0.3 * row_height + height / 2.0;
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
