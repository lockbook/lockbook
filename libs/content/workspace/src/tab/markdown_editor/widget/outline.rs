//! The outline: the note's headings, indented by level. With a mouse it is
//! a card docked beside the note's text, floating over the note's right edge
//! where the window is too narrow; on touch, a sheet from the bottom with
//! rows sized for a thumb. A row jumps to its heading, closing a floating
//! card or sheet. The current section is marked, and brought into view only
//! when out of it. Under the outline, the card lists the notes that link to
//! this one, each link with the words around it.

use std::ops::Range;
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Align2, Area, Color32, Galley, Id, Order, Rect, Sense, Ui, pos2, vec2};
use lb_rs::Uuid;
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

/// The outline's rows, and what it says when there are none.
#[derive(Clone, Copy)]
pub struct Headings<'a> {
    pub rows: &'a [OutlineRow],
    pub empty: &'a str,
}

/// A note that links to this one, and its links here.
pub struct LinkedNote {
    pub id: Uuid,
    pub title: String,
    pub links: Vec<Mention>,
}

/// A link to this note: the words around it, its own among them, and
/// where it is in its note.
pub struct Mention {
    pub context: String,
    pub label: Range<usize>,
    pub span: Range<usize>,
}

/// What a click in the outline asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
    /// A row of the outline, by its place.
    Heading(usize),
    /// A linking note, at the link when there's a span.
    Note { id: Uuid, span: Option<Range<usize>> },
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
    headings: Scroll,
    linked: Scroll,
}

/// A list's scroll.
#[derive(Default)]
struct Scroll {
    /// How far the rows are scrolled, in points from the top.
    at: f32,
    /// Where a reveal is easing the scroll to; the user's own scroll ends it.
    to: Option<f32>,
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

    /// The sheet, if open. Returns the chosen row's choice and the rect that
    /// takes touches.
    pub fn show(
        &mut self, ctx: &egui::Context, id: Id, rows: Headings, current: Option<usize>,
        linked: &[LinkedNote],
    ) -> (Option<Choice>, Option<Rect>) {
        if !self.sheet_open {
            return (None, None);
        }
        let (chosen, rect, dismissed) = self.sheet(ctx, id, rows, current, linked);
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if chosen.is_some() || dismissed || escape {
            self.sheet_open = false;
        }
        self.sheet_opening = false;
        (chosen, Some(rect))
    }

