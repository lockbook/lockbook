//! A web link's preview card: the hover tip on desktop, and the body a bare
//! link alone on its line shows as a block. Its picture runs edge to edge;
//! then where it stands, the site, the title, who, labels and description.

use egui::{Align2, Color32, CornerRadius, Painter, Rect, Response, Sense, Ui, vec2};

use super::{LinkMenuAction, fill_link_menu};
use crate::file_cache::ResolvedLink;
use crate::link_preview::{LinkMeta, Status};
use crate::style::button::{Button, icon_button};
use crate::style::phosphor::DOTS_THREE;
use crate::style::phosphor::{CHECK_CIRCLE, CIRCLE, GIT_MERGE, GIT_PULL_REQUEST, PROHIBIT};
use crate::style::{Pill, PillLead, Radius, Space, TypeRole, context_menu, phosphor_font_id};
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::inline::link::LinkMetaLookup;
use crate::theme::palette_v2::{Theme, ThemeExt as _};
use crate::widgets::GlyphonLabel;
use crate::widgets::TextOverflow;

/// Width of the hover card's content.
pub(crate) const CARD_W: f32 = 320.0;
/// Space between a card's edge and its content.
pub(crate) const CARD_PAD: Space = Space::Md;
pub(crate) const CARD_RADIUS: Radius = Radius::Control;
/// A picture this wide or wider runs across the card; narrower ones are left out.
const BANNER_MIN_ASPECT: f32 = 1.4;
const FAVICON: f32 = 16.0;
const AVATAR: f32 = 18.0;

impl MdRender {
    /// The preview card for a web link under the pointer, with the block
    /// card's buttons. `shows_url`: the link's text is its URL, so a card
    /// with nothing fetched would only repeat it. False when it has no card.
    pub(super) fn show_link_preview(
        &self, ctx: &egui::Context, response: &Response, url: &str, shows_url: bool, wikilink: bool,
    ) -> bool {
        if self.touch_mode {
            return false;
        }
        if let Some(place) = self.link_place(url, wikilink) {
            return self.show_tab_card(ctx, response, url, place);
        }
        if wikilink {
            return false;
        }
        let Some(ResolvedLink::External(target)) = self.resolve_link(url) else {
            return false;
        };
        if !(target.starts_with("https://") || target.starts_with("http://")) {
            return false;
        }
        let meta = match self.get_link_meta(url) {
            LinkMetaLookup::Loaded(meta) => Some(meta),
            _ => None,
        };
        if meta.is_none() && shows_url {
            return false;
        }
        crate::style::tip_card_reachable(
            ctx,
            response,
            response.rect,
            CARD_W,
            egui::Margin::same(CARD_PAD.pts() as i8),
            |ui| {
                ui.set_width(CARD_W);
                ui.spacing_mut().item_spacing.y = Space::Sm.pts();
                match &meta {
                    Some(meta) => {
                        if let Some(action) = self.card_body(ui, meta, &target, true) {
                            *self.card_action.borrow_mut() = Some((url.to_string(), action));
                            crate::style::dismiss_tip(ui.ctx());
                            // a chip's tip shows after the editor takes card actions
                            ui.ctx().request_repaint();
                        }
                    }
                    None => {
                        ui.add(
                            GlyphonLabel::new(&target, ink_secondary(&ui.ctx().get_lb_theme()))
                                .role(TypeRole::Mono)
                                .max_width(CARD_W)
                                .text_overflow(TextOverflow::EndEllipsis),
                        );
                    }
                }
            },
        );
        true
    }

