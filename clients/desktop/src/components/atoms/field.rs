//! Editable field: chrome + [`GlyphonTextEdit`].
//!
//! Optional leading icon, clear button, and static trailing text (e.g. `.md`).
//! Needs `register_font_system` and `register_render_callback_resources`.

use egui::{Align, Color32, Id, Layout, Response, Stroke, StrokeKind, Ui, pos2, vec2};
use workspace_rs::widgets::GlyphonTextEdit;

use crate::components::foundation::chrome::{
    Radius, STROKE_HAIRLINE, control_height, control_line_height, phosphor, phosphor_ui_font_id,
};
use crate::components::foundation::color::{
    FG_HOVER, FG_PRESS, QUIET_PLATE_HOVER, QUIET_PLATE_PRESS, Theme,
};
use crate::components::foundation::interact::{ControlFills, interact_fill, sense_click};
use crate::components::foundation::layout::{inset, paint_control_pads, place_at};
use crate::components::foundation::space::control as control_space;
use crate::components::foundation::typography::TypeRole;

/// Single-line field with border region chrome and glyphon text.
pub struct Field<'a> {
    tokens: &'a Theme,
    text: &'a mut String,
    hint: String,
    width: Option<f32>,
    /// Stable id salt (share sheet host, etc.). Auto id if none.
    id_salt: Option<Id>,
    leading: Option<&'static str>,
    /// Leading glyph ink; default [`Theme::neutral_fg_secondary`].
    leading_ink: Option<Color32>,
    clearable: bool,
    /// Non-editable suffix (file extension).
    trailing_static: Option<String>,
    /// Select all text the first frame the edit gains focus.
    select_all_on_focus: bool,
    /// Ghost autocomplete after typed text (not the empty-field hint).
    completion_suffix: Option<String>,
    /// Full string Tab accepts (prefix of which is already typed).
    completion_full: Option<String>,
    /// Always mask as `*` (account key).
    password: bool,
    /// After keyboard events, before paint: rewrite buffer + remap caret.
    /// `(old_text, cursor, anchor) -> (new_text, new_cursor, new_anchor)`.
    rewrite: Option<Box<FieldRewrite<'a>>>,
    /// Re-take focus when a press lands on a non-focusable control (default).
    sticky: bool,
}

/// Buffer rewrite after keyboard events (see [`Field::rewrite`]).
type FieldRewrite<'a> = dyn FnOnce(&str, usize, usize) -> (String, usize, usize) + 'a;