    /// A sheet from the bottom of the screen over a dimmed note, drawn as
    /// the desktop card is, with a close button: the outline, then the notes
    /// `linked` here.
    fn sheet(
        &mut self, ctx: &egui::Context, id: Id, rows: Headings, current: Option<usize>,
        linked: &[LinkedNote],
    ) -> (Option<Choice>, Rect, bool) {
        let t = ctx.get_lb_theme();
        let screen = ctx.content_rect();
        let layer = egui::LayerId::new(Order::Foreground, id);
        let mut dismissed = sheet_dim(ctx, id.with("dim"), layer) && !self.sheet_opening;
        let (margin, pad, header) = (Space::Md.pts(), Space::Sm.pts(), control_height());
        let width = (screen.width() - 2.0 * margin).min(SHEET_MAX_W);
        let inner_w = width - 2.0 * pad;
        // clear of the home indicator or gesture bar the page runs under
        let bottom = Space::Xl.pts();
        let max_list = screen.height() * 0.6;

        let gap = Space::Md.pts();
        let measured = measure_headings(ctx, rows, current, inner_w, true);
        let mentions = measure_linked(ctx, linked, inner_w, true);
        let room = (max_list - header - gap).max(0.0);
        let (heads_h, links_h) = split(room, measured.height(), mentions.height());
        let size = vec2(width, 2.0 * header + gap + heads_h + links_h + 2.0 * pad);
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
                paint_surface(ui, sheet, None);
                paint_title(ui, "Outline", sheet.left() + pad, sheet.top() + pad);
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
                let (left, mut top) = (sheet.left() + pad, sheet.top() + pad + header);
                let view = Rect::from_min_size(pos2(left, top), vec2(inner_w, heads_h));
                chosen = self
                    .headings
                    .list(ui, id, view, &measured, ground, false, true);
                top += heads_h + gap;
                paint_title(ui, "Backlinks", left, top);
                top += header;
                let view = Rect::from_min_size(pos2(left, top), vec2(inner_w, links_h));
                let links_id = id.with("linked");
                if let Some(c) = self
                    .linked
                    .list(ui, links_id, view, &mentions, ground, false, true)
                {
                    chosen = Some(c);
                }
            });
        // the dimmed note takes taps too, which dismiss
        (chosen, screen, dismissed)
    }

    /// The card at `place`, fading in or out: the outline, then the notes
    /// `linked` here. It is as tall as its rows, up to `area`, each list
    /// scrolling inside when they don't fit. Floating, it closes on a click
    /// elsewhere in `canvas` or Escape. Returns the choice, the card, and
    /// whether it was dismissed.
    #[allow(clippy::too_many_arguments)]
    pub fn card(
        &mut self, ctx: &egui::Context, id: Id, place: Option<CardPlace>, rows: Headings,
        current: Option<usize>, linked: &[LinkedNote],
    ) -> (Option<Choice>, Option<Rect>, bool) {
        let Some(CardPlace { area, floating, shown, open }) = place else {
            return (None, None, false);
        };
        let t = ctx.get_lb_theme();
        // the page's own color: docked it reads as part of the note
        let ground = t.neutral_bg();
        let pad = Space::Sm.pts();
        let header = control_height();
        let gap = Space::Md.pts();
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
                let headings = measure_headings(ctx, rows, current, inner_w, false);
                let mentions = measure_linked(ctx, linked, inner_w, false);
                let room = (area.height() - 2.0 * header - gap - 2.0 * pad).max(0.0);
                let (heads_h, links_h) = split(room, headings.height(), mentions.height());
                card = Rect::from_min_size(
                    area.min,
                    vec2(area.width(), 2.0 * header + gap + heads_h + links_h + 2.0 * pad),
                );
                ui.allocate_rect(card, Sense::hover());

                // over the text it lifts off it
                let radius = Radius::Surface.corner();
                if floating {
                    ui.painter().add(pin_shadow(&t).as_shape(card, radius));
                }
                ui.painter().rect_filled(card, radius, ground);
                let (left, mut top) = (card.left() + pad, card.top() + pad);
                paint_title(ui, "Outline", left, top);
                top += header;
                let view = Rect::from_min_size(pos2(left, top), vec2(inner_w, heads_h));
                if let Some(c) = self
                    .headings
                    .list(ui, id, view, &headings, ground, reveal, open)
                {
                    chosen = Some(c);
                }
                top += heads_h + gap;
                paint_title(ui, "Backlinks", left, top);
                top += header;
                let view = Rect::from_min_size(pos2(left, top), vec2(inner_w, links_h));
                let links_id = id.with("linked");
                if let Some(c) = self
                    .linked
                    .list(ui, links_id, view, &mentions, ground, false, open)
                {
                    chosen = Some(c);
                }
            });
        if !open {
            return (None, Some(card), false);
        }
        // a floating card stays while the reader works under it; Esc puts it away
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let dismissed = floating && escape;
        (chosen, Some(card), dismissed)
    }
}

/// Splits `room` between two lists `a` and `b` tall: each whole if both
/// fit, else the shorter whole if it fits in half, else half each.
fn split(room: f32, a: f32, b: f32) -> (f32, f32) {
    let half = room / 2.0;
    if a + b <= room {
        (a, b)
    } else if a <= half {
        (a, room - a)
    } else if b <= half {
        (room - b, b)
    } else {
        (half, half)
    }
}

