//! Buttons: primary (ink fill) or quiet (canvas rest). Danger / accent are tones
//! on the solid primary plate (same hover/press wash toward `neutral_bg`; label
//! and kbd use `neutral_bg` ink).
//! Padding uses [`Spacer`]s (`control` space tokens) so F2 paints pad bands.

use egui::{
    Color32, FontFamily, FontId, Galley, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use std::sync::Arc;

use crate::components::foundation::chrome::{
    HOVER_ANIM_SECS, KbdPart, Radius, STROKE_HAIRLINE, Shortcut, control_height, phosphor,
    phosphor_ui_font_id,
};
use crate::components::foundation::color::{Theme, BG_HOVER, BG_PRESS, FG_HOVER, FG_PRESS};
use crate::components::foundation::interact::{ControlFills, sense_click};
use crate::components::foundation::layout::{inset, paint_control_pads};
use crate::components::foundation::space::control as control_space;
use crate::components::foundation::typography::TypeRole;

#[derive(Clone, Copy)]
enum Treatment {
    Primary,
    Quiet,
}

#[derive(Clone, Copy)]
enum Tone {
    /// Solid ink (`neutral_fg`) — default primary.
    Brand,
    /// Destructive solid (`danger`).
    Danger,
    /// Emphasized non-destructive solid (`accent`) — e.g. Share / notify others.
    Accent,
}

/// How long a copy control shows the check instead of its label.
const COPY_FEEDBACK_SECS: f64 = 1.2;

pub struct Button<'a> {
    tokens: &'a Theme,
    label: String,
    treatment: Treatment,
    tone: Tone,
    enabled: bool,
    height: Option<f32>,
    min_width: Option<f32>,
    max_width: Option<f32>,
    shortcut: Option<Shortcut>,
    /// Leading Phosphor glyph (PUA codepoint string).
    icon: Option<&'static str>,
    /// Stable id: after click, replace label with a centered check for 1s.
    copy_feedback: Option<egui::Id>,
}

impl<'a> Button<'a> {
    /// Solid commit — one per decision region.
    pub fn primary(t: &'a Theme, label: impl Into<String>) -> Self {
        Self::new(t, label, Treatment::Primary)
    }

    /// Quiet action — canvas rest.
    pub fn quiet(t: &'a Theme, label: impl Into<String>) -> Self {
        Self::new(t, label, Treatment::Quiet)
    }

    fn new(tokens: &'a Theme, label: impl Into<String>, treatment: Treatment) -> Self {
        Self {
            tokens,
            label: label.into(),
            treatment,
            tone: Tone::Brand,
            enabled: true,
            height: None,
            min_width: None,
            max_width: None,
            shortcut: None,
            icon: None,
            copy_feedback: None,
        }
    }

    pub fn danger(mut self) -> Self {
        self.tone = Tone::Danger;
        self
    }