impl<'a> Field<'a> {
    pub fn new(tokens: &'a Theme, text: &'a mut String) -> Self {
        Self {
            tokens,
            text,
            hint: String::new(),
            width: None,
            id_salt: None,
            leading: None,
            leading_ink: None,
            clearable: false,
            trailing_static: None,
            select_all_on_focus: false,
            completion_suffix: None,
            completion_full: None,
            password: false,
            rewrite: None,
            sticky: true,
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// Stable host id; edit id is `host.with("edit")`.
    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id_salt = Some(id.into());
        self
    }

    /// Leading Phosphor glyph (e.g. [`phosphor::SEARCH`], [`phosphor::USER`]).
    pub fn leading(mut self, icon: &'static str) -> Self {
        self.leading = Some(icon);
        self
    }

    /// Ink for the leading glyph (share lookup: accent found / danger missing).
    pub fn leading_ink(mut self, color: Color32) -> Self {
        self.leading_ink = Some(color);
        self
    }

    /// Show a clear control when the buffer is non-empty.
    pub fn clearable(mut self, on: bool) -> Self {
        self.clearable = on;
        self
    }

    /// Static trailing text (e.g. `".md"`) — not editable.
    pub fn trailing_static(mut self, s: impl Into<String>) -> Self {
        self.trailing_static = Some(s.into());
        self
    }

    /// Select the full buffer when the edit first gains focus (create/rename sheets).
    pub fn select_all_on_focus(mut self, on: bool) -> Self {
        self.select_all_on_focus = on;
        self
    }

    /// Muted ghost after the typed buffer (autocomplete). Empty = none.
    pub fn completion_suffix(mut self, suffix: impl Into<String>) -> Self {
        let s = suffix.into();
        self.completion_suffix = if s.is_empty() { None } else { Some(s) };
        self
    }

    /// Full completion Tab accepts (keeps focus; does not tab-stop away).
    pub fn completion_full(mut self, full: impl Into<String>) -> Self {
        let s = full.into();
        self.completion_full = if s.is_empty() { None } else { Some(s) };
        self
    }

    /// Always paint `*` per character (account key).
    pub fn password(mut self, on: bool) -> Self {
        self.password = on;
        self
    }

    /// When false, a press outside surrenders focus (caption → field blur).
    pub fn sticky(mut self, on: bool) -> Self {
        self.sticky = on;
        self
    }

    /// Rewrite the buffer after events and before paint (e.g. card grouping).
    /// Receives the post-event text plus caret/anchor; returns rewritten text
    /// and remapped caret so invalid chars never flash and the cursor stays
    /// on the right digit.
    pub fn rewrite(
        mut self, f: impl FnOnce(&str, usize, usize) -> (String, usize, usize) + 'a,
    ) -> Self {
        self.rewrite = Some(Box::new(f));
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let t = self.tokens;
        let host = self.id_salt.unwrap_or_else(|| {
            let id = ui.next_auto_id();
            ui.skip_ahead_auto_ids(1);
            id
        });
        let edit_id = host.with("edit");

        // Drain before paint. Non-empty claims Tab (complete / stay).
        let _ = GlyphonTextEdit::process_events_ex(
            ui,
            edit_id,
            self.text,
            self.completion_full.as_deref(),
            true,
        );

        // Format / strip before paint so the frame never shows raw input.
        if let Some(rewrite) = self.rewrite {
            GlyphonTextEdit::rewrite_buffer(ui, edit_id, self.text, rewrite);
        }

        // Ghost must match **post-event** buffer. Callers often pass a suffix
        // computed from last frame's text; after a key, that suffix is stale
        // (typed char appears in real text *and* as ghost start → flicker/shift).
        // Prefer deriving from `completion_full` after events.
        let paint_suffix = self
            .completion_full
            .as_ref()
            .and_then(|full| completion_ghost_suffix(self.text, full))
            .or_else(|| {
                self.completion_suffix.as_ref().and_then(|s| {
                    if self.text.is_empty() || s.is_empty() { None } else { Some(s.clone()) }
                })
            });

        let height = control_height();
        let width = self
            .width
            .unwrap_or_else(|| crate::components::ui_width(ui).max(1.0));
        // Host chrome is click-only (no FOCUSABLE) — focus lives on `edit_id`.
        let (rect, mut response) = ui.allocate_exact_size(vec2(width, height), sense_click());

        let focused = ui.memory(|m| m.has_focus(edit_id));
        if response.clicked() {
            ui.memory_mut(|m| m.request_focus(edit_id));
        }

        let (fill, stroke_c) = if focused {
            (t.neutral_bg(), t.neutral_fg())
        } else {
            (t.neutral_bg_secondary(), t.neutral())
        };
        let radius = Radius::Control.corner();
        ui.painter().rect_filled(rect, radius, fill);
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(STROKE_HAIRLINE, stroke_c),
            StrokeKind::Inside,
        );

        let pad_x = control_space::PAD_X;
        let pad_y = control_space::PAD_Y;
        let icon_gap = control_space::ICON_GAP;
        let mid_h = (rect.height() - pad_y.pts() * 2.0).max(control_line_height());

        paint_control_pads(ui, rect, pad_x, pad_y);
        let mid = inset(rect, pad_x.pts(), pad_y.pts());
        let mut x = mid.left();

        if let Some(icon) = self.leading {
            let ig = ui.painter().layout_no_wrap(
                icon.to_owned(),
                phosphor_ui_font_id(),
                Color32::PLACEHOLDER,
            );
            let ir = egui::Rect::from_min_size(pos2(x, mid.top()), vec2(ig.size().x, mid_h));
            let ink = self.leading_ink.unwrap_or_else(|| t.neutral_fg_secondary());
            ui.painter()
                .galley(pos2(ir.left(), ir.center().y - ig.size().y / 2.0), ig, ink);
            x += ir.width() + icon_gap.pts();
        }

        // Layout: [edit] [.ext] [× clear] — extension stays on the name,
        // clear is outermost (not between stem and suffix).
        let show_clear = self.clearable && !self.text.is_empty();
        let clear_w = if show_clear {
            mid_h.min(control_height() - pad_y.pts()) + icon_gap.pts()
        } else {
            0.0
        };
        let trail_g = self.trailing_static.as_ref().map(|s| {
            ui.painter()
                .layout_no_wrap(s.clone(), TypeRole::Body.font_id(), Color32::PLACEHOLDER)
        });
        let trail_w = trail_g
            .as_ref()
            .map(|g| g.size().x + icon_gap.pts())
            .unwrap_or(0.0);

        let edit_w = (mid.right() - clear_w - trail_w - x).max(40.0);
        let edit_rect = egui::Rect::from_min_size(pos2(x, mid.top()), vec2(edit_w, mid_h));
        let (edit_resp, _) = place_at(ui, edit_rect, Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(edit_w);
            ui.set_max_width(edit_w);
            let mask = self.password;
            let mut edit = GlyphonTextEdit::new(self.text)
                .id(edit_id)
                .font_size(TypeRole::Body.size())
                .line_height(control_line_height())
                .password(mask)
                .claim_tab_when_nonempty(true);
            if !self.hint.is_empty() {
                edit = edit.hint_text(&self.hint);
            }
            if !mask {
                if let Some(ref suffix) = paint_suffix {
                    edit = edit.completion_suffix(suffix);
                }
            }
            if self.select_all_on_focus {
                edit = edit.select_all();
            }
            edit.show(ui)
        });
        x += edit_w;

        if let Some(tg) = trail_g {
            x += icon_gap.pts();
            let tr = egui::Rect::from_min_size(pos2(x, mid.top()), vec2(tg.size().x, mid_h));
            ui.painter().galley(
                pos2(tr.left(), tr.center().y - tg.size().y / 2.0),
                tg,
                t.neutral_fg_secondary(),
            );
            x += tr.width();
        }

