//! The outline: the note's headings, indented by level. With a mouse it is
//! a card docked beside the note's text, floating over the note's right edge
//! where the window is too narrow; on touch, a sheet from the bottom with
//! rows sized for a thumb. A row jumps to its heading, closing a floating
//! card or sheet. The current section is marked, and brought into view only
//! when out of it.

use std::sync::Arc;

use egui::text::{LayoutJob, TextWrapping};
use egui::{Align2, Area, Color32, Galley, Id, Order, Rect, Sense, Ui, pos2, vec2};
use lb_rs::model::text::offset_types::Grapheme;

use crate::style::sheet::sheet_dim;
use crate::style::{
    CHROME_BAND_GLYPH, EDGE_FADE, FG_HOVER, Radius, Space, ThemeExt as _, TypeRole, control_height,
    icon_button_glyph, overlay_scroll, paint_fade, phosphor, pin_shadow, tip_text,
};

/// How long the card takes to fade and the text to slide beside it.
pub const MOTION_SECS: f32 = 0.2;
/// A reveal eases the scroll with this time constant, settling within
/// [`MOTION_SECS`].
const SCROLL_TAU: f32 = MOTION_SECS / 4.0;
const SHEET_MAX_W: f32 = 480.0;
/// A thumb's row: Apple's 44 pt minimum touch target.
const TOUCH_ROW_H: f32 = 44.0;

/// A heading as the outline lists it.
pub struct OutlineRow {
    pub level: u8,
    pub text: String,
    pub slug: String,
    /// Where the heading starts, to tell which section is current.
    pub start: Grapheme,
    /// Where its text starts and ends, to find where it's drawn.
    pub span: (Grapheme, Grapheme),
}

/// Where the card stands: `area` bounds it, and `floating` puts it over the
/// note rather than beside the text. `shown` is how far it has faded in;
/// closing, it is not `open` and takes no clicks.
#[derive(Clone, Copy)]
pub struct CardPlace {
    pub area: Rect,
    pub floating: bool,
    pub shown: f32,
    pub open: bool,
}

#[derive(Default)]
pub struct Outline {
    sheet_open: bool,
    /// The sheet's first frame open, when the opening tap is not yet a tap
    /// outside.
    sheet_opening: bool,
    /// The current row last shown on the card, to follow it when it changes.
    card_current: Option<usize>,
    /// How far the rows are scrolled, in points from the top.
    scroll: f32,
    /// Where a reveal is easing the scroll to; the user's own scroll ends it.
    scroll_to: Option<f32>,
    /// The pass the rows were last drawn in, so they open already scrolled.
    drawn_pass: u64,
}

impl Outline {
    pub fn is_open(&self) -> bool {
        self.sheet_open
    }

    /// Opened since the last frame.
    pub fn is_opening(&self) -> bool {
        self.sheet_opening
    }

    /// Opens or closes the sheet from native chrome.
    pub fn toggle(&mut self) {
        self.sheet_open = !self.sheet_open;
        self.sheet_opening = self.sheet_open;
    }

    /// The sheet, if open. Returns the slug of a chosen row and the rect
    /// that takes touches.
    pub fn show(
        &mut self, ctx: &egui::Context, id: Id, rows: &[OutlineRow], current: Option<usize>,
    ) -> (Option<String>, Option<Rect>) {
        if !self.sheet_open {
            return (None, None);
        }
        let (chosen, rect, dismissed) = self.sheet(ctx, id, rows, current);
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if chosen.is_some() || dismissed || escape {
            self.sheet_open = false;
        }
        self.sheet_opening = false;
        (chosen, Some(rect))
    }

