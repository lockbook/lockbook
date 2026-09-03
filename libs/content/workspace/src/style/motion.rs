//! Shared reveal/hide clock for split surfaces (landing recents, sidebar).
//!
//! Both directions start at full speed and ease to rest. Movement starts
//! immediately.

use egui::Id;

/// Reveal and hide share this duration.
pub const SURFACE_SECS: f32 = 0.72;

#[derive(Clone, Copy, Debug)]
pub struct SurfaceMotion {
    /// 0 tucked .. 1 at rest.
    pub slide: f32,
}

#[derive(Clone, Copy)]
struct Gesture {
    open: bool,
    from_raw: f32,
    from_slide: f32,
    last_slide: f32,
}

fn rest(open: bool) -> SurfaceMotion {
    SurfaceMotion { slide: if open { 1.0 } else { 0.0 } }
}

fn ease_out(t: f32) -> f32 {
    egui::emath::easing::exponential_out(t)
}

/// Linear openness toward `open`, then ease-out in that direction.
/// Reversing mid-flight keeps the current slide so it does not jump.
pub fn surface_motion(ctx: &egui::Context, id: Id, open: bool) -> SurfaceMotion {
    let raw = ctx
        .animate_bool_with_time_and_easing(id, open, SURFACE_SECS, egui::emath::easing::linear)
        .clamp(0.0, 1.0);
    let key = id.with("gesture");
    let prev = ctx.data(|d| d.get_temp::<Gesture>(key));

    let Some(prev) = prev else {
        let slide = if open { 1.0 } else { 0.0 };
        ctx.data_mut(|d| {
            d.insert_temp(
                key,
                Gesture { open, from_raw: raw, from_slide: slide, last_slide: slide },
            );
        });
        return rest(open);
    };

    let g = if prev.open != open {
        Gesture { open, from_raw: raw, from_slide: prev.last_slide, last_slide: prev.last_slide }
    } else {
        prev
    };

    let motion = if raw <= 0.0 {
        rest(false)
    } else if raw >= 1.0 {
        rest(true)
    } else {
        let target = if open { 1.0 } else { 0.0 };
        let span = (target - g.from_raw).abs().max(1e-5);
        let u = ((raw - g.from_raw).abs() / span).clamp(0.0, 1.0);
        let eased = ease_out(u);
        SurfaceMotion { slide: (g.from_slide * (1.0 - eased) + target * eased).clamp(0.0, 1.0) }
    };

    ctx.data_mut(|d| {
        d.insert_temp(
            key,
            Gesture {
                open: g.open,
                from_raw: g.from_raw,
                from_slide: g.from_slide,
                last_slide: motion.slide,
            },
        );
    });
    motion
}
