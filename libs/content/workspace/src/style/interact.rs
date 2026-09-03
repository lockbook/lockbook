//! Pointer-driven fill animation: a **from → to** color pair.
//!
//! ## Model
//! Each control tracks one in-flight segment. When the desired fill changes,
//! `from` becomes the **current** sample (often mid-flight) and `to` becomes
//! the new target. Press snaps in (`duration = 0`); other edges ease over
//! [`HOVER_ANIM_SECS`].
//!
//! ## Press = down **or** `clicked`
//! egui often batches pointer down+up in one frame (or clears down on the
//! release paint while setting `clicked`). Stock visuals treat
//! `down || focus || clicked` as active. We match that: Press while held, and
//! also on the click frame so same-frame / rapid clicks still snap to press.
//! Press-out then starts the **following** frame (one frame after up).
//!
//! ## Click without focus
//! egui's [`Sense::click`] includes `FOCUSABLE`. Prefer [`sense_click`] for
//! buttons / chips / segmented so pointer hits do not tab-stop and do not
//! fight sticky text-edit focus ([`super::field::Field`]).

use egui::{Color32, Context, Id, Response, Sense};

use super::chrome::HOVER_ANIM_SECS;
use super::color::{FG_HOVER, FG_PRESS, Theme};

/// Click + hover, **not** keyboard-focusable.
///
/// Use for non-text controls. Text edits keep focus unless another text input
/// (or explicit `request_focus`) takes it — see Field sticky restore.
#[inline]
pub fn sense_click() -> Sense {
    Sense::CLICK
}

/// Pointer-driven control state (drives which fill is desired).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IxState {
    Rest,
    Hover,
    Press,
}

/// Solid fills for each settled state (already token-resolved).
#[derive(Clone, Copy, Debug)]
pub struct ControlFills {
    pub rest: Color32,
    pub hover: Color32,
    pub press: Color32,
}

impl ControlFills {
    pub fn of(self, state: IxState) -> Color32 {
        match state {
            IxState::Rest => self.rest,
            IxState::Hover => self.hover,
            IxState::Press => self.press,
        }
    }
}

/// One from→to segment.
#[derive(Clone, Copy, Debug)]
struct IxRuntime {
    from: Color32,
    to: Color32,
    start: f64,
    duration: f32,
}

impl IxRuntime {
    fn settled(fill: Color32, now: f64) -> Self {
        Self { from: fill, to: fill, start: now, duration: 0.0 }
    }

    fn sample(self, now: f64) -> Color32 {
        if self.duration <= 0.0 {
            return self.to;
        }
        let t = ((now - self.start) / f64::from(self.duration)).clamp(0.0, 1.0) as f32;
        let e = egui::emath::easing::cubic_out(t);
        self.from.lerp_to_gamma(self.to, e)
    }

    fn done(self, now: f64) -> bool {
        self.duration <= 0.0 || now >= self.start + f64::from(self.duration)
    }
}

/// Map pointer flags → state. `clicked` counts as Press (same-frame down+up).
pub fn desired_state(pointer_over: bool, pointer_down: bool, clicked: bool) -> IxState {
    if pointer_down || clicked {
        IxState::Press
    } else if pointer_over {
        IxState::Hover
    } else {
        IxState::Rest
    }
}

/// Duration for a retarget toward `desired`. Press-in is instant.
fn retarget_duration(desired: IxState) -> f32 {
    match desired {
        IxState::Press => 0.0,
        IxState::Hover | IxState::Rest => HOVER_ANIM_SECS,
    }
}

/// If `target` differs from the segment’s `to`, start a new segment:
/// `from = current sample`, `to = target`.
fn retarget(rt: &mut IxRuntime, now: f64, target: Color32, desired: IxState) {
    if target == rt.to {
        return;
    }
    let from = rt.sample(now);
    *rt = IxRuntime { from, to: target, start: now, duration: retarget_duration(desired) };
}

/// Advance the machine for this control id; return the fill to paint this frame.
pub fn interact_fill(
    ctx: &Context, id: Id, pointer_over: bool, pointer_down: bool, clicked: bool,
    fills: ControlFills,
) -> Color32 {
    let now = ctx.input(|i| i.time);
    let desired = desired_state(pointer_over, pointer_down, clicked);
    let target = fills.of(desired);
    let key = id.with("design_ix");

    let (fill, animating) = ctx.data_mut(|d| {
        let rt = d.get_temp_mut_or_insert_with(key, || IxRuntime::settled(fills.rest, now));
        retarget(rt, now, target, desired);
        let fill = rt.sample(now);
        let animating = !rt.done(now);
        (fill, animating)
    });

    if animating {
        ctx.request_repaint();
    }

    fill
}

