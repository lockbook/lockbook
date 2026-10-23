//! Blinking caret.

use egui::{Color32, Id, Rangef, Rect, Ui, pos2};

use crate::workspace::Workspace;

const WIDTH: f32 = 2.0;

// Mac insertion-point: 500ms opaque, 150ms fade out, 200ms off, 150ms fade in.
const HOLD: f32 = 0.50;
const FADE_OUT: f32 = 0.15;
const OFF: f32 = 0.20;
const FADE_IN: f32 = 0.15;
const PERIOD: f32 = HOLD + FADE_OUT + OFF + FADE_IN;
// 50ms during ramps → 4 samples over 150ms.
const FADE_STEP: f32 = 0.05;

pub fn paint_caret(ui: &Ui, x: f32, y: Rangef, color: Color32) {
    let rect = Rect::from_min_max(pos2(x - WIDTH * 0.5, y.min), pos2(x + WIDTH * 0.5, y.max));
    ui.painter().rect_filled(rect, WIDTH * 0.5, color);
}

pub fn with_blinking_caret(ui: &Ui, time_since_interact: f64, paint: impl FnOnce(f32)) {
    // Like AppKit: no caret in a background window, and a fresh blink on return.
    let key = Id::unique("caret_window_refocused_at");
    let (window_focused, now) = ui.input(|i| (i.focused, i.time));
    if !window_focused {
        ui.data_mut(|d| d.insert_temp::<Option<f64>>(key, None));
        return;
    }
    let refocused_at = ui.data_mut(|d| {
        *d.get_temp_mut_or_insert_with(key, || Some(f64::NEG_INFINITY))
            .get_or_insert(now)
    });
    let time_since_interact = time_since_interact.min(now - refocused_at);

    if !Workspace::user_present(ui.ctx()) {
        paint(1.0);
        return;
    }

    let t = (time_since_interact as f32).rem_euclid(PERIOD);
    let (alpha, fade) = if t < HOLD {
        (1.0, false)
    } else if t < HOLD + FADE_OUT {
        (1.0 - (t - HOLD) / FADE_OUT, true)
    } else if t < HOLD + FADE_OUT + OFF {
        (0.0, false)
    } else {
        ((t - HOLD - FADE_OUT - OFF) / FADE_IN, true)
    };

    if alpha > 0.0 {
        paint(alpha);
    }

    let delay = if fade {
        let remaining = if t < HOLD + FADE_OUT { HOLD + FADE_OUT - t } else { PERIOD - t };
        remaining.min(FADE_STEP)
    } else if t < HOLD {
        HOLD - t
    } else {
        HOLD + FADE_OUT + OFF - t
    };
    ui.ctx().request_repaint_after_secs(delay);
}
