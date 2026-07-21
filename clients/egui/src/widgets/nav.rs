//! Sidebar chrome primitives, sharing the file tree's visual language: a
//! frameless icon button for the toolbar and Apple-style action chips (New /
//! Import / Search).
//!
//! Chip chrome is design-system **chip**: solid canvas rest, outline only on
//! hover (not a lighten-to-bg wash — chips already sit off surface).

use egui::{CornerRadius, FontId, Response, Sense, Ui, pos2, vec2};

use crate::theme::{icons, tokens::Tokens};

/// Side of the square `icon_button`; exported so callers can center it.
pub const ICON_BUTTON_SIZE: f32 = 26.0;

/// Apple `actionChip` corner radius (`RoundedRectangle(cornerRadius: 7)`).
const CHIP_RADIUS: u8 = 7;
/// Vertical padding inside a chip (Apple `.padding(.vertical, 6)`).
const CHIP_PAD_Y: f32 = 6.0;
/// Horizontal pad required around icon+label for a comfortable labeled chip.
const CHIP_PAD_X: f32 = 8.0;
/// Gap between icon and label inside a chip.
const CHIP_ICON_GAP: f32 = 5.0;
/// Gap between chips (Apple `HStack(spacing: 8)`).
const CHIP_GAP: f32 = 8.0;

/// A frameless square icon button — toolbar affordance. Idle ink is muted;
/// hover eases ink toward primary and may show a light wash (no resting plate —
/// that language belongs to action chips).
pub fn icon_button(ui: &mut Ui, t: &Tokens, icon: &str) -> Response {
    icon_button_active(ui, t, icon, false)
}

/// Like `icon_button`, with an explicit selected/active chrome state.
///
/// View toggles (Files / Recents / Shared): selection is **ink hierarchy** only
/// (active = full `fg`, idle = muted). No solid selected fill — keeps them
/// quieter than the canvas action chips below and less heavy in light mode.
pub fn icon_button_active(ui: &mut Ui, t: &Tokens, icon: &str, active: bool) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(vec2(ICON_BUTTON_SIZE, ICON_BUTTON_SIZE), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    // Ghost wash on hover only — never a resting/selected plate.
    if hover > 0.0 && !active {
        ui.painter().rect_filled(
            rect,
            6.0,
            t.canvas().lerp_to_gamma(t.surface_raised(), 0.55 * hover),
        );
    } else if hover > 0.0 && active {
        // Active + hover: very light wash so it still feels pressable.
        ui.painter().rect_filled(
            rect,
            6.0,
            t.canvas().lerp_to_gamma(t.fg(), 0.04 * hover),
        );
    }
    // Idle muted → primary on hover; active stays primary.
    let color = if active {
        t.fg()
    } else {
        t.text_muted().lerp_to_gamma(t.fg(), hover)
    };
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(18.0), color);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, color);
    resp
}

/// A window-control button (minimize / maximize / close) — frameless like
/// `icon_button` but with a smaller mark. `danger` tints the hover state layer
/// and glyph toward red, for the close button. Windows/Linux only.
#[cfg(not(target_os = "macos"))]
pub fn window_button(ui: &mut Ui, t: &Tokens, icon: &str, danger: bool) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(vec2(ICON_BUTTON_SIZE, ICON_BUTTON_SIZE), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let accent = if danger { t.danger() } else { t.fg() };
    if hover > 0.0 {
        ui.painter().rect_filled(
            rect,
            6.0,
            t.canvas().lerp_to_gamma(t.surface_raised(), hover),
        );
    }
    let color = t.text_muted().lerp_to_gamma(accent, hover);
    let g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(15.0), color);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, color);
    resp
}

/// Paint chip chrome (see `button::paint_chip`).
///
/// `base` is the resting fill — canvas on surface chrome (header chips + pins).
/// Pass `animate_bool` hover so the outline alpha eases in/out.
pub fn paint_chip_chrome(
    ui: &Ui, t: &Tokens, rect: egui::Rect, radius: impl Into<CornerRadius>, hover: f32,
    pressed: bool, base: egui::Color32,
) {
    crate::widgets::button::paint_chip(ui, t, rect, radius, hover, pressed, base);
}