    /// Solid accent plate — same structure as [`Self::danger`], brand hue.
    pub fn accent(mut self) -> Self {
        self.tone = Tone::Accent;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }

    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }

    pub fn shortcut(mut self, s: Shortcut) -> Self {
        self.shortcut = Some(s);
        self
    }

    /// After a successful click, swap the label for a centered check for 1.2s
    /// (copy actions). Width stays that of the rest-state label.
    pub fn copy_feedback(mut self, id: impl Into<egui::Id>) -> Self {
        self.copy_feedback = Some(id.into());
        self
    }

    pub fn show(mut self, ui: &mut Ui) -> Response {
        let t = self.tokens;
        let font = TypeRole::Body.font_id();
        let pad_x = control_space::PAD_X.pts();
        let pad_y = control_space::PAD_Y.pts();
        let icon_gap = control_space::ICON_GAP.pts();
        let sc_gap = control_space::SHORTCUT_GAP.pts();
        let part_gap = control_space::PART_GAP.pts();

        // Copy feedback: hold rest-state width, paint check for 1s after click.
        let feedback_id = self.copy_feedback;
        let mut showing_check = false;
        if let Some(fid) = feedback_id {
            let now = ui.input(|i| i.time);
            let until = ui
                .ctx()
                .data(|d| d.get_temp::<f64>(fid.with("copied_until")))
                .unwrap_or(0.0);
            showing_check = now < until;
            if showing_check {
                ui.ctx().request_repaint();
                // Lock width to the rest label (usually "Copy").
                let rest_g = ui.painter().layout_no_wrap(
                    self.label.clone(),
                    font.clone(),
                    Color32::PLACEHOLDER,
                );
                let rest_w = rest_g.size().x + pad_x * 2.0;
                self.min_width = Some(self.min_width.unwrap_or(0.0).max(rest_w));
                self.label.clear();
                self.icon = Some(phosphor::CHECK);
                self.shortcut = None;
            }
        }

        let icon_galley = self.icon.map(|g| {
            ui.painter()
                .layout_no_wrap(g.to_owned(), phosphor_ui_font_id(), Color32::PLACEHOLDER)
        });
        let icon_w = icon_galley.as_ref().map(|g| g.size().x).unwrap_or(0.0);
        let has_label = !self.label.is_empty();
        let icon_only = icon_galley.is_some() && !has_label;
        let icon_block = if icon_galley.is_some() {
            icon_w + if has_label { icon_gap } else { 0.0 }
        } else {
            0.0
        };

        // (phosphor icon?, galley) — mono vs icon paint differently (bottom vs mid).
        let sc_parts: Vec<(bool, Arc<Galley>)> = self
            .shortcut
            .map(|sc| layout_shortcut_parts(ui, sc))
            .unwrap_or_default();
        let sc_inner_w: f32 = sc_parts
            .iter()
            .enumerate()
            .map(|(i, (_, g))| g.size().x + if i > 0 { part_gap } else { 0.0 })
            .sum();
        let sc_w = if sc_parts.is_empty() { 0.0 } else { sc_inner_w + sc_gap };

        let label_max = self
            .max_width
            .map(|mw| (mw - pad_x * 2.0 - sc_w - icon_block).max(24.0));
        let galley = if !has_label {
            None
        } else if let Some(max_w) = label_max {
            Some(
                ui.painter()
                    .layout(self.label.clone(), font, Color32::PLACEHOLDER, max_w),
            )
        } else {
            Some(
                ui.painter()
                    .layout_no_wrap(self.label.clone(), font, Color32::PLACEHOLDER),
            )
        };
        let label_size = galley.as_ref().map(|g| g.size()).unwrap_or(Vec2::ZERO);

        let content_h = label_size
            .y
            .max(icon_galley.as_ref().map(|g| g.size().y).unwrap_or(0.0));
        let mut desired =
            vec2(label_size.x + icon_block + sc_w + pad_x * 2.0, content_h + pad_y * 2.0);
        desired.y = self.height.unwrap_or(desired.y.max(control_height()));
        if let Some(min_w) = self.min_width {
            desired.x = desired.x.max(min_w);
        }
        if let Some(mw) = self.max_width {
            desired.x = desired.x.min(mw).max(0.0);
        }

        let sense = if self.enabled { sense_click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(desired, sense);

        let (fills, text) = self.fills_and_text();
        let fill = if self.enabled {
            crate::components::foundation::interact::interact_fill_response(
                ui.ctx(),
                &response,
                fills,
            )
        } else {
            fills.rest
        };
        let kbd = match self.treatment {
            // Same ink as the label on the solid plate (brand or danger).
            Treatment::Primary => text,
            Treatment::Quiet => {
                if self.enabled {
                    t.neutral_fg_secondary()
                } else {
                    text
                }
            }
        };

        let radius = Radius::Control.corner();
        ui.painter().rect_filled(rect, radius, fill);

        // Measure → place: pads + content x-walk (no horizontal placer / Center air).
        paint_control_pads(ui, rect, control_space::PAD_X, control_space::PAD_Y);
        let mid = inset(rect, pad_x, pad_y);
        // Label + kbd share layout-box **bottoms** (mesh-centering made mono `esc`
        // sit high vs body — #21). Block is vertically centered in the control.
        let sc_max_h = sc_parts
            .iter()
            .map(|(_, g)| g.size().y)
            .fold(0.0_f32, f32::max);
        let icon_h = icon_galley.as_ref().map(|g| g.size().y).unwrap_or(0.0);
        let block_h = label_size.y.max(icon_h).max(sc_max_h).max(1.0);
        let block_top = mid.center().y - block_h / 2.0;
        // Icon-only (copy check): center in the content band so width can match "Copy".
        let mut x = if icon_only { mid.center().x - icon_w / 2.0 } else { mid.left() };

        if let Some(ig) = icon_galley {
            let iw = ig.size().x;
            let ir = egui::Rect::from_min_size(pos2(x, block_top), vec2(iw, block_h));
            paint_galley_layout_mid(ui.painter(), ir, ig, text);
            x += iw;
            if has_label {
                x += icon_gap;
            }
        }
        if let Some(galley) = galley {
            let label_w = galley.size().x;
            let label_rect = egui::Rect::from_min_size(pos2(x, block_top), vec2(label_w, block_h));
            paint_galley_layout_bottom(ui.painter(), label_rect, galley, text);
            x += label_w;
        }
        if !sc_parts.is_empty() {
            x += sc_gap;
            for (i, (mid_align, sg)) in sc_parts.into_iter().enumerate() {
                if i > 0 {
                    x += part_gap;
                }
                let w = sg.size().x;
                let pr = egui::Rect::from_min_size(pos2(x, block_top), vec2(w, block_h));
                if mid_align {
                    paint_galley_layout_mid(ui.painter(), pr, sg, kbd);
                } else {
                    paint_galley_layout_bottom(ui.painter(), pr, sg, kbd);
                }
                x += w;
            }
        }
        let _ = x;

        if self.enabled && response.has_focus() {
            let focus_c = match self.treatment {
                Treatment::Primary => t.neutral(),
                Treatment::Quiet => t.neutral_fg(),
            };
            ui.painter().rect_stroke(
                rect,
                radius,
                Stroke::new(STROKE_HAIRLINE, focus_c),
                StrokeKind::Outside,
            );
        }

        if response.clicked() {
            if let Some(fid) = feedback_id {
                let now = ui.input(|i| i.time);
                ui.ctx().data_mut(|d| {
                    d.insert_temp(fid.with("copied_until"), now + COPY_FEEDBACK_SECS);
                });
                ui.ctx().request_repaint();
            }
        }
        let _ = showing_check;

        response
    }

    /// Settled fills per state + label ink. Motion is owned by [`interact_fill`].
    fn fills_and_text(&self) -> (ControlFills, Color32) {
        let t = self.tokens;

        if !self.enabled {
            return match self.treatment {
                Treatment::Primary => (
                    ControlFills {
                        rest: Color32::TRANSPARENT,
                        hover: Color32::TRANSPARENT,
                        press: Color32::TRANSPARENT,
                    },
                    t.neutral_fg_secondary(),
                ),
                Treatment::Quiet => (
                    ControlFills {
                        rest: t.neutral_bg(),
                        hover: t.neutral_bg(),
                        press: t.neutral_bg(),
                    },
                    t.neutral_fg_secondary(),
                ),
            };
        }

        match self.treatment {
            Treatment::Primary => {
                let base = match self.tone {
                    Tone::Brand => t.neutral_fg(),
                    Tone::Danger => t.danger(),
                    Tone::Accent => t.accent(),
                };
                (
                    ControlFills {
                        rest: base,
                        hover: base.lerp_to_gamma(t.neutral_bg(), BG_HOVER),
                        press: base.lerp_to_gamma(t.neutral_bg(), BG_PRESS),
                    },
                    t.neutral_bg(),
                )
            }
            Treatment::Quiet => {
                let ink = match self.tone {
                    Tone::Brand => t.neutral_fg(),
                    Tone::Danger => t.danger(),
                    // Quiet+accent is rare; ink-only emphasis if someone chains it.
                    Tone::Accent => t.accent(),
                };
                let fills = crate::components::foundation::interact::quiet_canvas_fills(t);
                (fills, ink)
            }
        }
    }
}

