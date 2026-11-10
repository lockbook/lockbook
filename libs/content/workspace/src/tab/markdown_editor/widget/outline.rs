//! The outline: the note's headings, indented by level. With a mouse it is
//! a card docked beside the note's text, floating over the note's right edge
//! where the window is too narrow; on touch, a sheet from the bottom with
//! rows sized for a thumb. A row jumps to its heading, closing a floating
//! card or sheet. The current section is marked, and brought into view only
//! when out of it. A caret folds a heading's section in the note, and a
//! folded section's headings hide here too. Under the outline, the card
//! lists the notes that link to this one, each link with the words around it.

use std::ops::Range;
use std::sync::Arc;

use egui::{Align2, Area, Color32, Galley, Id, Order, Rect, Sense, Ui, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::model::text::offset_types::Grapheme;

use crate::GlyphonRendererCallback;
use crate::style::sheet::sheet_dim;
use crate::style::{
    CHROME_BAND_GLYPH, EDGE_FADE, FG_HOVER, Radius, Space, ThemeExt as _, TypeRole, context_menu,
    control_height, icon_button_glyph, overlay_scroll, paint_fade, phosphor, pin_shadow, tip_text,
};
use crate::theme::palette_v2::Theme;
use crate::widgets::{GlyphonLabel, ShapedLabel, TextOverflow};

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
    /// Its section's fold in the note, for a heading that folds.
    pub fold: Option<HeadingFold>,
    /// Whether a folded section in the note hides it.
    pub hidden: bool,
}

/// A heading's fold in the note.
#[derive(Clone, Copy)]
pub struct HeadingFold {
    pub folded: bool,
    /// Folded, but shown while the selection is in it.
    pub revealed: bool,
}

/// A fold toggle's glyph, a step under the row's text like a caption's.
const CARET_GLYPH: f32 = TypeRole::Caption.size();

/// The outline's rows, and what it says when there are none.
#[derive(Clone, Copy)]
pub struct Headings<'a> {
    /// The note the headings are in.
    pub note: Uuid,
    pub rows: &'a [OutlineRow],
    pub empty: &'a str,
    /// Whether a row's slug lands a link on it, as a markdown heading's does.
    pub linkable: bool,
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
    /// A linking note, at the link when there's a span, in a new tab when
    /// asked.
    Note { id: Uuid, span: Option<Range<usize>>, new_tab: bool },
    /// Copy the link to a row of the outline.
    CopyHeadingLink(usize),
    /// Fold or unfold a row's section in the note.
    ToggleFold(usize),
    /// Copy the link to a note.
    CopyLink(Uuid),
}

