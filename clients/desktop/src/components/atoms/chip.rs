//! Token chip: soft hue wash fill, hue border, neutral text.
//! Optional dismiss.

use egui::{Color32, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use workspace_rs::theme::palette_v2::Palette;

use crate::components::foundation::chrome::{Radius, STROKE_HAIRLINE};
use crate::components::foundation::color::{Theme, hue_wash, QUIET_PLATE_PRESS, CHIP_DISMISS_PRESS};
use crate::components::foundation::interact::{ControlFills, interact_fill, sense_click};
use crate::components::foundation::layout::{inset, paint_control_pads};
use crate::components::foundation::space::Space;
use crate::components::foundation::space::control as control_space;
use crate::components::foundation::typography::TypeRole;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChipHue {
    #[default]
    Neutral,
    Red,
    Yellow,
}

impl ChipHue {
    fn palette(self) -> Option<Palette> {
        match self {
            Self::Neutral => None,
            Self::Red => Some(Palette::Red),
            Self::Yellow => Some(Palette::Yellow),
        }
    }

    /// Soft fill + border ink.
    pub fn colors(self, t: &Theme) -> (Color32, Color32) {
        match self.palette() {
            // Surface plate (raised vs canvas in both modes); mid border, not full fg.
            None => (t.neutral_bg_secondary(), t.neutral_fg_secondary()),
            Some(p) => (hue_wash(t, p), t.fg().get_color(p)),
        }
    }
}

pub struct ChipOut {
    pub dismissed: bool,
}

pub struct Chip<'a> {
    tokens: &'a Theme,
    label: String,
    hue: ChipHue,
    dismissible: bool,
}

impl<'a> Chip<'a> {
    pub fn new(tokens: &'a Theme, label: impl Into<String>) -> Self {
        Self { tokens, label: label.into(), hue: ChipHue::Neutral, dismissible: false }
    }

    pub fn hue(mut self, hue: ChipHue) -> Self {
        self.hue = hue;
        self
    }

    pub fn dismissible(mut self) -> Self {
        self.dismissible = true;
        self
    }

    pub fn show(self, ui: &mut Ui) -> ChipOut {
        let t = self.tokens;
        let (fill, border) = self.hue.colors(t);
        let text = t.neutral_fg();

        let font = TypeRole::Mono.font_id();
        let pad_x = Space::Xs;
        let pad_y = Space::Xxs;
        let gap = control_space::ICON_GAP;

        let label_g = ui
            .painter()
            .layout_no_wrap(self.label.clone(), font, Color32::PLACEHOLDER);
        let label_w = label_g.size().x;
        // Trailing dismiss: glyph is small; hit target is a square ~ content
        // height (field clear energy) and includes the trailing pad.
        let dismiss_g = if self.dismissible {
            Some(ui.painter().layout_no_wrap(
                "×".into(),
                TypeRole::Mono.font_id(),
                Color32::PLACEHOLDER,
            ))
        } else {
            None
        };

        // Fixed line box so descenders don't change chip height or baseline.
        let content_h = TypeRole::Mono.line_height();
        // Square-ish dismiss hit (line box, at least body size).
        let dismiss_hit_w =
            if self.dismissible { content_h.max(TypeRole::Body.size()) } else { 0.0 };

        let mut content_w = label_w;
        if self.dismissible {
            content_w += gap.pts() + dismiss_hit_w;
        }

        let desired = vec2(content_w + pad_x.pts() * 2.0, content_h + pad_y.pts() * 2.0);

        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
        let radius = Radius::Sm.corner();
        let body_fill = fill;

        ui.painter().rect_filled(rect, radius, body_fill);
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(STROKE_HAIRLINE, border),
            StrokeKind::Inside,
        );

        // Measure → place: pad bands + content along mid.y (no LTR placer).
        paint_control_pads(ui, rect, pad_x, pad_y);
        let mid = inset(rect, pad_x.pts(), pad_y.pts());
        let mid_h = mid.height().max(content_h);
        let mut x = mid.left();
        let mut dismissed = false;

        let lr = egui::Rect::from_min_size(pos2(x, mid.top()), vec2(label_w, mid_h));
        paint_galley_in_line(ui.painter(), lr, label_g, text);
        x += label_w;

        if let Some(xg) = dismiss_g {
            x += gap.pts();
            // Hit = square + trailing pad (edge of chip is part of dismiss).
            let hit_w = dismiss_hit_w + pad_x.pts();
            let ir = egui::Rect::from_min_size(pos2(x, mid.top()), vec2(hit_w, mid_h));
            let dresp = ui.interact(ir, response.id.with("chip_dismiss_hit"), sense_click());
            let over = ui.ctx().rect_contains_pointer(ui.layer_id(), ir);
            let d_fills = ControlFills {
                rest: body_fill,
                hover: body_fill.lerp_to_gamma(border, QUIET_PLATE_PRESS),
                press: body_fill.lerp_to_gamma(border, CHIP_DISMISS_PRESS),
            };
            let d_fill = interact_fill(
                ui.ctx(),
                response.id.with("chip_dismiss"),
                over,
                dresp.is_pointer_button_down_on(),
                dresp.clicked(),
                d_fills,
            );
            // Wash only the square around the ×, not the full pad stretch.
            let wash = egui::Rect::from_min_size(
                pos2(ir.left(), ir.top()),
                vec2(dismiss_hit_w, ir.height()),
            );
            if d_fill != body_fill {
                ui.painter().rect_filled(wash, Radius::Sm.corner(), d_fill);
            }
            let x_pos =
                pos2(wash.center().x - xg.size().x / 2.0, wash.center().y - xg.size().y / 2.0);
            ui.painter().galley(x_pos, xg, text);
            if dresp.clicked() {
                dismissed = true;
            }
        }

        ChipOut { dismissed }
    }
}

/// Layout-box vertical align (mesh center shifts with descenders).
fn paint_galley_in_line(
    painter: &egui::Painter, slot: egui::Rect, galley: std::sync::Arc<egui::Galley>, color: Color32,
) {
    let pos = pos2(slot.left(), slot.center().y - galley.size().y / 2.0);
    painter.galley(pos, galley, color);
}
