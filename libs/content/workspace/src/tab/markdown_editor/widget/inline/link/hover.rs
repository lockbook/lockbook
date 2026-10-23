//! The card a web link shows under a resting pointer on desktop: the site,
//! the page's title and description, its picture, and where the link goes.
//! Built from the same preview as the link's chip and block card.

use egui::{CornerRadius, Rect, Response, Sense, Ui, vec2};

use crate::file_cache::ResolvedLink;
use crate::link_preview::LinkMeta;
use crate::style::{Space, TypeRole};
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::link::LinkMetaLookup;
use crate::theme::palette_v2::ThemeExt as _;
use crate::widgets::GlyphonLabel;
use crate::widgets::TextOverflow;

/// Width of the card's content.
const CARD_W: f32 = 320.0;
/// A picture this wide or wider runs across the card; narrower ones are left out.
const BANNER_MIN_ASPECT: f32 = 1.4;
const BANNER_MAX_H: f32 = 180.0;
const FAVICON: f32 = 16.0;

impl MdRender {
    /// The preview card for a web link under the pointer. `shows_url`: the
    /// link's text is its URL, so a card with nothing fetched would only
    /// repeat it.
    pub(super) fn show_link_preview(
        &self, ctx: &egui::Context, response: &Response, url: &str, shows_url: bool,
    ) {
        if self.touch_mode {
            return;
        }
        let Some(ResolvedLink::External(target)) = self.resolve_link(url) else {
            return;
        };
        if !(target.starts_with("https://") || target.starts_with("http://")) {
            return;
        }
        let meta = match self.get_link_meta(url) {
            LinkMetaLookup::Loaded(meta) => Some(meta),
            _ => None,
        };
        if meta.is_none() && shows_url {
            return;
        }
        crate::style::tip_card_placed(ctx, response, response.rect, CARD_W, |ui| {
            ui.set_width(CARD_W);
            ui.spacing_mut().item_spacing.y = Space::Xs.pts();
            if let Some(meta) = &meta {
                self.preview_card_contents(ui, meta);
            }
            let shown = target
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_start_matches("www.")
                .trim_end_matches('/');
            ui.add(
                GlyphonLabel::new(shown, ui.ctx().get_lb_theme().neutral_fg_secondary())
                    .font_size(TypeRole::Mono.size())
                    .line_height(TypeRole::Mono.line_height())
                    .max_width(CARD_W)
                    .text_overflow(TextOverflow::EndEllipsis),
            );
        });
    }

    fn preview_card_contents(&self, ui: &mut Ui, meta: &LinkMeta) {
        let t = ui.ctx().get_lb_theme();

        if let Some(thumb) = meta.thumbnail_url.as_deref() {
            self.embeds.prefetch(thumb);
            let aspect = self.thumb_aspect(meta, thumb);
            if self.embeds.is_loaded(thumb) && aspect >= BANNER_MIN_ASPECT {
                let size = vec2(CARD_W, (CARD_W / aspect).min(BANNER_MAX_H));
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                self.paint_embed(ui, thumb, rect, CornerRadius::same(Space::Xs.pts() as u8));
            }
        }

        if let Some(site) = meta.site_name.as_deref() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = Space::Xs.pts();
                if let Some(favicon) = meta.favicon_url.as_deref() {
                    self.embeds.prefetch(favicon);
                    if self.embeds.is_loaded(favicon) {
                        let (rect, _) = ui.allocate_exact_size(
                            vec2(FAVICON, TypeRole::Body.line_height()),
                            Sense::hover(),
                        );
                        let icon = Rect::from_center_size(rect.center(), vec2(FAVICON, FAVICON));
                        // the same light tile as the chip's, for dark marks
                        if self.dark_mode {
                            ui.painter()
                                .rect_filled(icon, 3.0, egui::Color32::from_gray(235));
                        }
                        self.paint_embed(ui, favicon, icon, CornerRadius::same(3));
                    }
                }
                ui.add(body(site, t.neutral_fg_secondary(), false, 1));
            });
        }
        ui.add(body(&meta.title, t.neutral_fg(), true, 3));
        if let Some(description) = meta.description.as_deref() {
            ui.add(body(description, t.neutral_fg_secondary(), false, 3));
        }
    }

    /// Paints `url`'s image into `rect` without moving the layout cursor.
    fn paint_embed(&self, ui: &mut Ui, url: &str, rect: Rect, radius: CornerRadius) {
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(rect));
        self.embeds.show(ui, url, rect, radius);
    }
}

/// Body text that ends in an ellipsis past `lines` lines.
fn body(text: &str, color: egui::Color32, bold: bool, lines: usize) -> GlyphonLabel<'_> {
    GlyphonLabel::new_rich(vec![(text, bold)], color)
        .font_size(TypeRole::Body.size())
        .line_height(TypeRole::Body.line_height())
        .max_width(CARD_W)
        .max_lines(lines)
        .text_overflow(TextOverflow::EndEllipsis)
}
