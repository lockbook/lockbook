//! The setup checklist card above the sync footer ("Getting started · 1 / 3" → "2 / 3"):
//! - Step 1: `Create account` (completed)
//! - Step 2: `Create or import a file` (active neutral while vault is empty; checks off once a file exists)
//! - Step 3: `Back up secret key` (quiet while vault is empty; accented once a file exists)
//!
//! No dismiss X on the card itself — the card retires once backup completes
//! inside the `Back up your secret key` sheet.

use std::sync::Arc;

use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Id, Pos2, Rect, Stroke, StrokeKind, Ui,
    pos2, vec2,
};
use workspace_rs::style::interact::{ControlFills, interact_fill_response};
use workspace_rs::style::layout::PadContent;

use crate::components::{Radius, STROKE_HAIRLINE, Theme, phosphor, phosphor_font_id, sense_click};
use crate::shell::action::Action as A;

const PAD: f32 = 11.0;
const HEADER_H: f32 = 16.0;
const HEADER_TO_BAR: f32 = 7.0;
const BAR_H: f32 = 3.0;
const BAR_TO_ROWS: f32 = 9.0;
const ROW_H: f32 = 26.0;
const ROW_GAP: f32 = 2.0;

pub struct NudgeCard<'a> {
    pub t: &'a Theme,
    pub has_files: bool,
    pub queue: &'a mut Vec<A>,
}

impl NudgeCard<'_> {
    const fn card_height() -> f32 {
        PAD + HEADER_H + HEADER_TO_BAR + BAR_H + BAR_TO_ROWS + ROW_H * 3.0 + ROW_GAP * 2.0 + PAD
    }
}

impl PadContent for NudgeCard<'_> {
    fn measure(&self, _ui: &Ui, _width: f32) -> f32 {
        Self::card_height()
    }

    fn place(&mut self, ui: &mut Ui, rect: Rect) {
        let t = self.t;
        let bg = t.neutral_bg();
        let accent = t.accent();
        let green = t.fg().green;

        ui.painter().rect(
            rect,
            Radius::Control.corner(),
            bg,
            Stroke::new(STROKE_HAIRLINE, t.neutral_bg_tertiary()),
            StrokeKind::Inside,
        );

        let x0 = rect.left() + PAD;
        let x1 = rect.right() - PAD;
        let y0 = rect.top() + PAD;

        // Header: GETTING STARTED · 1 / 3 (or 2 / 3 once a file exists)
        let header_cy = y0 + HEADER_H / 2.0;
        ui.painter().text(
            pos2(x0, header_cy),
            Align2::LEFT_CENTER,
            "GETTING STARTED",
            FontId::new(10.5, FontFamily::Name(Arc::from("Bold"))),
            t.neutral_fg_secondary(),
        );
        ui.painter().text(
            pos2(x1, header_cy),
            Align2::RIGHT_CENTER,
            if self.has_files { "2 / 3" } else { "1 / 3" },
            FontId::new(11.0, FontFamily::Proportional),
            t.neutral_fg_secondary(),
        );

        // Progress hair-bar: calm neutral at 1/3 so the main canvas leads; accented at 2/3.
        let bar_y = y0 + HEADER_H + HEADER_TO_BAR;
        let track = Rect::from_min_max(pos2(x0, bar_y), pos2(x1, bar_y + BAR_H));
        ui.painter()
            .rect_filled(track, CornerRadius::same(2), t.neutral_bg_tertiary());
        let (frac, bar_color) = if self.has_files {
            (2.0 / 3.0, accent)
        } else {
            (1.0 / 3.0, t.neutral_fg_secondary())
        };
        let filled = Rect::from_min_size(track.min, vec2(track.width() * frac, BAR_H));
        ui.painter()
            .rect_filled(filled, CornerRadius::same(2), bar_color);

        // Step 1: Create account (always completed)
        let r1_y = bar_y + BAR_H + BAR_TO_ROWS;
        paint_done_row(ui, t, bg, green, x0, x1, r1_y, "Create account");

        // Step 2: Create or import a file
        let r2_y = r1_y + ROW_H + ROW_GAP;
        if self.has_files {
            paint_done_row(ui, t, bg, green, x0, x1, r2_y, "Create or import a file");
        } else {
            let r2_rect = Rect::from_min_max(pos2(x0 - 4.0, r2_y), pos2(x1 + 4.0, r2_y + ROW_H));
            let resp = ui.interact(r2_rect, Id::unique("nudge_create_file_row"), sense_click());
            let fills = ControlFills {
                rest: t.neutral_bg_secondary(),
                hover: t.neutral_bg_tertiary(),
                press: t.neutral_bg_tertiary().lerp_to_gamma(t.neutral_fg(), 0.08),
            };
            let fill = interact_fill_response(ui.ctx(), &resp, fills);
            ui.painter().rect_filled(r2_rect, Radius::Sm.corner(), fill);

            let r2_cy = r2_rect.center().y;
            ui.painter().circle_stroke(
                pos2(x0 + 8.0, r2_cy),
                6.2,
                Stroke::new(1.5, t.neutral_fg()),
            );
            paint_clipped_label(
                ui,
                x0,
                x1,
                r2_rect,
                r2_cy,
                "Create or import a file",
                FontId::new(13.0, FontFamily::Name(Arc::from("Bold"))),
                t.neutral_fg(),
            );
            paint_icon_centered(
                ui.painter(),
                pos2(x1 - 4.0, r2_cy),
                phosphor::CARET_RIGHT,
                12.0,
                t.neutral_fg_secondary(),
            );
            if resp.clicked() {
                self.queue.push(A::Create);
            }
        }

        // Step 3: Back up secret key
        let r3_y = r2_y + ROW_H + ROW_GAP;
        let r3_rect = Rect::from_min_max(pos2(x0 - 4.0, r3_y), pos2(x1 + 4.0, r3_y + ROW_H));
        let resp = ui.interact(r3_rect, Id::unique("nudge_backup_key_row"), sense_click());
        let r3_cy = r3_rect.center().y;

        if self.has_files {
            let fills = ControlFills {
                rest: bg.lerp_to_gamma(accent, 0.10),
                hover: bg.lerp_to_gamma(accent, 0.18),
                press: bg.lerp_to_gamma(accent, 0.26),
            };
            let fill = interact_fill_response(ui.ctx(), &resp, fills);
            ui.painter().rect_filled(r3_rect, Radius::Sm.corner(), fill);
            ui.painter()
                .circle_stroke(pos2(x0 + 8.0, r3_cy), 6.5, Stroke::new(1.5, accent));
            paint_clipped_label(
                ui,
                x0,
                x1,
                r3_rect,
                r3_cy,
                "Back up secret key",
                FontId::new(13.0, FontFamily::Name(Arc::from("Bold"))),
                t.neutral_fg(),
            );
            paint_icon_centered(
                ui.painter(),
                pos2(x1 - 4.0, r3_cy),
                phosphor::CARET_RIGHT,
                12.0,
                accent,
            );
        } else {
            let fills = ControlFills {
                rest: Color32::TRANSPARENT,
                hover: t.neutral_bg_secondary(),
                press: t.neutral_bg_tertiary(),
            };
            let fill = interact_fill_response(ui.ctx(), &resp, fills);
            ui.painter().rect_filled(r3_rect, Radius::Sm.corner(), fill);
            ui.painter().circle_stroke(
                pos2(x0 + 8.0, r3_cy),
                6.2,
                Stroke::new(1.2, t.neutral_bg_tertiary()),
            );
            paint_clipped_label(
                ui,
                x0,
                x1,
                r3_rect,
                r3_cy,
                "Back up secret key",
                FontId::new(13.0, FontFamily::Proportional),
                t.neutral_fg_secondary(),
            );
        }

        if resp.clicked() {
            self.queue.push(A::NudgeAccept);
        }
    }
}

