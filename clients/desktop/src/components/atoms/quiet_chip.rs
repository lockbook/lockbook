//! Quiet chip: secondary plate on canvas, icon + label in one body line-box.
//!
//! Shared shell for sidebar **action** chips (Create / Import / Search) and
//! **pin** chips. Call sites keep domain behavior (queue, menus, file names).

use egui::{Color32, Response, Ui, pos2, vec2};

use crate::components::foundation::chrome::{
    HOVER_ANIM_SECS, Radius, control_height, phosphor_ui_font_id,
};
use crate::components::foundation::color::{FG_HOVER, FG_PRESS, Theme};
use crate::components::foundation::layout::ui_width;
use crate::components::foundation::space::Space;
use crate::components::foundation::typography::TypeRole;

/// Chip height — same ladder as buttons / fields.
pub fn height() -> f32 {
    control_height()
}

pub const PAD_X: Space = Space::Sm;
pub const ICON_GAP: Space = Space::Xs;

/// How icon+label sit in the plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuietChipAlign {
    /// Pin chips — pad-left, name fills residual.
    Start,
    /// Action chips — cluster centered (clamped to pad).
    Center,
}

/// Label draw mode.
pub enum QuietChipLabel<'a> {
    /// Plain body string (Create / Import / Search).
    Plain(&'a str),
    /// Truncating file name (pins).
    FileName(&'a str),
}

/// Secondary plate + optional icon/label. Returns the click response.
pub fn quiet_chip(
    ui: &mut Ui, t: &Theme, icon: &str, icon_ink: Color32, label: Option<QuietChipLabel<'_>>,
    align: QuietChipAlign,
) -> Response {
    let h = height();
    let w = ui_width(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), crate::components::sense_click());

    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    let hover =
        ui.ctx()
            .animate_bool_with_time(resp.id.with("quiet_chip_hov"), over, HOVER_ANIM_SECS);
    let ground = t.neutral_bg_secondary();
    let fill = if resp.is_pointer_button_down_on() || resp.clicked() {
        t.wash_toward_neutral_fg(ground, FG_PRESS)
    } else if hover > 0.0 {
        let hover_c = t.wash_toward_neutral_fg(ground, FG_HOVER);
        ground.lerp_to_gamma(hover_c, hover)
    } else {
        ground
    };
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);

    let pad = PAD_X.pts();
    let gap = ICON_GAP.pts();
    let lh = TypeRole::Body.line_height();
    let line_top = rect.center().y - lh / 2.0;

    let ig = ui
        .painter()
        .layout_no_wrap(icon.into(), phosphor_ui_font_id(), icon_ink);

    match label {
        None => {
            ui.painter().galley(
                pos2(rect.center().x - ig.size().x / 2.0, line_top + (lh - ig.size().y) / 2.0),
                ig,
                icon_ink,
            );
        }
        Some(QuietChipLabel::Plain(text)) => {
            let lg =
                ui.painter()
                    .layout_no_wrap(text.into(), TypeRole::Body.font_id(), t.neutral_fg());
            let icon_w = ig.size().x;
            let icon_h = ig.size().y;
            let content_w = icon_w + gap + lg.size().x;
            let mut x = match align {
                QuietChipAlign::Center => {
                    (rect.center().x - content_w / 2.0).max(rect.left() + pad)
                }
                QuietChipAlign::Start => rect.left() + pad,
            };
            ui.painter()
                .galley(pos2(x, line_top + (lh - icon_h) / 2.0), ig, icon_ink);
            x += icon_w + gap;
            ui.painter()
                .galley(pos2(x, line_top + (lh - lg.size().y) / 2.0), lg, t.neutral_fg());
        }
        Some(QuietChipLabel::FileName(name)) => {
            let mut x = rect.left() + pad;
            let icon_w = ig.size().x;
            let icon_h = ig.size().y;
            ui.painter()
                .galley(pos2(x, line_top + (lh - icon_h) / 2.0), ig, icon_ink);
            x += icon_w + gap;
            let max_name_w = (rect.right() - pad - x).max(8.0);
            crate::components::atoms::file_name::paint_body(
                ui,
                name,
                t.neutral_fg(),
                egui::Rect::from_min_size(pos2(x, line_top), vec2(max_name_w, lh)),
            );
        }
    }

    resp
}

/// Min width to show icon + labels (for icon-only collapse).
pub fn labeled_min_width(ui: &Ui, t: &Theme, labels: &[&str]) -> f32 {
    let font = TypeRole::Body.font_id();
    let icon_w = ui
        .painter()
        .layout_no_wrap(
            crate::components::phosphor::SEARCH.into(),
            phosphor_ui_font_id(),
            t.neutral_fg(),
        )
        .size()
        .x;
    let max_label = labels
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap((*l).into(), font.clone(), t.neutral_fg())
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    PAD_X.pts() * 2.0 + icon_w + ICON_GAP.pts() + max_label
}