impl Choice {
    /// Whether the choice goes somewhere, putting the sheet away.
    fn goes(&self) -> bool {
        matches!(self, Choice::Heading(_) | Choice::Note { .. })
    }
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
        if chosen.as_ref().is_some_and(Choice::goes) || dismissed || escape {
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
        // a tap outside an open row menu puts the menu away, not the sheet
        let menu_open = context_menu::is_any_open(ctx);
        let mut dismissed =
            sheet_dim(ctx, id.with("dim"), layer) && !self.sheet_opening && !menu_open;
        let (margin, pad, header) = (Space::Md.pts(), Space::Sm.pts(), control_height());
        let width = (screen.width() - 2.0 * margin).min(SHEET_MAX_W);
        let inner_w = width - 2.0 * pad;
        // clear of the home indicator or gesture bar the page runs under
        let bottom = Space::Xl.pts();
        let max_list = screen.height() * 0.6;

        let gap = Space::Md.pts();
        let measured = measure_headings(ctx, rows, current, inner_w, true, 1.0);
        let mentions = measure_linked(ctx, linked, inner_w, true, 1.0);
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
                let title =
                    Rect::from_min_size(sheet.min + vec2(pad, pad), vec2(inner_w - header, header));
                chosen = title_menu(ui, id.with("title"), title, rows.note);
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
                if let Some(c) = self
                    .headings
                    .list(ui, id, view, &measured, ground, false, true)
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
                let headings = measure_headings(ctx, rows, current, inner_w, false, shown);
                let mentions = measure_linked(ctx, linked, inner_w, false, shown);
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
                let title = Rect::from_min_size(pos2(left, top), vec2(inner_w, header));
                chosen = title_menu(ui, id.with("title"), title, rows.note);
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
            let resp = ui.interact(view, id.with("empty"), Sense::click());
            return row_menu(&resp, &t, &measured.empty_menu);
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
            // a row's menu doesn't follow its row out of view
            context_menu::close(ui.ctx());
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
        let mut texts = vec![];
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
                resp.contains_pointer(),
                crate::style::chrome::HOVER_ANIM_SECS,
            );
            if on || hover > 0.0 {
                let amount = if on { FG_HOVER } else { FG_HOVER * hover };
                let wash = t.wash_toward_neutral_fg(ground, amount);
                painter.rect_filled(rect, Radius::Sm.corner(), wash);
            }
            let text_rect = Rect::from_min_max(
                pos2(rect.left() + Space::Sm.pts() + m.indent + measured.gutter, rect.top()),
                pos2(rect.right() - Space::Sm.pts(), rect.bottom()),
            );
            let at = Align2::LEFT_CENTER.align_size_within_rect(m.label.size, text_rect);
            let clip = rect.intersect(view);
            texts.push(m.label.clone().text_area(at, ui.ctx(), clip));
            if m.label.elided {
                tip_text(ui.ctx(), &resp, m.tip.clone());
            }
            // as in the note: shown on hover, on touch, and while folded
            let toggled = m.fold.and_then(|(at, fold)| {
                let shown = fold.folded || measured.touch || ui.rect_contains_pointer(clip);
                let caret_left = text_rect.left() - measured.gutter - Space::Xxs.pts();
                // a finger's target reaches back over the indent
                let left = if measured.touch { rect.left() } else { caret_left };
                let slot = Rect::from_min_max(
                    pos2(left, rect.top()),
                    pos2(text_rect.left(), rect.bottom()),
                );
                let caret = slot.with_min_x(caret_left);
                fold_toggle(ui, id.with(("fold", at)), slot, caret, clip, fold, shown)
                    .then_some(Choice::ToggleFold(at))
            });
            if let Some(toggle) = toggled {
                chosen = Some(toggle);
            } else if resp.clicked() {
                chosen = Some(m.choice.clone());
            }
            if let Some(c) = row_menu(&resp, &t, &m.menu) {
                chosen = Some(c);
            }
        }

        // glyphon shapes emoji, which egui's fonts leave out
        let visible = view.intersect(ui.clip_rect());
        if !texts.is_empty() && visible.is_positive() {
            painter.add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                visible,
                GlyphonRendererCallback::new(texts),
            ));
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

/// A row's fold toggle, taking clicks in `slot` and drawn centered in
/// `caret`, as the note draws its own: a caret down while its section shows,
/// a caret right while folded, in the accent unless revealed. Returns
/// whether it was clicked.
fn fold_toggle(
    ui: &mut Ui, id: Id, slot: Rect, caret: Rect, clip: Rect, fold: HeadingFold, shown: bool,
) -> bool {
    let HeadingFold { folded, revealed } = fold;
    let t = ui.ctx().get_lb_theme();
    let resp = ui.interact(slot.intersect(clip), id, Sense::click());
    if !shown && !resp.hovered() {
        return false;
    }
    let glyph = if folded { phosphor::CARET_RIGHT } else { phosphor::CARET_DOWN };
    let color = if folded && !revealed {
        t.accent()
    } else if resp.hovered() {
        t.neutral_fg()
    } else {
        t.neutral_fg_secondary()
    };
    let painter = ui.painter_at(clip);
    let g = painter.layout_no_wrap(
        glyph.into(),
        crate::style::phosphor_font_id(CARET_GLYPH),
        Color32::PLACEHOLDER,
    );
    let at = caret.center() - g.size() / 2.0;
    if resp.hovered() {
        let pad = vec2(Space::Xxs.pts(), Space::Xxs.pts());
        let well = Rect::from_center_size(caret.center(), g.size() + 2.0 * pad);
        let wash = t.wash_toward_neutral_fg(t.neutral_bg(), FG_HOVER);
        painter.rect_filled(well, Radius::Sm.corner(), wash);
    }
    painter.galley(at, g, color);
    tip_text(ui.ctx(), &resp, if folded { "Show Contents" } else { "Hide Contents" });
    resp.clicked()
}

