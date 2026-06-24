use egui::{Color32, Context, Id, Key, Modifiers, Pos2, Rect, Sense, Ui, Vec2};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::completion_popup_rect;
use crate::tab::markdown_editor::widget::inline::link::{
    LinkMetaLookup, LinkPreviewSize, LinkUnderCursor,
};

const ROW_PADDING_X: f32 = 12.0;
const NUM_OPTIONS: usize = 3;
const OPTION_LABELS: [&str; NUM_OPTIONS] = ["Title", "Small", "Large"];

/// Three-option popover summoned when the cursor sits inside a link node.
/// Steps the link through its display modes:
///
/// | idx | label | bracketed link               | autolink                |
/// |-----|-------|------------------------------|-------------------------|
/// | 0   | Title | strip suffix → `[t](url)`    | wrap → `[title](url)`   |
/// | 1   | Small | splice `|small` into bracket | wrap → `[title|small](url)` |
/// | 2   | Large | splice `|large` into bracket | wrap → `[title|large](url)` |
///
/// There's no "Keep" option — the user can just ignore the popover or
/// hit Escape to dismiss it. Mirrors `EmojiCompletions`: per-frame
/// `update_active_state` recompute, `handle_input` claims nav keys via
/// `consume_key`, and `show_link_preview_completions` paints. Up/Down
/// step selection, Enter commits, Escape suppresses for the current
/// link. Left/Right are left alone so the caret stays free to move
/// inside the URL while the popover is open.
#[derive(Default)]
pub struct LinkPreviewCompletions {
    pub active: bool,
    pub selected: usize,
    /// Latest snapshot from [`crate::tab::markdown_editor::MdRender::link_under_cursor`].
    pub(crate) snapshot: Option<LinkUnderCursor>,
    /// `(url, infix_range)` of the link the user last Escaped on. Stays
    /// suppressed until the cursor leaves that link.
    suppressed_for: Option<(String, (Grapheme, Grapheme))>,
    /// `text_seq` observed on the previous `update_active_state` call.
    /// Used to gate fresh activations: the popover only summons when
    /// the buffer *just* changed (typing or pasting). Cursor moves
    /// alone don't re-summon.
    prev_text_seq: u64,
}

impl LinkPreviewCompletions {
    /// Refresh state from the AST-resolved snapshot. `snapshot` is `None`
    /// when the cursor isn't inside a link — that drops suppression so a
    /// later visit starts fresh. `text_seq` is the buffer's
    /// monotonic-increment seq used to distinguish text mutations from
    /// pure cursor moves: the popover only summons on the frame the
    /// buffer changed, so cursoring back into an existing link doesn't
    /// re-summon, but typing or pasting a fresh link does.
    pub fn update_active_state(&mut self, snapshot: Option<LinkUnderCursor>, text_seq: u64) {
        let text_changed = text_seq != self.prev_text_seq;
        self.prev_text_seq = text_seq;

        let Some(snap) = snapshot else {
            self.active = false;
            self.snapshot = None;
            self.suppressed_for = None;
            return;
        };
        let key = (snap.url.clone(), snap.infix_range);
        if let Some(suppressed) = &self.suppressed_for {
            if suppressed == &key {
                self.active = false;
                self.snapshot = Some(snap);
                return;
            }
            self.suppressed_for = None;
        }
        // Only re-seed `selected` when the cursor moves to a *different*
        // link. Otherwise an arrow-key pick on a sized link would snap
        // straight back to the current-size match on the next frame.
        let prev_key = self
            .snapshot
            .as_ref()
            .map(|s| (s.url.clone(), s.infix_range));
        let fresh_visit = prev_key.as_ref() != Some(&key);

        // Activation rule: text just changed and cursor is in a link →
        // summon (typing the URL, pasting). Already active on this same
        // link → stay active across navigation/typing inside it. Cursor
        // just moved to a different link without a text change → leave
        // inactive (the user wants ignorable behavior on revisit).
        let should_activate = if fresh_visit { text_changed } else { self.active };

        if fresh_visit && should_activate {
            self.selected = match snap.current_size {
                LinkPreviewSize::None => 0,
                LinkPreviewSize::Small => 1,
                LinkPreviewSize::Large => 2,
            };
        }
        self.active = should_activate;
        self.snapshot = Some(snap);
    }