fn paint_done_row(
    ui: &Ui, t: &Theme, bg: Color32, green: Color32, x0: f32, x1: f32, y: f32, label: &str,
) {
    let row_rect = Rect::from_min_max(pos2(x0, y), pos2(x1, y + ROW_H));
    let cy = y + ROW_H / 2.0;
    ui.painter()
        .circle_filled(pos2(x0 + 8.0, cy), 6.5, bg.lerp_to_gamma(green, 0.20));
    paint_icon_centered(ui.painter(), pos2(x0 + 8.0, cy), phosphor::CHECK, 9.0, green);
    paint_clipped_label(
        ui,
        x0,
        x1,
        row_rect,
        cy,
        label,
        FontId::new(13.0, FontFamily::Proportional),
        t.neutral_fg_secondary(),
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_clipped_label(
    ui: &Ui, x0: f32, x1: f32, row_rect: Rect, cy: f32, text: &str, font: FontId, color: Color32,
) {
    let label_clip = Rect::from_min_max(
        pos2(x0 + 20.0, row_rect.top()),
        pos2((x1 - 14.0).max(x0 + 20.0), row_rect.bottom()),
    );
    ui.painter()
        .with_clip_rect(label_clip.intersect(ui.clip_rect()))
        .text(pos2(x0 + 22.0, cy), Align2::LEFT_CENTER, text, font, color);
}

fn paint_icon_centered(
    painter: &egui::Painter, center: Pos2, glyph: &str, size: f32, color: Color32,
) {
    let g = painter.layout_no_wrap(glyph.into(), phosphor_font_id(size), Color32::PLACEHOLDER);
    let ink_center = if g.mesh_bounds.width() > 0.0 && g.mesh_bounds.height() > 0.0 {
        g.mesh_bounds.center()
    } else {
        (g.size() / 2.0).to_pos2()
    };
    painter.galley(pos2(center.x - ink_center.x, center.y - ink_center.y), g, color);
}