/// A row laid out: its text, its height, how far it indents, what it says
/// when cut short, and what a click on it asks for.
struct MeasuredRow {
    label: ShapedLabel,
    height: f32,
    indent: f32,
    tip: String,
    choice: Choice,
    /// What a right click or a long press on it offers.
    menu: Vec<MenuItem>,
    /// The heading's place and whether it's folded, for a row that folds.
    fold: Option<(usize, HeadingFold)>,
}

/// A context menu row: its icon, its label, and what it asks for; `None`
/// separates.
type MenuItem = Option<(&'static str, &'static str, Choice)>;

/// Copying the link to note `note`: the outline's header, its empty list, a
/// linking note, or with a heading's own link.
fn note_menu(note: Uuid) -> Vec<MenuItem> {
    vec![Some((phosphor::LINK, "Copy Note Link", Choice::CopyLink(note)))]
}

/// The menu of heading `i` of `headings`: its own link, then the note's. A
/// heading no link lands on has none.
fn heading_menu(headings: Headings, i: usize) -> Vec<MenuItem> {
    let row = &headings.rows[i];
    if !headings.linkable || row.slug.is_empty() {
        return vec![];
    }
    let mut menu = vec![Some((phosphor::HASH, "Copy Heading Link", Choice::CopyHeadingLink(i)))];
    menu.extend(note_menu(headings.note));
    menu
}

/// The menu of a linking note's row, or with `span`, of one of its links.
fn linked_menu(id: Uuid, span: Option<Range<usize>>) -> Vec<MenuItem> {
    let is_note = span.is_none();
    let open = |new_tab| Choice::Note { id, span: span.clone(), new_tab };
    let mut menu = vec![
        Some((phosphor::ARROW_SQUARE_OUT, "Open", open(false))),
        Some((phosphor::APP_WINDOW, "Open in New Tab", open(true))),
    ];
    if is_note {
        menu.push(None);
        menu.extend(note_menu(id));
    }
    menu
}

/// A section header at `rect` that stands for the note `note`, and what was
/// chosen in its menu.
fn title_menu(ui: &mut Ui, id: Id, rect: Rect, note: Uuid) -> Option<Choice> {
    let resp = ui.interact(rect, id, Sense::click());
    row_menu(&resp, &ui.ctx().get_lb_theme(), &note_menu(note))
}

/// The context menu of a row, or of an empty list, and what was chosen in it.
fn row_menu(resp: &egui::Response, t: &Theme, menu: &[MenuItem]) -> Option<Choice> {
    if menu.is_empty() {
        return None;
    }
    context_menu::show(resp, t, |e| {
        for item in menu {
            match item {
                Some((icon, label, choice)) => e.item(icon, *label, choice.clone()),
                None => e.separator(),
            }
        }
    })
}

/// A list laid out at a width: its rows and each one's top, which is
/// current, or what it says when it has no rows.
struct Measured {
    rows: Vec<MeasuredRow>,
    tops: Vec<f32>,
    current: Option<usize>,
    empty: Option<Arc<Galley>>,
    empty_menu: Vec<MenuItem>,
    /// Room each row keeps for its fold toggle.
    gutter: f32,
    /// Rows sized for a thumb, whose fold toggles always show.
    touch: bool,
}

