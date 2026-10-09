//! Overlay scrollbars + fixed-height sheet lists.
//!
//! Scroll fade/reveal lives in [`workspace_rs::style::overlay_scroll`]. This
//! module re-exports it and keeps the sheet list plate
//! ([`fixed_height_list`]).

use egui::{Id, Ui};

pub use workspace_rs::style::{SIDEBAR_RESIZING_LATCH, with_overlay_scroll};

/// Fixed-height list viewport with overlay scrollbar, **tight clip**, and the
/// same rounded hairline plate as sheet folder trees.
///
/// Use for sheet lists (share access, short pickers). Avoids stock
/// `ScrollArea` traps:
/// - fat bar that expands on hover → overlay via [`with_overlay_scroll`]
/// - `auto_shrink` collapsing the viewport → both min/max height locked
/// - `clip_rect_margin` bleed (clip_rect margin pitfall) → clip to the **allocated slot**,
///   not content `max_rect ∩` margin-expanded `clip_rect`
/// - blank tail looking like layout hole → Control-radius stroke plate (like
///   create/move folder choosers) so a fixed N-row band stays readable when
///   content is short
///
/// ## Height: measure, don't guess
///
/// `height` is the **inner** content viewport in points (row pitch × N). The
/// hairline frame adds stroke outside that (stroke budget — budget `total_margin`
/// when filling a residual band). Prefer:
///
/// ```ignore
/// let row_h = person_row_height(ui, true); // or measure one sample row
/// fixed_height_list(ui, t, id, 5.0 * row_h, |ui| { … });
/// ```
///
/// Same spirit as Create's plate lock (fixed plate height): natural metrics
/// first, then a fixed slot. Formulae like `control_height() + line_height`
/// drift from real galley stacks and read as a short list.
///
/// Parent must afford `height` (Outside stroke is free of layout). No flex residual (#28).
pub fn fixed_height_list<R>(
    ui: &mut Ui, t: &crate::components::Theme, id: Id, height: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    use egui::{Align, Layout, Rect, ScrollArea, Sense, UiBuilder, vec2};

    use crate::components::foundation::chrome::{Radius, paint_plate_stroke};

    let h = height.max(1.0);
    let w = crate::components::ui_width(ui).max(1.0);
    let radius = Radius::Control.corner();
    // Outside hairline after content so row washes cannot cover the border
    // (paint order — same idea as plate_content).
    ui.allocate_ui_with_layout(vec2(w, h), Layout::top_down(Align::Min), |ui| {
        ui.set_width(w);
        ui.set_height(h);
        ui.set_max_height(h);
        let (slot, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let inner = ui
            .scope_builder(UiBuilder::new().max_rect(slot), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                // Keep row washes inside the slot; ScrollArea margin must not expand clip.
                ui.set_clip_rect(slot.intersect(ui.clip_rect()));
                with_overlay_scroll(ui, id, |ui| {
                    ui.set_height(h);
                    let out = ScrollArea::vertical()
                        .id_salt(id)
                        .max_height(h)
                        .min_scrolled_height(h)
                        .auto_shrink([false, false])
                        .show_viewport(ui, |ui, viewport| {
                            // Content origin = max_rect().min (clip_rect margin pitfall).
                            let content_min = ui.max_rect().min;
                            let view_screen = Rect::from_min_size(
                                content_min + viewport.min.to_vec2(),
                                viewport.size(),
                            );
                            let tight = view_screen.intersect(slot);
                            ui.set_clip_rect(tight);

                            ui.set_min_width(crate::components::ui_width(ui));
                            ui.spacing_mut().item_spacing.y = 0.0;
                            add_contents(ui)
                        });
                    (out.inner, out.state.offset.y, out.id)
                })
            })
            .inner;
        // Stroke last — wins over flush row fills / rounded-corner wash bleed.
        paint_plate_stroke(ui, slot, radius, t.neutral());
        inner
    })
    .inner
}