    /// A sheet from the bottom of the screen over a dimmed note, drawn as
    /// the desktop card is, with a close button.
    fn sheet(
        &mut self, ctx: &egui::Context, id: Id, rows: &[OutlineRow], current: Option<usize>,
    ) -> (Option<String>, Rect, bool) {
        let t = ctx.get_lb_theme();
        let screen = ctx.screen_rect();
        let layer = egui::LayerId::new(Order::Foreground, id);
        let mut dismissed = sheet_dim(ctx, id.with("dim"), layer) && !self.sheet_opening;
        let (margin, pad, header) = (Space::Md.pts(), Space::Sm.pts(), control_height());
        let width = (screen.width() - 2.0 * margin).min(SHEET_MAX_W);
        let inner_w = width - 2.0 * pad;
        // clear of the home indicator or gesture bar the page runs under
        let bottom = Space::Xl.pts();
        let max_list = screen.height() * 0.6;

        let measured = measure(ctx, rows, current, inner_w, true);
        let empty = rows.is_empty().then(|| empty_state(ctx, inner_w));
        let list_h = match &empty {
            Some(empty) => empty_height(empty),
            None => measured.height().min(max_list),
        };
        let size = vec2(width, header + list_h + 2.0 * pad);
        let sheet = Rect::from_min_size(
            pos2(screen.center().x - width / 2.0, screen.bottom() - bottom - size.y),
            size,
        );

        let mut chosen = None;
        Area::new(id)
            .order(Order::Foreground)
            .fixed_pos(sheet.min)
            .constrain(false)
            .fade_in(false)
            .show(ctx, |ui| {
                ui.allocate_rect(sheet, Sense::hover());
                paint_frame(ui, sheet, None);
                let close = Rect::from_min_size(
                    pos2(sheet.right() - pad - header, sheet.top() + pad),
                    vec2(header, header),
                );
                let ground = t.neutral_bg_secondary();
                let x = icon_button_glyph(
                    &mut ui.new_child(egui::UiBuilder::new().max_rect(close)),
                    &t,
                    phosphor::X,
                    false,
                    ground,
                    header,
                    CHROME_BAND_GLYPH,
                );
                if x.clicked() {
                    dismissed = true;
                }
                let view = Rect::from_min_size(
                    pos2(sheet.left() + pad, sheet.top() + pad + header),
                    vec2(inner_w, list_h),
                );
                match empty {
                    Some(empty) => paint_empty_state(ui, view, empty),
                    None => chosen = self.list(ui, id, view, &measured, ground, false),
                }
            });
        // the dimmed note takes taps too, which dismiss
        (chosen, screen, dismissed)
    }