/// One Apple-style sidebar action chip: icon + optional label, equal-width in a
/// row, `cornerRadius: 7`. When `icon_only`, the label is omitted (tooltip).
pub fn action_chip(ui: &mut Ui, t: &Tokens, icon: &str, label: &str, icon_only: bool) -> Response {
    let font = FontId::proportional(13.0); // ~callout
    let icon_font = icons::font(14.0);
    let icon_g = ui
        .painter()
        .layout_no_wrap(icon.into(), icon_font, t.fg());
    let label_g = if icon_only {
        None
    } else {
        Some(
            ui.painter()
                .layout_no_wrap(label.into(), font, t.fg()),
        )
    };
    // Natural height from content + Apple vertical pad; width comes from layout.
    let text_h = label_g.as_ref().map(|g| g.size().y).unwrap_or(0.0);
    let h = icon_g.size().y.max(text_h) + CHIP_PAD_Y * 2.0;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    if icon_only {
        workspace_rs::widgets::tip_text(ui.ctx(), &resp, label);
    }
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    paint_chip_chrome(
        ui,
        t,
        rect,
        CornerRadius::same(CHIP_RADIUS),
        hover,
        resp.is_pointer_button_down_on(),
        t.canvas(),
    );

    let ink = t.fg();
    let cy = rect.center().y;
    if let Some(label_g) = label_g {
        let content_w = icon_g.size().x + CHIP_ICON_GAP + label_g.size().x;
        let mut x = rect.center().x - content_w / 2.0;
        ui.painter()
            .galley(pos2(x, cy - icon_g.size().y / 2.0), icon_g.clone(), ink);
        x += icon_g.size().x + CHIP_ICON_GAP;
        ui.painter()
            .galley(pos2(x, cy - label_g.size().y / 2.0), label_g, ink);
    } else {
        ui.painter()
            .galley(rect.center() - icon_g.size() / 2.0, icon_g, ink);
    }
    resp
}

/// Minimum chip width that can still show icon + the longest label comfortably.
fn labeled_chip_min_width(ui: &Ui, t: &Tokens, labels: &[&str]) -> f32 {
    let font = FontId::proportional(13.0);
    let icon_font = icons::font(14.0);
    // Phosphor icons are roughly square at the font size; measure one glyph.
    let icon_w = ui
        .painter()
        .layout_no_wrap(icons::SEARCH.into(), icon_font, t.fg())
        .size()
        .x;
    let max_label = labels
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap((*l).into(), font.clone(), t.fg())
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    CHIP_PAD_X * 2.0 + icon_w + CHIP_ICON_GAP + max_label
}

/// Equal-width chip row: New / Import / Search (Apple `sidebarActions`).
/// Collapses to icon-only when each chip is too narrow for icon+label+pad.
/// Returns click flags in order `(new, import, search)`.
pub fn action_chip_row(ui: &mut Ui, t: &Tokens) -> (bool, bool, bool) {
    let n = 3usize;
    let total = ui.available_width();
    let chip_w = ((total - CHIP_GAP * (n as f32 - 1.0)) / n as f32).max(0.0);
    let specs = [
        (icons::NOTE_PENCIL, "New"),
        (icons::DOWNLOAD_SIMPLE, "Import"),
        (icons::SEARCH, "Search"),
    ];
    let labels: Vec<&str> = specs.iter().map(|(_, l)| *l).collect();
    let icon_only = chip_w < labeled_chip_min_width(ui, t, &labels);
    let mut hits = [false; 3];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = CHIP_GAP;
        for (i, (icon, label)) in specs.iter().enumerate() {
            ui.scope(|ui| {
                ui.set_width(chip_w);
                if action_chip(ui, t, icon, label, icon_only).clicked() {
                    hits[i] = true;
                }
            });
        }
    });
    (hits[0], hits[1], hits[2])
}