/// The room a row keeps for a fold toggle: none when no heading folds.
fn fold_gutter(headings: Headings) -> f32 {
    let folds = headings.rows.iter().any(|r| r.fold.is_some());
    if folds { Space::Md.pts() + Space::Xxs.pts() } else { 0.0 }
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
        Self { rows, tops, current, empty, empty_menu: vec![], gutter: 0.0, touch: false }
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

/// Lays out heading `rows` at `width`, `shown` faded. Touch rows are a thumb
/// tall and wrap a long title to two lines; mouse rows are a control tall and
/// truncate.
fn measure_headings(
    ctx: &egui::Context, headings: Headings, current: Option<usize>, width: f32, touch: bool,
    shown: f32,
) -> Measured {
    let rows = headings.rows;
    let t = ctx.get_lb_theme();
    let gutter = fold_gutter(headings);
    let visible: Vec<usize> = (0..rows.len()).filter(|&i| !rows[i].hidden).collect();
    let current_shown = current.and_then(|c| visible.iter().rposition(|&i| i <= c));
    let current = current_shown.map(|p| visible[p]);
    let measured = visible
        .iter()
        .map(|&i| (i, &rows[i]))
        .map(|(i, row)| {
            let indent = row.level.saturating_sub(1).min(5) as f32 * Space::Md.pts();
            let text_w = (width - indent - gutter - 2.0 * Space::Sm.pts()).max(1.0);
            let color = if current == Some(i) { t.accent() } else { t.neutral_fg() };
            let label = GlyphonLabel::new(&row.text, color.gamma_multiply(shown));
            let label = shape(ctx, label, text_w, if touch { 2 } else { 1 });
            let min_h = if touch { TOUCH_ROW_H } else { control_height() };
            let height = min_h.max(label.size.y + 2.0 * Space::Xs.pts());
            let tip = row.text.clone();
            let menu = heading_menu(headings, i);
            let fold = row.fold.map(|fold| (i, fold));
            MeasuredRow { label, height, indent, tip, choice: Choice::Heading(i), menu, fold }
        })
        .collect();
    let empty = rows
        .is_empty()
        .then(|| quiet_text(ctx, headings.empty, width));
    let mut measured = Measured::new(measured, current_shown, empty);
    measured.gutter = gutter;
    measured.touch = touch;
    measured.empty_menu = note_menu(headings.note);
    measured
}

/// Lines of words around a link shown.
const MENTION_LINES: usize = 3;
/// Characters kept before a link's own words, so they show in the lines.
const MENTION_LEAD: usize = 48;

/// Lays out the notes `linked` here at `width`, `shown` faded: each note's
/// title, then its links with the words around them, the link's own in the
/// text color. Touch rows are at least a thumb tall, and a long title wraps to
/// two lines.
fn measure_linked(
    ctx: &egui::Context, linked: &[LinkedNote], width: f32, touch: bool, shown: f32,
) -> Measured {
    let min_h = if touch { TOUCH_ROW_H } else { control_height() };
    let t = ctx.get_lb_theme();
    let text_w = |indent: f32| (width - indent - 2.0 * Space::Sm.pts()).max(1.0);
    let mut rows = vec![];
    for note in linked {
        let label = GlyphonLabel::new(&note.title, t.neutral_fg().gamma_multiply(shown));
        let label = shape(ctx, label, text_w(0.0), if touch { 2 } else { 1 });
        rows.push(MeasuredRow {
            height: min_h.max(label.size.y + 2.0 * Space::Xs.pts()),
            label,
            indent: 0.0,
            tip: note.title.clone(),
            choice: Choice::Note { id: note.id, span: None, new_tab: false },
            menu: linked_menu(note.id, None),
            fold: None,
        });
        let indent = Space::Md.pts();
        for m in &note.links {
            let (text, own) = lead_trimmed(&m.context, m.label.clone());
            let quiet = Some(t.neutral_fg_secondary().gamma_multiply(shown));
            let spans = vec![
                (&text[..own.start], quiet),
                (&text[own.clone()], None),
                (&text[own.end..], quiet),
            ];
            let label = GlyphonLabel::new_colored(spans, t.neutral_fg().gamma_multiply(shown));
            let label = shape(ctx, label, text_w(indent), MENTION_LINES);
            let least = if touch { TOUCH_ROW_H } else { 0.0 };
            rows.push(MeasuredRow {
                height: least.max(label.size.y + 2.0 * Space::Xs.pts()),
                label,
                indent,
                tip: m.context.clone(),
                choice: Choice::Note { id: note.id, span: Some(m.span.clone()), new_tab: false },
                menu: linked_menu(note.id, Some(m.span.clone())),
                fold: None,
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

/// `label` in body text at `max_width`, cut short after `max_lines`.
fn shape(
    ctx: &egui::Context, label: GlyphonLabel, max_width: f32, max_lines: usize,
) -> ShapedLabel {
    label
        .role(TypeRole::Body)
        .max_width(max_width)
        .text_overflow(TextOverflow::EndEllipsis)
        .max_lines(max_lines)
        .build(ctx)
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
