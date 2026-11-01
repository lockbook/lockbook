//! Link-preview **cards**: a qualifying bare URL ([`MdRender::link_renders_as_card`])
//! renders as a block card on its own row, reusing the image embed slot
//! ([`EmbedKind::LinkCard`]) and its hit-test / reveal / selection machinery.
//! Its body is the hover card's ([`MdRender::card_body`]), laid out off screen
//! for the height to reserve; a skeleton holds the space until the fetch lands
//! (height re-stamped via the previews' seq).

use comrak::nodes::{AstNode, NodeValue};
use egui::{CornerRadius, Rect, Sense, UiBuilder, Vec2, pos2, vec2};
use lb_rs::model::text::offset_types::Grapheme;
use lb_rs::model::text::operation_types::Operation;

use super::LinkMenuAction;
use super::hover::{CARD_PAD, CARD_RADIUS, CARD_W};
use crate::link_preview::LinkMeta;
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::inline::link::LinkMetaLookup;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{EmbedKind, EmbedSpec, Layout};
use crate::tab::markdown_editor::{MdEdit, MdRender};
use crate::theme::palette_v2::ThemeExt as _;

const SKELETON_HEIGHT: f32 = 84.0;

impl<'ast> MdRender {
    fn card_box_width(&self, block_width: f32) -> f32 {
        // the hover card's width: one card, resting or hovered
        block_width.clamp(0.0, CARD_W + 2.0 * CARD_PAD.pts())
    }

    /// The thumbnail's width/height ratio: declared `og:image` dims first (known
    /// before the texture loads, so no reflow), else the decoded texture's real
    /// aspect, else square.
    pub(super) fn thumb_aspect(&self, meta: &LinkMeta, thumb: &str) -> f32 {
        if let (Some(w), Some(h)) = (meta.thumbnail_width, meta.thumbnail_height) {
            if w > 0 && h > 0 {
                return w as f32 / h as f32;
            }
        }
        let s = self.embeds.size(thumb);
        if s.x > 0.0 && s.y > 0.0 { s.x / s.y } else { 1.0 }
    }

    /// Where a card's link goes, as its last line shows it.
    fn card_target(&self, url: &str) -> String {
        match self.resolve_link(url) {
            Some(crate::file_cache::ResolvedLink::External(target)) => target,
            _ => url.into(),
        }
    }

    /// How tall `meta`'s card is at `box_w`, laid out for real off screen.
    fn card_height(&self, meta: &LinkMeta, url: &str, box_w: f32) -> f32 {
        let pad = CARD_PAD.pts();
        let far = pos2(-100_000.0, -100_000.0);
        let room = Rect::from_min_size(far, vec2(box_w - 2.0 * pad, 100_000.0));
        let id = egui::Id::unique("md_link_card_measure");
        let mut ui =
            egui::Ui::new(self.ctx.clone(), id, UiBuilder::new().max_rect(room).invisible());
        self.card_body(&mut ui, meta, &self.card_target(url), !self.touch_mode);
        ui.min_rect().height() + 2.0 * pad
    }

    /// Logical-point box a link card reserves, or `None` when the link
    /// shouldn't card right now (no metadata and either fetching off or an
    /// internal link). A pending fetch reserves a fixed skeleton box.
    fn card_logical_size(&self, node: &'ast AstNode<'ast>, url: &str) -> Option<Vec2> {
        let block_ancestor = node
            .ancestors()
            .skip(1)
            .find(|a| a.data.borrow().value.is_leaf_block())?;
        let box_w = self.card_box_width(self.width(block_ancestor));
        if box_w <= 0.0 {
            return None;
        }
        match self.get_link_meta(url) {
            LinkMetaLookup::Loaded(meta) => Some(vec2(box_w, self.card_height(&meta, url, box_w))),
            // Skeleton only while fetching; failed/unavailable yields `None` so
            // the link degrades to a plain link rather than a stuck skeleton.
            LinkMetaLookup::Loading => Some(vec2(box_w, SKELETON_HEIGHT)),
            LinkMetaLookup::Internal(_) | LinkMetaLookup::Unavailable => None,
        }
    }

    pub fn card_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::unique(("md_link_card", node_range))
    }

    /// Emit a card embed for a qualifying link; `false` (caller renders the
    /// inline link) when it doesn't card or has nothing to show yet. Caller
    /// guarantees the cursor isn't inside and the node fits this wrap unit.
    pub fn layout_link_card(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, url: &str,
    ) -> bool {
        if !self.link_renders_as_card(node) {
            return false;
        }
        let Some(size) = self.card_logical_size(node, url) else {
            return false;
        };
        let node_range = self.node_range(node);
        layout.interaction_open(Self::card_interaction_id_salt(node_range), Sense::click());
        layout.push_embed(EmbedSpec {
            advance: size.x,
            ascent: size.y,
            descent: 0.0,
            source_range: node_range,
            url: url.to_owned(),
            kind: EmbedKind::LinkCard,
        });
        layout.interaction_close();
        true
    }

    /// Paint a link card into `rect` (the slot the layout reserved): the hover
    /// card's body resting on the page. A skeleton holds it while fetching.
    pub fn paint_link_card(&self, ui: &mut egui::Ui, url: &str, rect: Rect) {
        let t = ui.ctx().get_lb_theme();
        match self.get_link_meta(url) {
            LinkMetaLookup::Loaded(meta) => {
                // non-advancing child ui — see the embed paint in
                // `show_wrap_layout` (#4892)
                let body = rect.shrink(CARD_PAD.pts());
                let ui = &mut ui.new_child(UiBuilder::new().max_rect(body));
                // on touch a tap selects the card and brings up the native menu
                let actions = !self.touch_mode;
                if let Some(action) = self.card_body(ui, &meta, &self.card_target(url), actions) {
                    *self.card_action.borrow_mut() = Some((url.to_string(), action));
                }
            }
            _ => paint_skeleton(ui, rect),
        }
        // over the picture, which runs to the card's edges
        crate::style::paint_doc_border(ui.painter(), rect, CARD_RADIUS.corner(), &t);
    }
}

