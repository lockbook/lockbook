//! Buttons: primary (ink fill), secondary (off-canvas plate), or quiet (canvas
//! rest). Danger / accent are tones on the solid primary plate (same hover/press
//! wash toward `neutral_bg`; label and kbd use `neutral_bg` ink).
//! Padding uses [`Spacer`]s (`control` space tokens) so F2 paints pad bands.

use egui::{
    Color32, FontFamily, FontId, Galley, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use std::sync::Arc;

use crate::style::chrome::{
    HOVER_ANIM_SECS, KbdPart, Radius, STROKE_HAIRLINE, Shortcut, control_height, phosphor,
    phosphor_font_id, phosphor_ui_font_id,
};
use crate::style::color::{BG_HOVER, BG_PRESS, FG_HOVER, Theme};
use crate::style::interact::{ControlFills, sense_click};
use crate::style::layout::{inset, paint_control_pads};
use crate::style::space::control as control_space;
use crate::style::typography::TypeRole;

#[derive(Clone, Copy)]
enum Treatment {
    Primary,
    /// Off-canvas plate (`neutral_bg_secondary`) — quieter than primary, visible
    /// on canvas (unlike quiet, whose rest matches the surface).
    Secondary,
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

/// Invisible pill-slot metrics for button keyboard shortcuts: matches the
/// spacing of an 18pt tonal keycap (`8pt` gap + `5pt` inner x-pad = `13pt`
/// from label, `5pt` inner x-pad + `6pt` trailing button pad = `11pt` to the
/// right edge) without painting a background fill.
const SHORTCUT_FONT_SIZE: f32 = 11.5;
const SHORTCUT_PART_GAP: f32 = 1.5;
const SHORTCUT_SLOT_GAP: f32 = 8.0;
const SHORTCUT_SLOT_PAD_X: f32 = 5.0;
const SHORTCUT_SLOT_MIN_W: f32 = 18.0;
const SHORTCUT_TRAILING_PAD_X: f32 = 6.0;

/// How long a copy control shows the check instead of its label.
pub const COPY_FEEDBACK_SECS: f64 = 1.2;

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

    /// Secondary plate on canvas — `neutral_bg_secondary` rest, same ground as
    /// chips / unfocused fields. Visible without the ink fill of [`Self::primary`].
    pub fn secondary(t: &'a Theme, label: impl Into<String>) -> Self {
        Self::new(t, label, Treatment::Secondary)
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

    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = Some(w);
        self
    }

    /// Leading Phosphor glyph. Combine with a label so the mark is not the
    /// only way to read the control.
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
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

    /// Exact width [`Self::show`] will allocate, for explicit rect placement.
    pub fn measure_width(&self, ui: &Ui) -> f32 {
        let font = TypeRole::Body.font_id();
        let pad_x = control_space::PAD_X.pts();
        let icon_gap = control_space::ICON_GAP.pts();
        let icon_w = self
            .icon
            .map(|g| {
                ui.painter()
                    .layout_no_wrap(g.to_owned(), phosphor_ui_font_id(), Color32::PLACEHOLDER)
                    .size()
                    .x
            })
            .unwrap_or(0.0);
        let has_label = !self.label.is_empty();
        let icon_block =
            if self.icon.is_some() { icon_w + if has_label { icon_gap } else { 0.0 } } else { 0.0 };
        let sc_parts: Vec<ShortcutPart> = self
            .shortcut
            .map(|sc| layout_shortcut_parts(ui, sc))
            .unwrap_or_default();
        let has_shortcut = !sc_parts.is_empty();
        let sc_slot_w = if has_shortcut {
            (shortcut_content_width(&sc_parts) + SHORTCUT_SLOT_PAD_X * 2.0).max(SHORTCUT_SLOT_MIN_W)
        } else {
            0.0
        };
        let sc_w = if has_shortcut { SHORTCUT_SLOT_GAP + sc_slot_w } else { 0.0 };
        let pad_x_right = if has_shortcut { SHORTCUT_TRAILING_PAD_X } else { pad_x };
        let label_w = if has_label {
            ui.painter()
                .layout_no_wrap(self.label.clone(), font, Color32::PLACEHOLDER)
                .size()
                .x
        } else {
            0.0
        };
        let mut w = label_w + icon_block + sc_w + pad_x + pad_x_right;
        if let Some(min_w) = self.min_width {
            w = w.max(min_w);
        }
        if let Some(mw) = self.max_width {
            w = w.min(mw).max(0.0);
        }
        w.ceil()
    }

    pub fn show(mut self, ui: &mut Ui) -> Response {
        let t = self.tokens;
        let font = TypeRole::Body.font_id();
        let pad_x = control_space::PAD_X.pts();
        let pad_y = control_space::PAD_Y.pts();
        let icon_gap = control_space::ICON_GAP.pts();

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

        // Shortcut parts laid out in unified Proportional UI sans (avoiding
        // Monospace's +0.1 MONO_Y_OFFSET sag and serifed I) inside an invisible
        // pill-shaped slot so spacing matches a keycap without adding a fill.
        let sc_parts: Vec<ShortcutPart> = self
            .shortcut
            .map(|sc| layout_shortcut_parts(ui, sc))
            .unwrap_or_default();
        let has_shortcut = !sc_parts.is_empty();
        let sc_content_w = shortcut_content_width(&sc_parts);
        let sc_slot_w = if has_shortcut {
            (sc_content_w + SHORTCUT_SLOT_PAD_X * 2.0).max(SHORTCUT_SLOT_MIN_W)
        } else {
            0.0
        };
        let sc_w = if has_shortcut { SHORTCUT_SLOT_GAP + sc_slot_w } else { 0.0 };
        let pad_x_right = if has_shortcut { SHORTCUT_TRAILING_PAD_X } else { pad_x };

        let label_max = self
            .max_width
            .map(|mw| (mw - pad_x - pad_x_right - sc_w - icon_block).max(24.0));
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
            vec2(label_size.x + icon_block + sc_w + pad_x + pad_x_right, content_h + pad_y * 2.0);
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
            crate::style::interact::interact_fill_response(ui.ctx(), &response, fills)
        } else {
            fills.rest
        };
        let kbd = match self.treatment {
            // Quieter hint ink on the solid primary plate so the shortcut doesn't
            // compete with the label; secondary/quiet already use secondary fg.
            Treatment::Primary => {
                if self.enabled {
                    text.gamma_multiply(0.58)
                } else {
                    text
                }
            }
            Treatment::Secondary | Treatment::Quiet => {
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

        // Optical vertical alignment:
        // - Label and text shortcut parts share one typographic baseline, centered
        //   on the label's cap height inside the control.
        // - Icons (leading glyph, ⌘, ↩) center their actual ink bounds on that
        //   cap-height midpoint so symbols and letters align optically.
        let h_ref =
            ui.painter()
                .layout_no_wrap("H".into(), TypeRole::Body.font_id(), Color32::PLACEHOLDER);
        let h_ink_mid = galley_ink_mid_y(&h_ref);
        let cap_mid_y = rect.center().y;
        let label_top = cap_mid_y - h_ink_mid;
        let baseline_y = label_top + galley_baseline(&h_ref);

        // Icon-only (copy check): center horizontally in the content band so width can match "Copy".
        let mut x = if icon_only { mid.center().x - icon_w / 2.0 } else { mid.left() };

        if let Some(ig) = icon_galley {
            let iw = ig.size().x;
            paint_galley_ink_mid(ui.painter(), x, cap_mid_y, ig, text);
            x += iw;
            if has_label {
                x += icon_gap;
            }
        }
        if let Some(galley) = galley {
            let label_w = galley.size().x;
            paint_galley_baseline(ui.painter(), x, baseline_y, galley, text);
            x += label_w;
        }
        if !sc_parts.is_empty() {
            let sc_font = FontId::new(SHORTCUT_FONT_SIZE, FontFamily::Proportional);
            let kbd_h = ui
                .painter()
                .layout_no_wrap("H".into(), sc_font, Color32::PLACEHOLDER);
            let slot_mid_y = rect.center().y;
            let slot_baseline = slot_mid_y - galley_ink_mid_y(&kbd_h) + galley_baseline(&kbd_h);
            let slot_center_x = x + SHORTCUT_SLOT_GAP + sc_slot_w / 2.0;
            let mut gx = slot_center_x - sc_content_w / 2.0;
            for part in sc_parts {
                gx += part.gap_before;
                let w = part.galley.size().x;
                if part.center_ink {
                    paint_galley_ink_mid(ui.painter(), gx, slot_mid_y, part.galley, kbd);
                } else {
                    paint_galley_baseline(ui.painter(), gx, slot_baseline, part.galley, kbd);
                }
                gx += w;
            }
            x += sc_w;
        }
        let _ = x;

        if self.enabled && response.has_focus() {
            let focus_c = match self.treatment {
                Treatment::Primary => t.neutral(),
                Treatment::Secondary | Treatment::Quiet => t.neutral_fg(),
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
                Treatment::Secondary => (
                    ControlFills {
                        rest: t.neutral_bg_secondary(),
                        hover: t.neutral_bg_secondary(),
                        press: t.neutral_bg_secondary(),
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
            Treatment::Secondary => {
                let ink = match self.tone {
                    Tone::Brand => t.neutral_fg(),
                    Tone::Danger => t.danger(),
                    Tone::Accent => t.accent(),
                };
                (crate::style::interact::quiet_secondary_fills(t), ink)
            }
            Treatment::Quiet => {
                let ink = match self.tone {
                    Tone::Brand => t.neutral_fg(),
                    Tone::Danger => t.danger(),
                    // Quiet+accent is rare; ink-only emphasis if someone chains it.
                    Tone::Accent => t.accent(),
                };
                let fills = crate::style::interact::quiet_canvas_fills(t);
                (fills, ink)
            }
        }
    }
}

struct ShortcutPart {
    galley: Arc<Galley>,
    center_ink: bool,
    gap_before: f32,
}

/// Lay out shortcut parts in unified Proportional UI sans so modifiers and key
/// letters share one stroke weight without `Monospace`'s `MONO_Y_OFFSET` sag.
/// On macOS, `⌘` and `⏎` use native Unicode glyphs in the same Proportional
/// font; elsewhere Phosphor icons stay on the icon font to avoid missing glyphs.
fn layout_shortcut_parts(ui: &Ui, sc: Shortcut) -> Vec<ShortcutPart> {
    let text_font = FontId::new(SHORTCUT_FONT_SIZE, FontFamily::Proportional);
    let icon_font = phosphor_font_id(SHORTCUT_FONT_SIZE + 0.5);
    let mut prev_ended_plus = false;
    sc.parts
        .iter()
        .enumerate()
        .map(|(i, part)| {
            let gap_before = if i == 0 {
                0.0
            } else if prev_ended_plus {
                0.5
            } else {
                SHORTCUT_PART_GAP
            };
            let (center_ink, text, font) = match *part {
                KbdPart::Icon(s) => {
                    #[cfg(target_os = "macos")]
                    let native = match s {
                        phosphor::COMMAND => Some("⌘"),
                        phosphor::KEY_RETURN => Some("⏎"),
                        _ => None,
                    };
                    #[cfg(not(target_os = "macos"))]
                    let native: Option<&str> = None;

                    if let Some(sym) = native {
                        (true, sym.to_owned(), text_font.clone())
                    } else {
                        (true, s.to_owned(), icon_font.clone())
                    }
                }
                KbdPart::Mono(s) | KbdPart::MonoSm(s) => {
                    // Lowercase-only words like "esc" have no cap height; center
                    // their ink directly on the slot midline instead of using "H".
                    let lowercase_only = !s.is_empty() && s.chars().all(|c| !c.is_uppercase());
                    (lowercase_only, s.to_owned(), text_font.clone())
                }
            };
            prev_ended_plus = text.ends_with('+');
            let galley = ui
                .painter()
                .layout_no_wrap(text, font, Color32::PLACEHOLDER);
            ShortcutPart { galley, center_ink, gap_before }
        })
        .collect()
}

fn shortcut_content_width(parts: &[ShortcutPart]) -> f32 {
    parts.iter().map(|p| p.gap_before + p.galley.size().x).sum()
}

/// First row's typographic baseline relative to the galley's top-left origin.
fn galley_baseline(g: &Galley) -> f32 {
    g.rows.first().map_or(g.size().y, |r| {
        r.pos.y
            + r.row
                .glyphs
                .first()
                .map_or(r.rect().bottom(), |gl| gl.pos.y)
    })
}

/// Optical vertical center of a galley's actual inked pixels (falls back to
/// layout-box center in headless contexts with empty mesh bounds).
fn galley_ink_mid_y(g: &Galley) -> f32 {
    if g.mesh_bounds.height() > 0.0 { g.mesh_bounds.center().y } else { g.size().y / 2.0 }
}

/// Place a text galley so its first baseline sits on `baseline_y`.
fn paint_galley_baseline(
    painter: &egui::Painter, x: f32, baseline_y: f32, galley: Arc<Galley>, color: Color32,
) {
    let pos = pos2(x, baseline_y - galley_baseline(&galley));
    painter.galley(pos, galley, color);
}

/// Place an icon galley so its inked center sits on `cap_mid_y`.
fn paint_galley_ink_mid(
    painter: &egui::Painter, x: f32, cap_mid_y: f32, galley: Arc<Galley>, color: Color32,
) {
    let pos = pos2(x, cap_mid_y - galley_ink_mid_y(&galley));
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
/// One [`animate_bool_with_time`] clock drives the hover wash.
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
    icon_button_glyph(ui, t, icon, active, ground, hit, TypeRole::Body.size())
}

/// [`icon_button_hit`] with an explicit Phosphor size (titleband toolbar).
pub fn icon_button_glyph(
    ui: &mut Ui, t: &Theme, icon: &'static str, active: bool, ground: Color32, hit: f32, glyph: f32,
) -> Response {
    icon_button_mark(ui, t, active, ground, hit, |painter, rect, color| {
        let g = painter.layout_no_wrap(icon.into(), phosphor_font_id(glyph), Color32::PLACEHOLDER);
        painter.galley(rect.center() - g.size() / 2.0, g, color);
    })
}

/// [`icon_button_glyph`] for a mark Phosphor lacks: `mark` paints it in the
/// hit rect, in the ink color it is given.
pub fn icon_button_mark(
    ui: &mut Ui, t: &Theme, active: bool, ground: Color32, hit: f32,
    mark: impl FnOnce(&egui::Painter, egui::Rect, Color32),
) -> Response {
    let hit = hit.max(1.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(hit, hit), sense_click());
    let plate = icon_plate(ui, t, &resp, ground);
    if plate != ground {
        // Sm matches compact icon hits (tab close X); Control reads as a
        // pill on titleband-sized (~22pt) washes.
        ui.painter().rect_filled(rect, Radius::Sm.corner(), plate);
    }
    // Idle: foreground. Applied / selected: accent.
    let color = if active { t.accent() } else { t.neutral_fg() };
    mark(ui.painter(), rect, color);
    resp
}

/// What an [`icon_button_mark`] shows behind its mark this frame: `ground`,
/// or the ghost wash while hovered. Same amount whether or not the mark is
/// active (active is ink, not plate); it darkens `ground`, never replaces it.
pub fn icon_plate(ui: &Ui, t: &Theme, resp: &Response, ground: Color32) -> Color32 {
    let hover = icon_hover_t(ui, resp, resp.rect);
    if hover > 0.0 { t.wash_toward_neutral_fg(ground, FG_HOVER * hover) } else { ground }
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
