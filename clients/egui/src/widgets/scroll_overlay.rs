//! macOS-style overlay scrollbars for the sidebar: the bar stays dormant on
//! mere hover and only appears while the content is scrolling (and briefly
//! after). egui's default floating style reveals on area hover via
//! `active_handle_opacity` — we zero that and drive visibility from offset
//! changes instead.

use egui::{Id, Ui};

/// How long the bar stays fully “active” after the last scroll, in seconds.
const FADE_SECS: f64 = 0.85;

#[derive(Clone, Copy)]
struct State {
    last_offset_y: f32,
    /// `f64::NEG_INFINITY` until the first real scroll.
    last_active: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_offset_y: f32::NAN,
            last_active: f64::NEG_INFINITY,
        }
    }
}

/// Call *before* building a `ScrollArea` (inside a `ui.scope` so style is local).
/// Sets floating scrollbar opacities from whether this area scrolled recently.
pub fn prepare(ui: &mut Ui, id: Id) {
    let now = ui.input(|i| i.time);
    let st = ui.ctx().data(|d| d.get_temp::<State>(id)).unwrap_or_default();
    let show = now - st.last_active < FADE_SECS;

    // Keep animating opacity until the fade window ends.
    if show {
        ui.ctx().request_repaint();
    }

    let scroll = &mut ui.style_mut().spacing.scroll;
    scroll.floating = true;
    // Thin rail when visible; expand on bar hover only while shown.
    scroll.floating_width = 4.0;
    scroll.bar_width = 7.0;
    scroll.dormant_handle_opacity = 0.0;
    scroll.dormant_background_opacity = 0.0;

    if show {
        // Match egui floating defaults for “content is active”.
        scroll.active_handle_opacity = 0.55;
        scroll.active_background_opacity = 0.35;
        scroll.interact_handle_opacity = 0.9;
        scroll.interact_background_opacity = 0.65;
    } else {
        // Critical: active_* is what lerp uses when hovering the *area*.
        // Zero it so hover alone never reveals the bar (macOS).
        scroll.active_handle_opacity = 0.0;
        scroll.active_background_opacity = 0.0;
        scroll.interact_handle_opacity = 0.0;
        scroll.interact_background_opacity = 0.0;
        // No thin residual rail when dormant.
        scroll.floating_width = 0.0;
        scroll.bar_width = 0.0;
    }
}

/// Call each frame with the vertical scroll offset (e.g. `viewport.min.y`).
/// Marks the bar active when the offset moves.
pub fn note_offset(ui: &Ui, id: Id, offset_y: f32) {
    let now = ui.input(|i| i.time);
    ui.ctx().data_mut(|d| {
        let st = d.get_temp_mut_or_default::<State>(id);
        if st.last_offset_y.is_nan() {
            // First layout — don't flash the bar for the initial settle.
            st.last_offset_y = offset_y;
            return;
        }
        if (offset_y - st.last_offset_y).abs() > 0.25 {
            st.last_active = now;
            st.last_offset_y = offset_y;
        }
    });
}

/// Scope + prepare style, run `f`, then note the returned vertical offset.
///
/// `f` builds a `ScrollArea` (or equivalent) and returns `(result, offset_y)` —
/// typically `out.state.offset.y` from `ScrollArea::show` / `show_viewport`.
pub fn with_overlay_scroll<R>(
    ui: &mut Ui, id: Id, f: impl FnOnce(&mut Ui) -> (R, f32),
) -> R {
    ui.scope(|ui| {
        prepare(ui, id);
        let (result, offset_y) = f(ui);
        note_offset(ui, id, offset_y);
        result
    })
    .inner
}
