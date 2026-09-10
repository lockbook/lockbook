//! Document outline sidecar — slides out from a hairline on the content's
//! right edge. Not a sidebar: the content+outline cluster stays centered.

use egui::{Align, CursorIcon, Id, Layout, Rect, ScrollArea, Sense, Ui, pos2, vec2};

use crate::style::{
    Radius, STROKE_HAIRLINE, Space, Theme, TypeRole, control_height, interact_fill_response,
    paint_file_name, place_at, quiet_canvas_fills, sense_click, surface_motion,
};
use crate::tab::markdown_editor::fragment::OutlineItem;

/// Resting sidecar width (gap is extra, outside this).
pub const OUTLINE_W: f32 = 200.0;

pub fn gap() -> f32 {
    Space::Md.pts()
}

/// Width added to the content cluster at this slide (0 tucked .. 1 open).
pub fn sidecar_width(slide: f32) -> f32 {
    (gap() + OUTLINE_W) * slide
}

pub fn motion(ui: &Ui, file_id: lb_rs::Uuid, open: bool) -> crate::style::SurfaceMotion {
    surface_motion(ui.ctx(), Id::new("md_outline").with(file_id), open)
}

/// Paint the sidecar. `hairline_x` is the content column's right edge.
/// Returns a heading slug to jump to.
pub fn show(
    ui: &mut Ui, t: &Theme, items: &[OutlineItem], canvas: Rect, hairline_x: f32, slide: f32,
    scroll_id: Id,
) -> Option<String> {
    if slide <= 0.0 {
        return None;
    }
    let sidecar = sidecar_width(slide);
    let clip = Rect::from_min_max(
        pos2(hairline_x, canvas.top()),
        pos2(hairline_x + sidecar, canvas.bottom()),
    )
    .intersect(ui.clip_rect());
    if clip.width() < 0.5 {
        return None;
    }
    let panel = Rect::from_min_size(
        pos2(hairline_x + sidecar - OUTLINE_W, canvas.top()),
        vec2(OUTLINE_W, canvas.height()),
    );

    let mut jump = None;
    place_at(ui, panel, Layout::top_down(Align::Min), |ui| {
        ui.set_clip_rect(clip);
        ui.set_width(OUTLINE_W);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let pad = Space::Sm.pts();
        ui.add_space(pad);
        jump = ScrollArea::vertical()
            .id_salt(scroll_id.with("outline"))
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.set_width(OUTLINE_W);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                if items.is_empty() {
                    ui.label(
                        TypeRole::Body
                            .rich("No headings")
                            .color(t.neutral_fg_secondary()),
                    );
                    return None;
                }
                let mut hit = None;
                for (i, item) in items.iter().enumerate() {
                    if row(ui, t, item, scroll_id.with(i)).clicked() {
                        hit = Some(item.slug.clone());
                    }
                }
                hit
            })
            .inner;
    });

    if slide > 0.0 {
        ui.painter().vline(
            hairline_x,
            canvas.y_range(),
            egui::Stroke { width: STROKE_HAIRLINE, color: t.neutral() },
        );
    }
    jump
}

fn row(ui: &mut Ui, t: &Theme, item: &OutlineItem, id: Id) -> egui::Response {
    let h = control_height();
    let w = crate::style::ui_width(ui).max(1.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let resp = ui.interact(rect, id, sense_click());
    let fill = interact_fill_response(ui.ctx(), &resp, quiet_canvas_fills(t));
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);
    if resp.hovered() {
        ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
    }
    let indent = Space::Sm.pts() * (item.level.saturating_sub(1) as f32).min(5.0);
    let pad = Space::Sm.pts();
    let slot = Rect::from_min_max(
        pos2(rect.left() + pad + indent, rect.top()),
        pos2(rect.right() - pad, rect.bottom()),
    );
    let label = if item.text.is_empty() { "Untitled".into() } else { item.text.clone() };
    paint_file_name(ui, &label, t.neutral_fg(), slot);
    resp
}