impl Scroll {
    /// Draws `measured` rows in `view` over `ground`, scrolled by the wheel, a
    /// drag, or the bar. Rows fade out at an edge with more beyond it, and
    /// `reveal` eases the current row into view clear of the fades; a list
    /// just opened starts there. A list that isn't `live` (fading out) takes
    /// no scrolling. Returns the chosen row's choice.
    #[allow(clippy::too_many_arguments)]
    fn list(
        &mut self, ui: &mut Ui, id: Id, view: Rect, measured: &Measured, ground: Color32,
        reveal: bool, live: bool,
    ) -> Option<Choice> {
        let t = ui.ctx().get_lb_theme();
        if let Some(empty) = &measured.empty {
            let at = pos2(view.left() + Space::Sm.pts(), view.top() + Space::Xs.pts());
            ui.painter_at(view)
                .galley(at, empty.clone(), Color32::PLACEHOLDER);
            return None;
        }
        let fade = EDGE_FADE.pts();
        let content = measured.height();
        let max_scroll = (content - view.height()).max(0.0);

        let pass = ui.ctx().cumulative_pass_nr();
        let opening = pass > self.drawn_pass + 1;
        self.drawn_pass = pass;

        let drag = ui.interact(view, id.with("drag"), Sense::drag());
        let before = self.at;
        if live && ui.rect_contains_pointer(view) {
            self.at -= ui.input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta.y));
        }
        if live && drag.dragged() {
            self.at -= drag.drag_delta().y;
        }
        let user_scrolled = self.at != before;
        if user_scrolled {
            self.to = None;
        }
        // the current row clear of the fades, from where the scroll is headed
        if let Some(i) = measured.current.filter(|_| reveal || opening) {
            let (top, bottom) = (measured.tops[i], measured.tops[i] + measured.rows[i].height);
            let headed = self.to.unwrap_or(self.at);
            let to = if top - fade < headed {
                top - fade
            } else if bottom + fade > headed + view.height() {
                bottom + fade - view.height()
            } else {
                headed
            };
            self.to = Some(to.clamp(0.0, max_scroll));
        }
        if let Some(to) = self.to {
            let dt = ui.input(|i| i.stable_dt).min(0.1);
            let step = 1.0 - (-dt / SCROLL_TAU).exp();
            self.at += (to - self.at) * if opening { 1.0 } else { step };
            if (to - self.at).abs() < 0.5 {
                self.at = to;
                self.to = None;
            } else {
                ui.ctx().request_repaint();
            }
        }
        self.at = self.at.clamp(0.0, max_scroll);

        let painter = ui.painter_at(view);
        let mut chosen = None;
        for (i, m) in measured.rows.iter().enumerate() {
            let top = view.top() + measured.tops[i] - self.at;
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
            let text_rect = Rect::from_min_max(
                pos2(rect.left() + Space::Sm.pts() + m.indent, rect.top()),
                pos2(rect.right() - Space::Sm.pts(), rect.bottom()),
            );
            let at = Align2::LEFT_CENTER
                .align_size_within_rect(m.galley.size(), text_rect)
                .min;
            painter.galley(at, m.galley.clone(), Color32::PLACEHOLDER);
            if m.galley.elided {
                tip_text(ui.ctx(), &resp, m.tip.clone());
            }
            if resp.clicked() {
                chosen = Some(m.choice.clone());
            }
        }

        let clear = Color32::TRANSPARENT;
        if self.at > 0.5 {
            paint_fade(&painter, view.with_max_y(view.top() + fade), ground, clear, true);
        }
        if self.at < max_scroll - 0.5 {
            paint_fade(&painter, view.with_min_y(view.bottom() - fade), clear, ground, true);
        }
        let before = self.at;
        // the bar shows for the user's scrolling, not for a reveal's
        let quiet = !user_scrolled;
        overlay_scroll::scroller_bar(ui, id.with("scroll"), view, content, &mut self.at, quiet);
        if self.at != before {
            self.to = None;
        }
        chosen
    }
}