    /// The card's content in `ui`, which sits [`CARD_PAD`] inside a card of
    /// [`CARD_RADIUS`] corners; the picture reaches past the pad to the edges.
    /// `target` is where the link goes, shown last; with `actions`, beside
    /// it an Open button and a `⋯` menu, whose choice this returns.
    pub(super) fn card_body(
        &self, ui: &mut Ui, meta: &LinkMeta, target: &str, actions: bool,
    ) -> Option<LinkMenuAction> {
        let t = ui.ctx().get_lb_theme();
        ui.spacing_mut().item_spacing.y = Space::Sm.pts();

        if let Some(thumb) = meta.thumbnail_url.as_deref() {
            self.embeds.prefetch(thumb);
            let aspect = self.thumb_aspect(meta, thumb);
            if self.embeds.is_loaded(thumb) && aspect >= BANNER_MIN_ASPECT {
                let pad = CARD_PAD.pts();
                let width = ui.available_width() + 2.0 * pad;
                let at = ui.cursor().min - vec2(pad, pad);
                let banner = Rect::from_min_size(at, vec2(width, width / aspect));
                ui.allocate_exact_size(
                    vec2(width - 2.0 * pad, banner.height() - pad),
                    Sense::hover(),
                );
                let r = CARD_RADIUS.pts();
                self.paint_embed(ui, thumb, banner, CornerRadius { nw: r, ne: r, sw: 0, se: 0 });
            }
        }

        // the site's icon leads the title
        ui.horizontal_top(|ui| {
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
            ui.add(body(&meta.title, t.neutral_fg(), true, 3).max_width(ui.available_width()));
        });
        if meta.byline.is_some() || meta.at.is_some() {
            self.byline(ui, meta, &t);
        }
        if let Some(description) = meta.description.as_deref() {
            ui.add(body(description, ink_secondary(&t), false, 3).max_width(ui.available_width()));
        }
        self.capsules(ui, meta, &t);
        let shown = target
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .trim_end_matches('/');
        let url = |ui: &mut Ui| {
            ui.add(
                GlyphonLabel::new(shown, ink_secondary(&t))
                    .role(TypeRole::Mono)
                    .max_width(ui.available_width())
                    .text_overflow(TextOverflow::EndEllipsis),
            );
        };
        if !actions {
            url(ui);
            return None;
        }
        let mut chosen = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = Space::Xs.pts();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let more = icon_button(ui, &t, DOTS_THREE, false, t.neutral_bg());
                let editable = !self.readonly;
                chosen = context_menu::show_click(&more, &t, |e| {
                    fill_link_menu(e, false, editable, true, |a| a)
                });
                let open = Button::secondary(&t, "Open").show(ui);
                if open.clicked() {
                    chosen = Some(LinkMenuAction::Open);
                }
                // controls keep the arrow over the page's text cursor
                if more.hovered() || open.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), url);
            });
        });
        chosen
    }

    /// The author's picture and name, and how long ago it came to stand where
    /// it does.
    fn byline(&self, ui: &mut Ui, meta: &LinkMeta, t: &Theme) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = Space::Xs.pts();
            if let Some(avatar) = meta.avatar_url.as_deref() {
                self.embeds.prefetch(avatar);
                if self.embeds.is_loaded(avatar) {
                    let h = TypeRole::Body.line_height();
                    let (rect, _) = ui.allocate_exact_size(vec2(AVATAR, h), Sense::hover());
                    let face = Rect::from_center_size(rect.center(), vec2(AVATAR, AVATAR));
                    self.paint_embed(ui, avatar, face, CornerRadius::same((AVATAR / 2.0) as u8));
                }
            }
            let ago = meta.at.map(ago);
            let text = [meta.byline.as_deref(), ago.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ");
            ui.add(body(&text, ink_secondary(t), false, 1).max_width(ui.available_width()));
        });
    }

    /// Where it stands, lines added and removed and the files they're in, then
    /// its labels: every pill, wrapping as one run.
    fn capsules(&self, ui: &mut Ui, meta: &LinkMeta, t: &Theme) {
        let changes = meta
            .changes
            .map(|(add, del)| (format!("+{}", grouped(add)), format!("−{}", grouped(del))));
        let files = meta.changed_files.map(|n| match n {
            1 => "1 file".to_string(),
            n => format!("{} files", grouped(n)),
        });
        if meta.status.is_none() && changes.is_none() && files.is_none() && meta.labels.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(Space::Xs.pts(), Space::Xs.pts());
            if let Some(status) = meta.status {
                let (_, color) = status_mark(status, t);
                let wash = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 36);
                let mark = |p: &Painter, r: Rect| paint_status(p, r, status, t);
                ui.add(Pill::new(vec![(status.label(), color)], wash).lead(PillLead::Mark(&mark)));
            }
            let quiet = t.neutral_bg_secondary();
            if let Some((add, del)) = &changes {
                let spans = vec![
                    (add.as_str(), t.fg().green),
                    (" ", t.neutral_fg()),
                    (del.as_str(), t.fg().red),
                ];
                ui.add(Pill::new(spans, quiet));
            }
            if let Some(files) = &files {
                ui.add(Pill::new(vec![(files.as_str(), ink_secondary(t))], quiet));
            }
            for l in &meta.labels {
                let pill = Pill::new(vec![(l.name.as_str(), ink_secondary(t))], quiet);
                ui.add(match l.rgb {
                    Some([r, g, b]) => pill.lead(PillLead::Dot(Color32::from_rgb(r, g, b))),
                    None => pill,
                });
            }
        });
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
        .max_lines(lines)
        .text_overflow(TextOverflow::EndEllipsis)
}