    /// Consume nav keys for the popover. Up/Down step selection; Enter
    /// commits the selected option; Escape suppresses until the cursor
    /// leaves the link. Arrow keys at a boundary (Up at top, Down at
    /// bottom) dismiss the menu *and* pass the keystroke through so the
    /// caret can keep moving in that direction. Left/Right are never
    /// consumed so the caret can still move inside the URL itself. No
    /// Cmd+digit jump-picks — those collide with the workspace's
    /// tab-switching shortcuts.
    pub fn handle_input(
        &mut self, ctx: &Context, focused: bool, events: &mut Vec<Event>,
        title_for: impl Fn(&str) -> Option<String>,
    ) {
        if !self.active {
            return;
        }
        let Some(snap) = self.snapshot.as_ref() else { return };

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.suppressed_for = Some((snap.url.clone(), snap.infix_range));
            self.active = false;
            return;
        }
        if !focused {
            return;
        }
        // Peek before consuming — at a boundary we want the arrow key to
        // fall through to the editor's own caret handling instead of
        // being silently eaten.
        let pressed_up = ctx.input(|i| i.key_pressed(Key::ArrowUp) && i.modifiers.is_none());
        let pressed_down = ctx.input(|i| i.key_pressed(Key::ArrowDown) && i.modifiers.is_none());
        if pressed_up {
            if self.selected > 0 {
                ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
                self.selected -= 1;
            } else {
                self.suppressed_for = Some((snap.url.clone(), snap.infix_range));
                self.active = false;
                return;
            }
        }
        if pressed_down {
            if self.selected + 1 < NUM_OPTIONS {
                ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
                self.selected += 1;
            } else {
                self.suppressed_for = Some((snap.url.clone(), snap.infix_range));
                self.active = false;
                return;
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            let idx = self.selected;
            self.commit(idx, events, &title_for);
        }
    }

    /// Push the `Event::Replace` for option `idx`. Both bracketed and
    /// autolinks rebuild the full link source so `advance_cursor: true`
    /// lands the caret right after `](url)` — picking a size and
    /// pressing Enter feels like "commit and keep typing", not "commit
    /// and stay parked inside the link".
    pub fn commit(
        &mut self, idx: usize, events: &mut Vec<Event>, title_for: &impl Fn(&str) -> Option<String>,
    ) {
        let Some(snap) = self.snapshot.as_ref() else { return };
        let target_size = match idx {
            0 => LinkPreviewSize::None,
            1 => LinkPreviewSize::Small,
            2 => LinkPreviewSize::Large,
            _ => return,
        };
        let suffix = match target_size {
            LinkPreviewSize::None => "",
            LinkPreviewSize::Small => "|small",
            LinkPreviewSize::Large => "|large",
        };

        let title = if snap.is_autolink {
            title_for(&snap.url).unwrap_or_else(|| {
                url::Url::parse(&snap.url)
                    .ok()
                    .and_then(|u| u.host_str().map(|h| h.to_string()))
                    .unwrap_or_else(|| snap.url.clone())
            })
        } else {
            snap.stripped_title.clone()
        };
        let new_text = format!("[{title}{suffix}]({url})", url = snap.url);
        events.push(Event::Replace {
            region: Region::BetweenLocations {
                start: Location::Grapheme(snap.node_range.start()),
                end: Location::Grapheme(snap.node_range.end()),
            },
            text: new_text,
            advance_cursor: true,
        });
        self.suppressed_for = Some((snap.url.clone(), snap.infix_range));
        self.active = false;
    }
}