    /// The card at `place`, fading in or out. It is as tall as its rows, up
    /// to `area`, scrolling inside when they don't fit. Floating, it closes
    /// on a click elsewhere in `canvas` or Escape. Returns the choice, the
    /// card, and whether it was dismissed.
    pub fn card(
        &mut self, ctx: &egui::Context, id: Id, place: Option<CardPlace>, canvas: Rect,
        rows: &[OutlineRow], current: Option<usize>,
    ) -> (Option<String>, Option<Rect>, bool) {
        let Some(CardPlace { area, floating, shown, open }) = place else {
            return (None, None, false);
        };
        let t = ctx.get_lb_theme();
        let ground = t.neutral_bg_secondary();
        let pad = Space::Sm.pts();
        let header = control_height();
        let reveal = self.card_current != current;
        self.card_current = current;

        let mut chosen = None;
        let mut card = Rect::NOTHING;
        Area::new(id)
            .order(Order::Middle)
            .fixed_pos(area.min)
            .constrain(false)
            .fade_in(false)
            .sense(Sense::hover())
            .show(ctx, |ui| {
                ui.multiply_opacity(shown);
                let inner_w = area.width() - 2.0 * pad;
                let measured = measure(ctx, rows, current, inner_w, false);
                let empty = rows.is_empty().then(|| empty_state(ctx, inner_w));
                let max_list = (area.height() - header - 2.0 * pad).max(control_height());
                let list_h = match &empty {
                    Some(empty) => empty_height(empty),
                    None => measured.height().min(max_list),
                };
                card =
                    Rect::from_min_size(area.min, vec2(area.width(), header + list_h + 2.0 * pad));
                ui.allocate_rect(card, Sense::hover());

                let shadow = floating.then(|| pin_shadow(&t));
                paint_frame(ui, card, shadow);
                let view = Rect::from_min_size(
                    pos2(card.left() + pad, card.top() + pad + header),
                    vec2(inner_w, list_h),
                );
                match empty {
                    Some(empty) => paint_empty_state(ui, view, empty),
                    None => chosen = self.list(ui, id, view, &measured, ground, reveal),
                }
            });
        if !open {
            return (None, Some(card), false);
        }
        let click_elsewhere = ctx.input(|i| {
            i.pointer.any_click()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| canvas.contains(p) && !card.contains(p))
        });
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let dismissed = floating && (chosen.is_some() || click_elsewhere || escape);
        (chosen, Some(card), dismissed)
    }

    /// Draws `measured` rows in `view` over `ground`, scrolled by the wheel, a
    /// drag, or the bar. Rows fade out at an edge with more beyond it, and
    /// `reveal` eases the current row into view clear of the fades; a list
    /// just opened starts there. Returns the slug of a chosen row.
    fn list(
        &mut self, ui: &mut Ui, id: Id, view: Rect, measured: &Measured, ground: Color32,
        reveal: bool,
    ) -> Option<String> {
        let t = ui.ctx().get_lb_theme();
        let fade = EDGE_FADE.pts();
        let content = measured.height();
        let max_scroll = (content - view.height()).max(0.0);

        let pass = ui.ctx().cumulative_pass_nr();
        let opening = pass > self.drawn_pass + 1;
        self.drawn_pass = pass;

        let drag = ui.interact(view, id.with("drag"), Sense::drag());
        let before = self.scroll;
        if ui.rect_contains_pointer(view) {
            self.scroll -= ui.input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta.y));
        }
        if drag.dragged() {
            self.scroll -= drag.drag_delta().y;
        }
        let user_scrolled = self.scroll != before;
        if user_scrolled {
            self.scroll_to = None;
        }
        // the current row clear of the fades, from where the scroll is headed
        if let Some(i) = measured.current.filter(|_| reveal || opening) {
            let (top, bottom) = (measured.tops[i], measured.tops[i] + measured.rows[i].height);
            let headed = self.scroll_to.unwrap_or(self.scroll);
            let to = if top - fade < headed {
                top - fade
            } else if bottom + fade > headed + view.height() {
                bottom + fade - view.height()
            } else {
                headed
            };
            self.scroll_to = Some(to.clamp(0.0, max_scroll));
        }
        if let Some(to) = self.scroll_to {
            let dt = ui.input(|i| i.stable_dt).min(0.1);
            let step = 1.0 - (-dt / SCROLL_TAU).exp();
            self.scroll += (to - self.scroll) * if opening { 1.0 } else { step };
            if (to - self.scroll).abs() < 0.5 {
                self.scroll = to;
                self.scroll_to = None;
            } else {
                ui.ctx().request_repaint();
            }
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);

        let painter = ui.painter_at(view);
        let mut chosen = None;
        for (i, m) in measured.rows.iter().enumerate() {
            let top = view.top() + measured.tops[i] - self.scroll;
            let rect = Rect::from_min_size(pos2(view.left(), top), vec2(view.width(), m.height));
            if !rect.intersects(view) {
                continue;
            }
            let resp = ui.interact(rect.intersect(view), id.with(("row", i)), Sense::click());
            let on = measured.current == Some(i);
            let hover = ui.ctx().animate_bool_with_time(
                id.with(("row", i)),
                resp.hovered(),
                crate::style::chrome::HOVER_ANIM_SECS,
            );
            if on || hover > 0.0 {
                let amount = if on { FG_HOVER } else { FG_HOVER * hover };
                let wash = t.wash_toward_neutral_fg(ground, amount);
                painter.rect_filled(rect, Radius::Sm.corner(), wash);
            }
            if resp.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }
            let text_rect = Rect::from_min_max(
                pos2(rect.left() + Space::Sm.pts() + m.indent, rect.top()),
                pos2(rect.right() - Space::Sm.pts(), rect.bottom()),
            );
            let at = Align2::LEFT_CENTER
                .align_size_within_rect(m.galley.size(), text_rect)
                .min;
            painter.galley(at, m.galley.clone(), Color32::PLACEHOLDER);
            if m.galley.elided {
                tip_text(ui.ctx(), &resp, m.row.text.clone());
            }
            if resp.clicked() {
                chosen = Some(m.row.slug.clone());
            }
        }

        let clear = Color32::TRANSPARENT;
        if self.scroll > 0.5 {
            paint_fade(&painter, view.with_max_y(view.top() + fade), ground, clear, true);
        }
        if self.scroll < max_scroll - 0.5 {
            paint_fade(&painter, view.with_min_y(view.bottom() - fade), clear, ground, true);
        }
        let before = self.scroll;
        // the bar shows for the user's scrolling, not for a reveal's
        let quiet = !user_scrolled;
        overlay_scroll::scroller_bar(ui, id.with("scroll"), view, content, &mut self.scroll, quiet);
        if self.scroll != before {
            self.scroll_to = None;
        }
        chosen
    }
}