/// Convenience: read over / down / clicked from a [`Response`].
pub fn interact_fill_response(ctx: &Context, response: &Response, fills: ControlFills) -> Color32 {
    interact_fill(
        ctx,
        response.id,
        response.hovered(),
        response.is_pointer_button_down_on(),
        response.clicked(),
        fills,
    )
}

// ── Quiet control fills (shared by buttons, picker, …) ──────────────────────

/// Quiet control on **canvas** ground — rest is canvas, washes toward fg.
pub fn quiet_canvas_fills(t: &Theme) -> ControlFills {
    ControlFills {
        rest: t.neutral_bg(),
        hover: t.wash_toward_neutral_fg(t.neutral_bg(), FG_HOVER),
        press: t.wash_toward_neutral_fg(t.neutral_bg(), FG_PRESS),
    }
}

/// Settled selection wash on canvas (file row / menu selected item).
pub fn canvas_selected_fills(t: &Theme) -> ControlFills {
    let sel = t.wash_toward_neutral_fg(t.neutral_bg(), FG_PRESS);
    ControlFills {
        rest: sel,
        hover: sel,
        press: t.wash_toward_neutral_fg(t.neutral_bg(), FG_PRESS + 0.02),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(r: u8, g: u8, b: u8) -> Color32 {
        Color32::from_rgb(r, g, b)
    }

    #[test]
    fn desired_priority() {
        assert_eq!(desired_state(true, true, false), IxState::Press);
        assert_eq!(desired_state(true, false, false), IxState::Hover);
        assert_eq!(desired_state(false, false, false), IxState::Rest);
        assert_eq!(desired_state(false, true, false), IxState::Press);
        // Release / same-frame click: down cleared, clicked set — still Press.
        assert_eq!(desired_state(true, false, true), IxState::Press);
        assert_eq!(desired_state(false, false, true), IxState::Press);
    }

    #[test]
    fn retarget_uses_current_as_from() {
        let rest = rgb(255, 255, 255);
        let hover = rgb(200, 200, 200);
        let press = rgb(100, 100, 100);

        let mut rt = IxRuntime::settled(rest, 0.0);
        retarget(&mut rt, 0.0, hover, IxState::Hover);
        assert_eq!(rt.from, rest);
        assert_eq!(rt.to, hover);
        assert!(rt.duration > 0.0);

        let mid = rt.sample(0.1);
        retarget(&mut rt, 0.1, press, IxState::Press);
        assert_eq!(rt.from, mid);
        assert_eq!(rt.to, press);
        assert_eq!(rt.duration, 0.0);
        assert_eq!(rt.sample(0.1), press);
    }

    #[test]
    fn same_target_does_not_restart() {
        let hover = rgb(200, 200, 200);
        let mut rt = IxRuntime {
            from: rgb(255, 255, 255),
            to: hover,
            start: 0.0,
            duration: HOVER_ANIM_SECS,
        };
        let before = rt;
        retarget(&mut rt, 0.05, hover, IxState::Hover);
        assert_eq!(rt.from, before.from);
        assert_eq!(rt.to, before.to);
        assert_eq!(rt.start, before.start);
        assert_eq!(rt.duration, before.duration);
    }

    #[test]
    fn rapid_press_release_always_from_current() {
        let hover = rgb(200, 200, 200);
        let press = rgb(100, 100, 100);
        let mut rt = IxRuntime::settled(hover, 0.0);

        retarget(&mut rt, 0.0, press, IxState::Press);
        assert_eq!(rt.sample(0.0), press);

        retarget(&mut rt, 0.01, hover, IxState::Hover);
        assert_eq!(rt.from, press);
        assert_eq!(rt.to, hover);

        let mid = rt.sample(0.05);
        retarget(&mut rt, 0.05, press, IxState::Press);
        assert_eq!(rt.from, mid);
        assert_eq!(rt.sample(0.05), press);

        retarget(&mut rt, 0.06, hover, IxState::Hover);
        assert_eq!(rt.from, press);
        assert_eq!(rt.to, hover);
    }

    /// Same-frame down+up often only surfaces as `clicked` while mid Press→Hover.
    #[test]
    fn click_frame_mid_ease_out_snaps_to_press() {
        let hover = rgb(200, 200, 200);
        let press = rgb(100, 100, 100);
        let mut rt = IxRuntime::settled(press, 0.0);
        retarget(&mut rt, 0.0, hover, IxState::Hover);
        let mid = rt.sample(0.08);

        let desired = desired_state(true, false, true);
        assert_eq!(desired, IxState::Press);
        retarget(&mut rt, 0.08, press, desired);
        assert_eq!(rt.from, mid);
        assert_eq!(rt.to, press);
        assert_eq!(rt.sample(0.08), press);
    }
}
