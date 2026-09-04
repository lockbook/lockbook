//! macOS-style overlay scrollbars for the sidebar.
//!
//! Worked example: `lb-desktop-2026` `widgets/scroll_overlay.rs`, plus platform
//! norms for hold-while-interacting:
//!
//! | Event | Bar |
//! |-------|-----|
//! | Content scroll | show |
//! | Hover empty list (no recent scroll) | stay hidden |
//! | Hover / drag the **thumb** | stay visible |
//! | Separator mid-drag | force hidden |
//! | Idle after leave | fade (~0.85s) |
//!
//! egui’s default floating style reveals on *area* hover via `active_handle_opacity`
//! — we zero that and drive visibility from offset + bar interaction instead.
//!
//! **Resize strip:** SidePanel registers its resize grab *after* the scroll
//! area, so it wins shared pixels. [`BAR_OUTER_MARGIN`] (= sidebar `RESIZE_GRAB`)
//! keeps the floating bar left of that strip.

use std::time::Duration;

use egui::{Context, Id, Ui};

/// How long the bar stays fully “active” after the last scroll / bar leave.
const FADE_SECS: f64 = 0.85;

/// Shell sets this while the **sidebar separator is mid-drag** (not hover).
pub const SIDEBAR_RESIZING_LATCH: &str = "lb_sidebar_resizing";

/// Thumb width when the overlay bar is visible (`floating_width` / `bar_width`).
///
/// Floating bars paint **on top of** content — they must not change content
/// width or row layout (egui: floating ⇒ `current_bar_use == 0`).
const OVERLAY_BAR_WIDTH: f32 = 6.0;

#[derive(Clone, Copy)]
struct State {
    last_offset_y: f32,
    /// `f64::NEG_INFINITY` until the first real scroll / bar hold.
    last_active: f64,
    /// Last frame's thumb hover/drag — `prepare` runs before the bar exists.
    thumb_held: bool,
}

impl Default for State {
    fn default() -> Self {
        Self { last_offset_y: f32::NAN, last_active: f64::NEG_INFINITY, thumb_held: false }
    }
}

fn separator_dragging(ui: &Ui) -> bool {
    ui.ctx().data(|d| {
        d.get_temp::<bool>(Id::new(SIDEBAR_RESIZING_LATCH))
            .unwrap_or(false)
    })
}

/// True when the pointer is hovering or dragging either scroll bar of a
/// `ScrollArea` (egui registers bars as `scroll_area_id.with(0|1)`).
///
/// `ScrollArea` state’s `scroll_bar_interaction` is private; this uses the same
/// interact ids egui uses for the thumbs.
pub fn bar_held(ctx: &Context, scroll_area_id: Id) -> bool {
    (0..2).any(|d| {
        let bar_id = scroll_area_id.with(d);
        ctx.is_being_dragged(bar_id)
            || ctx
                .read_response(bar_id)
                .is_some_and(|r| r.hovered() || r.dragged())
    })
}

/// Call *before* building a `ScrollArea` (inside a `ui.scope` so style is local).
pub fn prepare(ui: &mut Ui, id: Id) {
    let now = ui.input(|i| i.time);
    let st = ui
        .ctx()
        .data(|d| d.get_temp::<State>(id))
        .unwrap_or_default();
    let show = !separator_dragging(ui) && (st.thumb_held || now - st.last_active < FADE_SECS);

    if show && !st.thumb_held {
        let remaining = (FADE_SECS - (now - st.last_active)).max(0.0);
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(remaining.max(1.0 / 120.0)));
    }

    // egui paints the thumb with *widget* visuals: track hover → `inactive`,
    // thumb hover → `hovered`, drag → `active`. Defaults differ in corner
    // radius (2 vs 3 → rect vs capsule) and fills/strokes. For this scope only,
    // lock those so only scroll opacities change strength — not shape/hue.
    {
        let style = ui.style_mut();
        let ink = style.visuals.widgets.inactive.fg_stroke.color;
        let radius = style.visuals.widgets.inactive.corner_radius;
        for w in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
        ] {
            w.corner_radius = radius;
            w.expansion = 0.0;
            w.fg_stroke.color = ink;
            // `foreground_color` uses fg; also pin bg in case a theme path ignores it.
            w.bg_fill = ink;
            w.weak_bg_fill = ink;
        }
    }

    let scroll = &mut ui.style_mut().spacing.scroll;
    scroll.floating = true;
    // Same width idle + hover — avoids 4→7px expand reading as a shape change.
    scroll.floating_width = OVERLAY_BAR_WIDTH;
    scroll.bar_width = OVERLAY_BAR_WIDTH;
    // Floating: no layout gutter. Outer margin only affects where the thumb is
    // painted, not content size — keep 0 so the thumb sits on the content edge.
    scroll.bar_outer_margin = 0.0;
    scroll.dormant_handle_opacity = 0.0;
    scroll.dormant_background_opacity = 0.0;
    scroll.foreground_color = true;

    if show {
        // Opacity alone drives strength: idle-visible < bar-hover/drag.
        scroll.active_handle_opacity = 0.5;
        scroll.active_background_opacity = 0.2;
        scroll.interact_handle_opacity = 0.85;
        scroll.interact_background_opacity = 0.35;
    } else {
        // Zero active_* so area hover alone never reveals the bar.
        scroll.active_handle_opacity = 0.0;
        scroll.active_background_opacity = 0.0;
        scroll.interact_handle_opacity = 0.0;
        scroll.interact_background_opacity = 0.0;
    }
}

/// Call each frame with the vertical scroll offset (e.g. `out.state.offset.y`).
pub fn note_offset(ui: &Ui, id: Id, offset_y: f32) {
    let now = ui.input(|i| i.time);
    ui.ctx().data_mut(|d| {
        let st = d.get_temp_mut_or_default::<State>(id);
        if st.last_offset_y.is_nan() {
            st.last_offset_y = offset_y;
            return;
        }
        if (offset_y - st.last_offset_y).abs() > 0.25 {
            st.last_active = now;
            st.last_offset_y = offset_y;
        }
    });
}

/// Keep the bar in the “active” window while the thumb is hovered or dragged.
pub fn note_bar_interaction(ui: &Ui, id: Id, bar_held: bool) {
    let now = ui.input(|i| i.time);
    let left = ui.ctx().data_mut(|d| {
        let st = d.get_temp_mut_or_default::<State>(id);
        let was = st.thumb_held;
        st.thumb_held = bar_held;
        if bar_held || was {
            st.last_active = now;
        }
        was && !bar_held
    });
    if left {
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(FADE_SECS));
    }
}

/// Scope + prepare style, run `f`, then note offset and bar hold.
///
/// `f` builds a `ScrollArea` and returns `(result, offset_y, scroll_area_id)`:
/// - `offset_y` — typically `out.state.offset.y`
/// - `scroll_area_id` — `out.id` (for [`bar_held`])
pub fn with_overlay_scroll<R>(ui: &mut Ui, id: Id, f: impl FnOnce(&mut Ui) -> (R, f32, Id)) -> R {
    ui.scope(|ui| {
        prepare(ui, id);
        let (result, offset_y, scroll_area_id) = f(ui);
        note_offset(ui, id, offset_y);
        note_bar_interaction(ui, id, bar_held(ui.ctx(), scroll_area_id));
        result
    })
    .inner
}

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
