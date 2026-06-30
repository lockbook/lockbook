//! Link-preview **cards**: a qualifying bare URL ([`MdRender::link_renders_as_card`])
//! renders as a block card on its own row, reusing the image embed slot
//! ([`EmbedKind::LinkCard`]) and its hit-test / reveal / selection machinery.
//! Sizer ([`MdRender::card_metrics`]) and painter ([`MdRender::paint_link_card`])
//! share metrics so the reserved height matches what's drawn; a skeleton holds
//! the space until the fetch lands (height re-stamped via `link_seq`).

use std::sync::Arc;

use comrak::nodes::{AstNode, NodeValue};
use egui::epaint::text::Galley;
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use lb_rs::model::text::offset_types::Grapheme;
use lb_rs::model::text::operation_types::Operation;

use crate::tab::markdown_editor::widget::inline::link::LinkMetaLookup;
use crate::tab::markdown_editor::widget::inline::link_meta::LinkMeta;
use crate::tab::markdown_editor::widget::utils::NodeValueExt as _;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{EmbedKind, EmbedSpec, Layout};
use crate::tab::markdown_editor::{MdEdit, MdRender};

const MAX_WIDTH: f32 = 560.0;
const PAD: f32 = 12.0;
const GAP: f32 = 4.0;
const HERO_MIN_ASPECT: f32 = 1.4; // wider than this → full-width hero, else side thumbnail
const HERO_MIN_HEIGHT: f32 = 120.0;
const HERO_MAX_HEIGHT: f32 = 280.0;
const THUMB: f32 = 80.0; // square side-thumbnail box for the horizontal form
const GAP_H: f32 = 12.0; // gap between side thumbnail and text
const SKELETON_HEIGHT: f32 = 84.0;
const RADIUS: u8 = 12;
const SITE_SIZE: f32 = 12.0;
const TITLE_SIZE: f32 = 16.0;
const DESC_SIZE: f32 = 13.0;

/// Resolved card geometry, box-relative (origin at the card's top-left). Shared
/// by the sizer and the painter so reserved height == painted height. `image`
/// is the final, aspect-correct draw rect (hero band on top, or side thumbnail).
struct CardMetrics {
    size: Vec2,
    image: Option<(Rect, String)>,
    site: Option<(Pos2, Arc<Galley>)>,
    title: Option<(Pos2, Arc<Galley>)>,
    desc: Option<(Pos2, Arc<Galley>)>,
}

/// Aspect-preserving fit (`a` = width / height) centered in `box_size`, box-
/// relative. `embeds.show` stretches a texture to its rect, so this is what keeps
/// a non-1.91 image (e.g. a square logo) from distorting.
fn contain(box_size: Vec2, a: f32) -> Rect {
    let by_width = vec2(box_size.x, box_size.x / a);
    let fit = if by_width.y <= box_size.y { by_width } else { vec2(box_size.y * a, box_size.y) };
    Rect::from_min_size(((box_size - fit) / 2.0).to_pos2(), fit)
}