        if show_clear {
            x += icon_gap.pts();
            let clear_sz = mid_h.min(control_height() - pad_y.pts());
            let cr = egui::Rect::from_min_size(pos2(x, mid.top()), vec2(clear_sz, mid_h));
            let cresp = ui.interact(cr, host.with("clear_hit"), sense_click());
            let over = ui.ctx().rect_contains_pointer(ui.layer_id(), cr);
            let ground = if focused { t.neutral_bg() } else { t.neutral_bg_secondary() };
            let (h_amt, p_amt) =
                if focused { (FG_HOVER, FG_PRESS) } else { (QUIET_PLATE_HOVER, QUIET_PLATE_PRESS) };
            let fills = ControlFills {
                rest: ground,
                hover: t.wash_toward_neutral_fg(ground, h_amt),
                press: t.wash_toward_neutral_fg(ground, p_amt),
            };
            let fill = interact_fill(
                ui.ctx(),
                host.with("clear"),
                over,
                cresp.is_pointer_button_down_on(),
                cresp.clicked(),
                fills,
            );
            ui.painter().rect_filled(cr, Radius::Sm.corner(), fill);
            let hover_t = ui.ctx().animate_bool(host.with("clear_ink"), over);
            let ink = t
                .neutral_fg_secondary()
                .lerp_to_gamma(t.neutral_fg(), hover_t);
            let xg = ui
                .painter()
                .layout_no_wrap("×".into(), TypeRole::Body.font_id(), ink);
            ui.painter().galley(cr.center() - xg.size() / 2.0, xg, ink);
            if cresp.clicked() {
                self.text.clear();
                if self.sticky {
                    ui.memory_mut(|m| m.request_focus(edit_id));
                } else {
                    ui.memory_mut(|m| m.surrender_focus(edit_id));
                }
            }
        }

        response = response.union(edit_resp);

        if response.secondary_clicked() {
            ui.memory_mut(|m| m.request_focus(edit_id));
        }
        if let Some(cmd) = crate::components::context_menu::show(&response, t, |e| {
            e.item(phosphor::SCISSORS, "Cut", FieldEditCmd::Cut);
            e.item(phosphor::COPY, "Copy", FieldEditCmd::Copy);
            e.item(phosphor::CLIPBOARD_TEXT, "Paste", FieldEditCmd::Paste);
            e.separator();
            e.item(phosphor::SELECTION_ALL, "Select all", FieldEditCmd::SelectAll);
        }) {
            ui.memory_mut(|m| m.request_focus(edit_id));
            match cmd {
                FieldEditCmd::Cut => {
                    ui.ctx().input_mut(|i| i.events.push(egui::Event::Cut));
                    let _ = GlyphonTextEdit::process_events_ex(
                        ui,
                        edit_id,
                        self.text,
                        self.completion_full.as_deref(),
                        true,
                    );
                }
                FieldEditCmd::Copy => {
                    ui.ctx().input_mut(|i| i.events.push(egui::Event::Copy));
                    let _ = GlyphonTextEdit::process_events_ex(
                        ui,
                        edit_id,
                        self.text,
                        self.completion_full.as_deref(),
                        true,
                    );
                }
                FieldEditCmd::Paste => {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
                FieldEditCmd::SelectAll => {
                    let len = self.text.len();
                    GlyphonTextEdit::place_cursor(ui, edit_id, len, 0);
                }
            }
        }

        // Sticky text focus: egui surrenders focus on any press outside the
        // focused widget. Non-text controls use `sense_click` (no FOCUSABLE) and
        // do not request focus — if a pointer press cleared us and nothing else
        // claimed focus, re-take it. Keyboard surrender (Esc / Enter submit) has
        // no pointer press, so we leave focus clear.
        let lost = ui.memory(|m| m.had_focus_last_frame(edit_id) && !m.has_focus(edit_id));
        if self.sticky && lost {
            let free = ui.memory(|m| m.focused().is_none());
            let pointer = ui.input(|i| i.pointer.any_pressed() || i.pointer.any_down());
            if free && pointer {
                ui.memory_mut(|m| m.request_focus(edit_id));
            }
        }

        response
    }
}

#[derive(Clone, Copy)]
enum FieldEditCmd {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

/// Ghost after typed buffer: remainder of `full` when `query` is a
/// case-insensitive prefix. Empty / exact / non-prefix → none.
fn completion_ghost_suffix(query: &str, full: &str) -> Option<String> {
    if query.is_empty() {
        return None;
    }
    let q_chars: Vec<char> = query.chars().collect();
    let f_chars: Vec<char> = full.chars().collect();
    if f_chars.len() <= q_chars.len() {
        return None;
    }
    let prefix_ok = q_chars
        .iter()
        .zip(f_chars.iter())
        .all(|(a, b)| a.eq_ignore_ascii_case(b));
    if !prefix_ok {
        return None;
    }
    Some(f_chars[q_chars.len()..].iter().collect())
}