impl<'ast> MdEdit {
    /// Open or select a clicked link card. Classification differs from images (a
    /// `Link` that `link_renders_as_card`); tap handling is shared via
    /// [`MdEdit::handle_embed_tap`].
    pub fn handle_card_interactions(
        &mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui, id: egui::Id, ops: &mut Vec<Operation>,
    ) {
        let open = self.embed_tap_opens(ui);
        let mut action = self.renderer.card_action.take();
        for node in root.descendants() {
            let (url, wikilink) = match &node.data.borrow().value {
                NodeValue::Link(link) => (link.url.clone(), false),
                NodeValue::WikiLink(link) => (link.url.clone(), true),
                _ => continue,
            };
            // a block card's or a hover card's buttons
            if let Some((_, chosen)) = action.take_if(|(for_url, _)| *for_url == url) {
                self.act_on_card(root, node, ui, ops, &url, chosen);
            }
            if wikilink || !self.renderer.link_renders_as_card(node) {
                continue;
            }
            let salt = MdRender::card_interaction_id_salt(self.renderer.node_range(node));
            self.handle_embed_tap(root, node, ui, id, ops, &url, salt, open);
        }
    }

    /// A card's Open button or `⋯` menu choice, as the link menu does it.
    fn act_on_card(
        &mut self, root: &'ast AstNode<'ast>, node: &'ast AstNode<'ast>, ui: &egui::Ui,
        ops: &mut Vec<Operation>, url: &str, action: LinkMenuAction,
    ) {
        let wikilink = matches!(node.data.borrow().value, NodeValue::WikiLink(_));
        match action {
            LinkMenuAction::Open if wikilink => self.renderer.open_wikilink(url, ui.ctx(), false),
            LinkMenuAction::Open => self.renderer.open_resolved_link(url, ui.ctx(), false),
            LinkMenuAction::Copy => ui
                .ctx()
                .copy_text(self.renderer.link_to_copy(url, wikilink)),
            LinkMenuAction::Refresh => self.renderer.refresh_link_meta(url),
            LinkMenuAction::Edit => {
                let (select, force_reveal) = self.atom_edit_selection(node);
                if force_reveal {
                    self.renderer.entered_atom = Some(self.renderer.node_range(node));
                }
                let select = Event::Select {
                    region: Region::BetweenLocations {
                        start: Location::Grapheme(select.0),
                        end: Location::Grapheme(select.1),
                    },
                };
                self.calc_operations(ui.ctx(), root, select, ops);
            }
        }
    }

    /// Open or select a link rendered as a capsule preview — the inline-link
    /// counterpart to [`Self::handle_card_interactions`]. Only links that
    /// rendered as a capsule have a registered response.
    pub fn handle_link_capsule_interactions(
        &mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui, id: egui::Id, ops: &mut Vec<Operation>,
    ) {
        let open = self.embed_tap_opens(ui);
        for node in root.descendants() {
            let url = match &node.data.borrow().value {
                NodeValue::Link(link) => link.url.clone(),
                _ => continue,
            };
            let salt = MdRender::link_capsule_id_salt(self.renderer.node_range(node));
            self.handle_embed_tap(root, node, ui, id, ops, &url, salt, open);
        }
    }

    /// If the selection exactly covers a link or wikilink, move it inside
    /// for editing via [`MdEdit::atom_edit_selection`]. The touch edit
    /// menu's "Edit" lands here. True if handled.
    pub fn enter_at_link(
        &mut self, root: &'ast AstNode<'ast>, operations: &mut Vec<Operation>,
    ) -> bool {
        let selection = self.renderer.buffer.current.selection;
        for node in root.descendants() {
            if !matches!(node.data.borrow().value, NodeValue::Link(_) | NodeValue::WikiLink(_)) {
                continue;
            }
            let node_range = self.renderer.node_range(node);
            if selection != node_range {
                continue;
            }
            let (select, force_reveal) = self.atom_edit_selection(node);
            if force_reveal {
                self.renderer.entered_atom = Some(node_range);
            }
            operations.push(Operation::Select(select));
            return true;
        }
        false
    }
}

/// Two faint bars standing in for the title/description while metadata loads.
fn paint_skeleton(ui: &egui::Ui, rect: Rect) {
    let color = ui.style().visuals.text_color().gamma_multiply(0.12);
    let cr = CornerRadius::same(3);
    let pad = CARD_PAD.pts();
    let bar = |y: f32, w: f32, h: f32| {
        let min = pos2(rect.min.x + pad, rect.min.y + y);
        ui.painter()
            .rect_filled(Rect::from_min_size(min, vec2(w, h)), cr, color);
    };
    let inner = (rect.width() - 2.0 * pad).max(0.0);
    bar(pad, inner * 0.5, 12.0);
    bar(pad + 22.0, inner, 10.0);
    bar(pad + 40.0, inner * 0.8, 10.0);
}