/// The mark for where an issue, pull request, or question stands, in its color.
fn status_mark(status: Status, t: &Theme) -> (&'static str, Color32) {
    let (fg, quiet) = (t.fg(), ink_secondary(t));
    match status {
        Status::IssueOpen => (CIRCLE, fg.green),
        Status::IssueClosed => (CHECK_CIRCLE, fg.magenta),
        Status::IssueNotPlanned => (PROHIBIT, quiet),
        Status::PullOpen => (GIT_PULL_REQUEST, fg.green),
        Status::PullDraft => (GIT_PULL_REQUEST, quiet),
        Status::PullMerged => (GIT_MERGE, fg.magenta),
        Status::PullClosed => (GIT_PULL_REQUEST, fg.red),
        Status::Answered => (CHECK_CIRCLE, fg.green),
    }
}

/// `status`'s mark filling `rect`; an open issue's circle gets its dot.
pub(super) fn paint_status(painter: &Painter, rect: Rect, status: Status, t: &Theme) {
    let (glyph, color) = status_mark(status, t);
    let size = rect.height();
    if status == Status::IssueOpen {
        // ring and dot drawn on one center, on Phosphor's 256-unit grid:
        // the font's circle centers by its line box, not its outline
        let unit = size / 256.0;
        let stroke = egui::Stroke::new(16.0 * unit, color);
        painter.circle_stroke(rect.center(), 96.0 * unit, stroke);
        painter.circle_filled(rect.center(), size * 0.13, color);
        return;
    }
    painter.text(rect.center(), Align2::CENTER_CENTER, glyph, phosphor_font_id(size), color);
}

/// 17831 → "17,831".
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// How long ago `at` (Unix seconds) was, in its largest whole unit: "2y ago".
fn ago(at: i64) -> String {
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(at, |d| d.as_secs() as i64);
    let s = (now - at).max(0);
    let (n, unit) = match s {
        0..60 => return "now".into(),
        60..3_600 => (s / 60, "m"),
        3_600..86_400 => (s / 3_600, "h"),
        86_400..2_592_000 => (s / 86_400, "d"),
        2_592_000..31_536_000 => (s / 2_592_000, "mo"),
        _ => (s / 31_536_000, "y"),
    };
    format!("{n}{unit} ago")
}

/// The palette's secondary neutral, 800 on light and 200 on dark: the
/// quietest grey its rules allow on the card's 50 / 950 background.
fn ink_secondary(t: &Theme) -> Color32 {
    let toward_bg = if t.light() { 0.29 } else { 0.15 };
    t.neutral_fg().lerp_to_gamma(t.neutral_bg(), toward_bg)
}