impl MdEdit {
    /// Paint the link-preview popover near the link's start. Skipped when
    /// inactive (cursor not in a link), readonly, or when the renderer
    /// can't resolve the anchor's screen position.
    pub fn show_link_preview_completions(&mut self, ui: &mut Ui) {
        if self.renderer.readonly || !self.link_preview_completions.active {
            return;
        }
        let Some(snap) = self.link_preview_completions.snapshot.as_ref() else {
            return;
        };
        let Some([cursor_top, cursor_bot]) = self.cursor_line(snap.node_range.start()) else {
            return;
        };

        let (popup_rect, row_rects) =
            popover_layout(ui, cursor_top, cursor_bot, &self.renderer.layout);
        self.renderer.touch_consuming_rects.push(popup_rect);

        let mut clicked: Option<usize> = None;
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        for (idx, rect) in row_rects.iter().enumerate() {
            let resp = ui.interact(*rect, Id::new("md_link_preview_row").with(idx), Sense::click());
            if resp.clicked() {
                clicked = Some(idx);
            }
        }
        self.renderer.draw_completion_popup(
            ui,
            popup_rect,
            &row_rects,
            self.link_preview_completions.selected,
            hover_pos,
        );
        paint_rows(ui, &row_rects, &self.renderer.layout);

        if let Some(idx) = clicked {
            let renderer = &self.renderer;
            let title_for = |u: &str| match renderer.get_link_meta(u) {
                LinkMetaLookup::External(Some(m)) => Some(m.title),
                LinkMetaLookup::Internal(t) => Some(t),
                _ => None,
            };
            self.link_preview_completions
                .commit(idx, &mut self.event.internal_events, &title_for);
        }
    }
}

/// Measure the widest pill label, pick a popup rect, and slice it into
/// per-row rects.
fn popover_layout(
    ui: &Ui, cursor_top: Pos2, cursor_bot: Pos2, layout: &crate::tab::markdown_editor::MdLayout,
) -> (Rect, Vec<Rect>) {
    let font_id = egui::FontId::proportional(layout.completion_font_size);
    let label_w = OPTION_LABELS
        .iter()
        .map(|l| measure_label(ui, l, &font_id))
        .fold(0.0_f32, f32::max);
    let popup_width = label_w + ROW_PADDING_X * 2.0;
    let popup_height = NUM_OPTIONS as f32 * layout.completion_row_height;

    let popup_rect = completion_popup_rect(
        cursor_top,
        cursor_bot,
        Vec2::new(popup_width, popup_height),
        ui.ctx().screen_rect(),
    );
    let row_rects: Vec<Rect> = (0..NUM_OPTIONS)
        .map(|i| {
            Rect::from_min_size(
                Pos2::new(
                    popup_rect.min.x,
                    popup_rect.min.y + i as f32 * layout.completion_row_height,
                ),
                Vec2::new(popup_rect.width(), layout.completion_row_height),
            )
        })
        .collect();
    (popup_rect, row_rects)
}

/// Per-row label paint, left-aligned. Backgrounds and the popup frame
/// are drawn separately by `draw_completion_popup`.
fn paint_rows(ui: &Ui, row_rects: &[Rect], layout: &crate::tab::markdown_editor::MdLayout) {
    let vis = ui.visuals();
    let painter = ui.painter();
    let font_id = egui::FontId::proportional(layout.completion_font_size);
    for (idx, rect) in row_rects.iter().enumerate() {
        painter.text(
            Pos2::new(rect.min.x + ROW_PADDING_X, rect.center().y),
            egui::Align2::LEFT_CENTER,
            OPTION_LABELS[idx],
            font_id.clone(),
            vis.text_color(),
        );
    }
}

fn measure_label(ui: &Ui, label: &str, font_id: &egui::FontId) -> f32 {
    ui.fonts(|f| f.layout_no_wrap(label.to_string(), font_id.clone(), Color32::WHITE))
        .size()
        .x
}