/// `(mid_align, galley)` — Icon + body Mono mid-align (⌘N energy); MonoSm
/// (`esc`) bottom-aligns with the button label.
fn layout_shortcut_parts(ui: &Ui, sc: Shortcut) -> Vec<(bool, Arc<Galley>)> {
    let mono_body = FontId::new(TypeRole::Body.size(), FontFamily::Monospace);
    let mono_sm = TypeRole::Mono.font_id();
    let icon = phosphor_ui_font_id();
    sc.parts
        .iter()
        .map(|part| {
            let (mid, text, font) = match *part {
                KbdPart::Icon(s) => (true, s.to_owned(), icon.clone()),
                KbdPart::Mono(s) => (true, s.to_owned(), mono_body.clone()),
                KbdPart::MonoSm(s) => (false, s.to_owned(), mono_sm.clone()),
            };
            (
                mid,
                ui.painter()
                    .layout_no_wrap(text, font, Color32::PLACEHOLDER),
            )
        })
        .collect()
}

/// Layout-box bottom on `slot.bottom()` — body + mono kbd.
fn paint_galley_layout_bottom(
    painter: &egui::Painter, slot: egui::Rect, galley: Arc<Galley>, color: Color32,
) {
    let pos = pos2(slot.left(), slot.bottom() - galley.size().y);
    painter.galley(pos, galley, color);
}