impl<'ast> MdRender {
    fn card_box_width(&self, block_width: f32) -> f32 {
        block_width.clamp(0.0, MAX_WIDTH)
    }

    /// The thumbnail's width/height ratio: declared `og:image` dims first (known
    /// before the texture loads, so no reflow), else the decoded texture's real
    /// aspect, else square.
    fn thumb_aspect(&self, meta: &LinkMeta, thumb: &str) -> f32 {
        if let (Some(w), Some(h)) = (meta.thumbnail_width, meta.thumbnail_height) {
            if w > 0 && h > 0 {
                return w as f32 / h as f32;
            }
        }
        let s = self.embeds.size(thumb);
        if s.x > 0.0 && s.y > 0.0 { s.x / s.y } else { 1.0 }
    }

    fn card_metrics(&self, block_width: f32, meta: &LinkMeta) -> CardMetrics {
        let box_w = self.card_box_width(block_width);

        let text_color = self.ctx.style().visuals.text_color();
        let muted = text_color.gamma_multiply(0.6);
        let galley =
            |text: &str, size: f32, color: Color32, rows: usize, wrap_w: f32| -> Arc<Galley> {
                let mut job = LayoutJob::single_section(
                    text.to_owned(),
                    TextFormat { font_id: FontId::proportional(size), color, ..Default::default() },
                );
                job.wrap.max_width = wrap_w.max(0.0);
                job.wrap.max_rows = rows;
                job.wrap.overflow_character = Some('…');
                self.ctx.fonts(|f| f.layout_job(job))
            };

        // Warm so the texture (hence real aspect) loads before the first paint.
        let thumb = meta.thumbnail_url.as_deref();
        if let Some(t) = thumb {
            self.embeds.warm(t);
        }

        // Hero (full-width landscape band) vs. horizontal (square/portrait
        // thumbnail beside text) vs. text-only.
        let mut image = None;
        let mut text_x = PAD;
        let mut text_w = box_w - 2.0 * PAD;
        let mut y = PAD;
        let mut min_h = 0.0;
        if let Some(t) = thumb {
            let a = self.thumb_aspect(meta, t);
            if a >= HERO_MIN_ASPECT {
                let band_h = (box_w / a).clamp(HERO_MIN_HEIGHT, HERO_MAX_HEIGHT);
                image = Some((contain(vec2(box_w, band_h), a), t.to_owned()));
                y = band_h + PAD;
            } else {
                let rect = contain(Vec2::splat(THUMB), a).translate(vec2(PAD, PAD));
                image = Some((rect, t.to_owned()));
                text_x = PAD + THUMB + GAP_H;
                text_w = box_w - text_x - PAD;
                min_h = 2.0 * PAD + THUMB;
            }
        }

        let site = meta
            .site_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| {
                let g = galley(s, SITE_SIZE, muted, 1, text_w);
                let pos = pos2(text_x, y);
                y += g.size().y + GAP;
                (pos, g)
            });

        let title = (!meta.title.is_empty()).then(|| {
            let g = galley(&meta.title, TITLE_SIZE, text_color, 2, text_w);
            let pos = pos2(text_x, y);
            y += g.size().y;
            (pos, g)
        });

        let desc = meta
            .description
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|d| {
                y += GAP;
                let g = galley(d, DESC_SIZE, muted, 2, text_w);
                let pos = pos2(text_x, y);
                y += g.size().y;
                (pos, g)
            });

        let height = (y + PAD).max(min_h);
        CardMetrics { size: vec2(box_w, height), image, site, title, desc }
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
            LinkMetaLookup::External(Some(meta)) => {
                Some(self.card_metrics(self.width(block_ancestor), &meta).size)
            }
            LinkMetaLookup::External(None) if self.fetch_link_previews => {
                Some(vec2(box_w, SKELETON_HEIGHT))
            }
            _ => None,
        }
    }

    pub fn card_interaction_id_salt(node_range: (Grapheme, Grapheme)) -> egui::Id {
        egui::Id::new(("md_link_card", node_range))
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

    /// Paint a link card into `rect` (the slot the layout reserved). Falls back
    /// to a skeleton while the fetch is in flight.
    pub fn paint_link_card(&self, ui: &mut egui::Ui, url: &str, rect: Rect) {
        let vis = ui.style().visuals.clone();
        let cr = CornerRadius::same(RADIUS);
        let border = Stroke::new(1.0, vis.widgets.noninteractive.bg_stroke.color);
        ui.painter()
            .rect(rect, cr, vis.faint_bg_color, border, StrokeKind::Inside);

        let LinkMetaLookup::External(Some(meta)) = self.get_link_meta(url) else {
            paint_skeleton(ui, rect);
            return;
        };

        let m = self.card_metrics(rect.width(), &meta);
        let origin = rect.min.to_vec2();
        if let Some((img_rect, thumb)) = &m.image {
            self.embeds.show(ui, thumb, img_rect.translate(origin));
        }
        for (pos, galley) in [&m.site, &m.title, &m.desc].into_iter().flatten() {
            ui.painter()
                .galley(*pos + origin, galley.clone(), Color32::PLACEHOLDER);
        }
    }
}

impl<'ast> MdEdit {
    /// Open or select a clicked link card. Classification differs from images (a
    /// `Link` that `link_renders_as_card`); tap handling is shared via
    /// [`MdEdit::handle_embed_tap`].
    pub fn handle_card_interactions(
        &mut self, root: &'ast AstNode<'ast>, ui: &egui::Ui, id: egui::Id, keyboard_visible: bool,
        ops: &mut Vec<Operation>,
    ) {
        let open = self.embed_tap_opens(ui, keyboard_visible);
        for node in root.descendants() {
            let url = match &node.data.borrow().value {
                NodeValue::Link(link) => link.url.clone(),
                _ => continue,
            };
            if !self.renderer.link_renders_as_card(node) {
                continue;
            }
            let node_range = self.renderer.node_range(node);
            let salt = MdRender::card_interaction_id_salt(node_range);
            self.handle_embed_tap(root, ui, id, ops, node_range, &url, salt, open);
        }
    }
}

/// Two faint bars standing in for the title/description while metadata loads.
fn paint_skeleton(ui: &egui::Ui, rect: Rect) {
    let color = ui.style().visuals.text_color().gamma_multiply(0.12);
    let cr = CornerRadius::same(3);
    let bar = |y: f32, w: f32, h: f32| {
        let min = pos2(rect.min.x + PAD, rect.min.y + y);
        ui.painter()
            .rect_filled(Rect::from_min_size(min, vec2(w, h)), cr, color);
    };
    let inner = (rect.width() - 2.0 * PAD).max(0.0);
    bar(PAD, inner * 0.5, 12.0);
    bar(PAD + 22.0, inner, 10.0);
    bar(PAD + 40.0, inner * 0.8, 10.0);
}