/// A row laid out: its text, its height, how far it indents, what it says
/// when cut short, and what a click on it asks for.
struct MeasuredRow {
    galley: Arc<Galley>,
    height: f32,
    indent: f32,
    tip: String,
    choice: Choice,
}

/// A list laid out at a width: its rows and each one's top, which is
/// current, or what it says when it has no rows.
struct Measured {
    rows: Vec<MeasuredRow>,
    tops: Vec<f32>,
    current: Option<usize>,
    empty: Option<Arc<Galley>>,
}

impl Measured {
    fn new(rows: Vec<MeasuredRow>, current: Option<usize>, empty: Option<Arc<Galley>>) -> Self {
        let mut top = 0.0;
        let tops = rows
            .iter()
            .map(|r| {
                let at = top;
                top += r.height;
                at
            })
            .collect();
        Self { rows, tops, current, empty }
    }

    fn height(&self) -> f32 {
        match &self.empty {
            Some(empty) => control_height().max(empty.size().y + 2.0 * Space::Xs.pts()),
            None => self
                .rows
                .last()
                .map_or(0.0, |r| self.tops[self.rows.len() - 1] + r.height),
        }
    }
}

/// Lays out heading `rows` at `width`. Touch rows are a thumb tall and wrap a
/// long title to two lines; mouse rows are a control tall and truncate.
fn measure_headings(
    ctx: &egui::Context, headings: Headings, current: Option<usize>, width: f32, touch: bool,
) -> Measured {
    let rows = headings.rows;
    let t = ctx.get_lb_theme();
    let measured = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let indent = row.level.saturating_sub(1).min(5) as f32 * Space::Md.pts();
            let text_w = (width - indent - 2.0 * Space::Sm.pts()).max(1.0);
            let color = if current == Some(i) { t.accent() } else { t.neutral_fg() };
            let mut job =
                LayoutJob::simple(row.text.clone(), TypeRole::Body.font_id(), color, text_w);
            job.wrap = wrap(text_w, if touch { 2 } else { 1 });
            let galley = ctx.fonts_mut(|f| f.layout_job(job));
            let min_h = if touch { TOUCH_ROW_H } else { control_height() };
            let height = min_h.max(galley.size().y + 2.0 * Space::Xs.pts());
            let tip = row.text.clone();
            MeasuredRow { galley, height, indent, tip, choice: Choice::Heading(i) }
        })
        .collect();
    let empty = rows
        .is_empty()
        .then(|| quiet_text(ctx, headings.empty, width));
    Measured::new(measured, current, empty)
}

/// Lines of words around a link shown.
const MENTION_LINES: usize = 3;
/// Characters kept before a link's own words, so they show in the lines.
const MENTION_LEAD: usize = 48;

/// Lays out the notes `linked` here at `width`: each note's title, then its
/// links with the words around them, the link's own in the text color.
/// Touch rows are at least a thumb tall, and a long title wraps to two lines.
fn measure_linked(ctx: &egui::Context, linked: &[LinkedNote], width: f32, touch: bool) -> Measured {
    let min_h = if touch { TOUCH_ROW_H } else { control_height() };
    let t = ctx.get_lb_theme();
    let text_w = |indent: f32| (width - indent - 2.0 * Space::Sm.pts()).max(1.0);
    let mut rows = vec![];
    for note in linked {
        let mut job = LayoutJob::simple(
            note.title.clone(),
            TypeRole::Body.font_id(),
            t.neutral_fg(),
            text_w(0.0),
        );
        job.wrap = wrap(text_w(0.0), if touch { 2 } else { 1 });
        let galley = ctx.fonts_mut(|f| f.layout_job(job));
        rows.push(MeasuredRow {
            height: min_h.max(galley.size().y + 2.0 * Space::Xs.pts()),
            galley,
            indent: 0.0,
            tip: note.title.clone(),
            choice: Choice::Note { id: note.id, span: None },
        });
        let indent = Space::Md.pts();
        for m in &note.links {
            let (text, label) = lead_trimmed(&m.context, m.label.clone());
            let font = TypeRole::Body.font_id();
            let quiet = TextFormat::simple(font.clone(), t.neutral_fg_secondary());
            let loud = TextFormat::simple(font, t.neutral_fg());
            let mut job = LayoutJob::default();
            job.append(&text[..label.start], 0.0, quiet.clone());
            job.append(&text[label.clone()], 0.0, loud);
            job.append(&text[label.end..], 0.0, quiet);
            job.wrap = wrap(text_w(indent), MENTION_LINES);
            let galley = ctx.fonts_mut(|f| f.layout_job(job));
            let least = if touch { TOUCH_ROW_H } else { 0.0 };
            rows.push(MeasuredRow {
                height: least.max(galley.size().y + 2.0 * Space::Xs.pts()),
                galley,
                indent,
                tip: m.context.clone(),
                choice: Choice::Note { id: note.id, span: Some(m.span.clone()) },
            });
        }
    }
    let empty = linked
        .is_empty()
        .then(|| quiet_text(ctx, "No backlinks yet. Type [[ in another note to link here.", width));
    Measured::new(rows, None, empty)
}