/// Layout-box vertical mid in `slot` — Phosphor (no mesh_bounds).
fn paint_galley_layout_mid(
    painter: &egui::Painter, slot: egui::Rect, galley: Arc<Galley>, color: Color32,
) {
    let pos = pos2(slot.left(), slot.center().y - galley.size().y / 2.0);
    painter.galley(pos, galley, color);
}

/// Frameless square toolbar / view control.
///
/// Hit target = [`control_height`]. **No resting plate** — selection is ink only
/// (active = full `fg`, idle = muted).
///
/// `ground` is the **opaque parent fill** under this control (e.g. `t.neutral_bg_secondary()`
/// or `t.neutral_bg()`). Washes are relative to that color only:
/// `ink_wash(ground, FG_HOVER × hover)` — never a hardcoded canvas plate.
/// Caller must pass the real ground; wrong ground is a light-mode flash.
///
/// Hit/hover wash is [`control_height`] square. Glyph size is fixed (phosphor UI
/// font) — for a tighter hit with the same mark, use [`icon_button_hit`].
///
/// One [`animate_bool_with_time`] clock drives wash + muted→fg ink together.
pub fn icon_button(
    ui: &mut Ui, t: &Theme, icon: &'static str, active: bool, ground: Color32,
) -> Response {
    icon_button_hit(ui, t, icon, active, ground, control_height())
}

/// Like [`icon_button`], but hit + hover wash use `hit` (glyph size unchanged).
///
/// Titleband / dense chrome: pass a hit smaller than [`control_height`] so the
/// wash is not flush to the window edge while the mark stays the same size.
pub fn icon_button_hit(
    ui: &mut Ui, t: &Theme, icon: &'static str, active: bool, ground: Color32, hit: f32,
) -> Response {
    let hit = hit.max(1.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(hit, hit), sense_click());
    let hover = icon_hover_t(ui, &resp, rect);
    // Ghost wash on hover only — darkens `ground`, never replaces it with canvas.
    if hover > 0.0 {
        let amt = if active {
            // Slightly stronger so active still reads pressable.
            FG_PRESS * hover
        } else {
            FG_HOVER * hover
        };
        let wash = t.wash_toward_neutral_fg(ground, amt);
        // Sm matches compact icon hits (tab close X); Control reads as a pill
        // on titleband-sized (~22pt) washes.
        ui.painter().rect_filled(rect, Radius::Sm.corner(), wash);
    }
    // Idle muted → primary on hover; active stays primary.
    let color = if active {
        t.neutral_fg()
    } else {
        t.neutral_fg_secondary()
            .lerp_to_gamma(t.neutral_fg(), hover)
    };
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), phosphor_ui_font_id(), Color32::PLACEHOLDER);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, color);
    resp
}

/// Shared hover probe + ease for frameless icon controls.
///
/// `Response::hovered()` is layer/topmost-aware (correct under Foreground
/// Areas). Fall back to a rect hit so a single lag frame doesn't snap the ease
/// off. One clock (`HOVER_ANIM_SECS`) drives wash and ink together.
///
/// **Ease in, snap out.** Leaving with a slow ease-out fights sibling washes
/// (e.g. Shared Save → file row): button darkens toward idle while the row
/// lightens toward hover. Snap-off cancels that clash for free.
fn icon_hover_t(ui: &Ui, resp: &Response, rect: egui::Rect) -> f32 {
    let over = resp.hovered() || ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    let id = resp.id.with("icon_hov");
    if over {
        ui.ctx().animate_bool_with_time(id, true, HOVER_ANIM_SECS)
    } else {
        // Force the clock to idle this frame (duration 0).
        let _ = ui.ctx().animate_bool_with_time(id, false, 0.0);
        0.0
    }
}