/// A row laid out: its text, its height, and how far its level indents it.
struct MeasuredRow<'a> {
    row: &'a OutlineRow,
    galley: Arc<Galley>,
    height: f32,
    indent: f32,
}

/// The rows laid out at a width, each row's top, and which is current.
struct Measured<'a> {
    rows: Vec<MeasuredRow<'a>>,
    tops: Vec<f32>,
    current: Option<usize>,
}

impl Measured<'_> {
    fn height(&self) -> f32 {
        self.rows
            .last()
            .map_or(0.0, |r| self.tops[self.rows.len() - 1] + r.height)
    }
}

/// Lays out `rows` at `width`. Touch rows are a thumb tall and wrap a long
/// title to two lines; mouse rows are a control tall and truncate.
fn measure<'a>(
    ctx: &egui::Context, rows: &'a [OutlineRow], current: Option<usize>, width: f32, touch: bool,
) -> Measured<'a> {
    let t = ctx.get_lb_theme();
    let mut top = 0.0;
    let mut tops = Vec::with_capacity(rows.len());
    let rows = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let indent = row.level.saturating_sub(1).min(5) as f32 * Space::Md.pts();
            let text_w = (width - indent - 2.0 * Space::Sm.pts()).max(1.0);
            let color = if current == Some(i) { t.accent() } else { t.neutral_fg() };
            let mut job =
                LayoutJob::simple(row.text.clone(), TypeRole::Body.font_id(), color, text_w);
            job.wrap = TextWrapping {
                max_width: text_w,
                max_rows: if touch { 2 } else { 1 },
                break_anywhere: false,
                overflow_character: Some('…'),
            };
            let galley = ctx.fonts(|f| f.layout_job(job));
            let min_h = if touch { TOUCH_ROW_H } else { control_height() };
            let height = min_h.max(galley.size().y + 2.0 * Space::Xs.pts());
            tops.push(top);
            top += height;
            MeasuredRow { row, galley, height, indent }
        })
        .collect();
    Measured { rows, tops, current }
}

/// What the outline says of a note with no headings, laid out at `width`.
fn empty_state(ctx: &egui::Context, width: f32) -> Arc<Galley> {
    let t = ctx.get_lb_theme();
    let text_w = (width - 2.0 * Space::Sm.pts()).max(1.0);
    ctx.fonts(|f| {
        f.layout(
            "No headings yet. Start a line with # to add one.".into(),
            TypeRole::Body.font_id(),
            t.neutral_fg_secondary(),
            text_w,
        )
    })
}

fn empty_height(empty: &Galley) -> f32 {
    control_height().max(empty.size().y + 2.0 * Space::Xs.pts())
}

fn paint_empty_state(ui: &Ui, view: Rect, empty: Arc<Galley>) {
    let at = pos2(view.left() + Space::Sm.pts(), view.center().y - empty.size().y / 2.0);
    ui.painter().galley(at, empty, Color32::PLACEHOLDER);
}

/// The outline's surface, card or sheet: the sidebar's color with no border,
/// and its title in the header row.
fn paint_frame(ui: &Ui, rect: Rect, shadow: Option<egui::Shadow>) {
    let t = ui.ctx().get_lb_theme();
    let (pad, header) = (Space::Sm.pts(), control_height());
    let radius = Radius::Surface.corner();
    if let Some(shadow) = shadow {
        ui.painter().add(shadow.as_shape(rect, radius));
    }
    ui.painter()
        .rect_filled(rect, radius, t.neutral_bg_secondary());
    let font = egui::FontId::new(TypeRole::Body.size(), egui::FontFamily::Name(Arc::from("Bold")));
    let title = ui
        .painter()
        .layout_no_wrap("Outline".into(), font, t.neutral_fg_secondary());
    let at = pos2(
        rect.left() + pad + Space::Xs.pts(),
        rect.top() + pad + (header - title.size().y) / 2.0,
    );
    ui.painter().galley(at, title, t.neutral_fg_secondary());
}