/// `context` cut to [`MENTION_LEAD`] characters before `label`, at a word.
fn lead_trimmed(context: &str, label: Range<usize>) -> (String, Range<usize>) {
    let head = &context[..label.start];
    if head.chars().count() <= MENTION_LEAD {
        return (context.to_string(), label);
    }
    let cut = head
        .char_indices()
        .rev()
        .nth(MENTION_LEAD - 1)
        .map_or(0, |(i, _)| i);
    let cut = head[cut..].find(' ').map_or(cut, |i| cut + i + 1);
    let text = format!("…{}", &context[cut..]);
    let shift = cut as isize - '…'.len_utf8() as isize;
    let moved = |i: usize| (i as isize - shift) as usize;
    (text, moved(label.start)..moved(label.end))
}

fn wrap(max_width: f32, max_rows: usize) -> TextWrapping {
    TextWrapping { max_width, max_rows, break_anywhere: false, overflow_character: Some('…') }
}

/// A list's empty state, laid out at `width`.
fn quiet_text(ctx: &egui::Context, text: &str, width: f32) -> Arc<Galley> {
    let t = ctx.get_lb_theme();
    let text_w = (width - 2.0 * Space::Sm.pts()).max(1.0);
    ctx.fonts_mut(|f| {
        f.layout(text.into(), TypeRole::Body.font_id(), t.neutral_fg_secondary(), text_w)
    })
}

/// The outline's surface, card or sheet: the sidebar's color with no border.
fn paint_surface(ui: &Ui, rect: Rect, shadow: Option<egui::Shadow>) {
    let t = ui.ctx().get_lb_theme();
    let radius = Radius::Surface.corner();
    if let Some(shadow) = shadow {
        ui.painter().add(shadow.as_shape(rect, radius));
    }
    ui.painter()
        .rect_filled(rect, radius, t.neutral_bg_secondary());
}

/// A section's title in a header row whose top left is (`left`, `top`).
/// A section's title, in small caps as list sections are titled.
fn paint_title(ui: &Ui, title: &str, left: f32, top: f32) {
    let t = ui.ctx().get_lb_theme();
    let title = ui.painter().layout_no_wrap(
        TypeRole::caps(title),
        TypeRole::Caption.font_id(),
        t.neutral_fg_secondary(),
    );
    let at = pos2(left + Space::Xs.pts(), top + (control_height() - title.size().y) / 2.0);
    ui.painter().galley(at, title, t.neutral_fg_secondary());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_lead_is_cut_at_a_word() {
        let context = format!("{} the [plan] here", "word ".repeat(30));
        let start = context.find("plan").unwrap();
        let (text, label) = lead_trimmed(&context, start..start + 4);
        assert!(text.starts_with("…word"));
        assert_eq!(&text[label], "plan");
    }
}
