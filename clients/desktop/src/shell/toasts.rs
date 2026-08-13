//! Transient bottom-of-window toasts for errors and short status.
//!
//! ## Design
//! - **Product chrome**, not a design-system atom — one host on [`super::ShellApp`].
//! - **Non-focusable** (no sticky-field fights).
//! - **Auto-dismiss** after a short lifetime; capped stack so failures cannot pile up.
//! - **Workers** push via [`ToastInbox`] (`Arc`) and are drained each frame before paint.
//!
//! Prefer the sync footer for ongoing work. Use toasts for **completed failures**
//! (or rare one-shot info) the user would otherwise miss.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{Align2, Area, Frame, Id, Order, Sense, Stroke, vec2};

use crate::components::{Radius, STROKE_HAIRLINE, Space, Theme, TypeRole};

const LIFETIME: Duration = Duration::from_secs(4);
const MAX_VISIBLE: usize = 3;
const MAX_QUEUE: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Error,
    Info,
}

struct ToastItem {
    id: u64,
    kind: ToastKind,
    message: String,
    born: Instant,
}

/// Thread-safe queue for background workers (import, etc.).
#[derive(Clone, Default)]
pub struct ToastInbox {
    inner: Arc<Mutex<VecDeque<(ToastKind, String)>>>,
}

impl ToastInbox {
    pub fn error(&self, message: impl Into<String>) {
        self.push(ToastKind::Error, message.into());
    }

    fn push(&self, kind: ToastKind, message: String) {
        let Ok(mut g) = self.inner.lock() else {
            return;
        };
        if g.len() >= MAX_QUEUE {
            g.pop_front();
        }
        g.push_back((kind, message));
    }

    fn drain(&self) -> Vec<(ToastKind, String)> {
        let Ok(mut g) = self.inner.lock() else {
            return Vec::new();
        };
        g.drain(..).collect()
    }
}

/// On-screen toast stack + worker inbox.
pub struct ToastHost {
    next_id: u64,
    items: VecDeque<ToastItem>,
    /// Clone into workers; drain each frame in [`Self::show`].
    pub inbox: ToastInbox,
}

impl Default for ToastHost {
    fn default() -> Self {
        Self { next_id: 1, items: VecDeque::new(), inbox: ToastInbox::default() }
    }
}

impl ToastHost {
    pub fn error(&mut self, message: impl Into<String>) {
        self.push(ToastKind::Error, message.into());
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.push(ToastKind::Info, message.into());
    }

    fn push(&mut self, kind: ToastKind, message: String) {
        let message = message.trim();
        if message.is_empty() {
            return;
        }
        while self.items.len() >= MAX_VISIBLE {
            self.items.pop_front();
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.items.push_back(ToastItem {
            id,
            kind,
            message: message.to_owned(),
            born: Instant::now(),
        });
    }

    /// Drain worker inbox, expire old toasts, paint stack at bottom-center.
    pub fn show(&mut self, ctx: &egui::Context, t: &Theme) {
        for (kind, message) in self.inbox.drain() {
            self.push(kind, message);
        }

        let now = Instant::now();
        self.items
            .retain(|it| now.duration_since(it.born) < LIFETIME);
        if self.items.is_empty() {
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(200));

        let screen = ctx.screen_rect();
        let max_w = (screen.width() - Space::Xl.pts() * 2.0).clamp(200.0, 420.0);
        let pad = Space::Md.pts();
        let gap = Space::Sm.pts();
        let mut bottom_y = screen.bottom() - Space::Lg.pts();

        let items: Vec<_> = self.items.iter().collect();
        for it in items.iter().rev() {
            let age = now.duration_since(it.born);
            let fade_in = (age.as_secs_f32() / 0.12).min(1.0);
            let fade_out = {
                let left = LIFETIME.saturating_sub(age).as_secs_f32();
                (left / 0.35).clamp(0.0, 1.0)
            };
            let a = (fade_in * fade_out).clamp(0.0, 1.0);

            let text_color = match it.kind {
                ToastKind::Error => t.danger(),
                ToastKind::Info => t.neutral_fg(),
            };
            let border = match it.kind {
                ToastKind::Error => t.danger(),
                ToastKind::Info => t.neutral(),
            };
            let fill = t.neutral_bg_secondary();

            let galley = ctx.fonts(|f| {
                f.layout(
                    it.message.clone(),
                    TypeRole::Body.font_id(),
                    text_color,
                    max_w - pad * 2.0,
                )
            });
            let body_h = galley.size().y.max(TypeRole::Body.line_height());
            let plate_h = body_h + pad * 2.0;
            let plate_w = (galley.size().x + pad * 2.0).clamp(120.0, max_w);

            let id = Id::new(("shell_toast", it.id));
            Area::new(id)
                .order(Order::Foreground)
                .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -(screen.bottom() - bottom_y)))
                .interactable(false)
                .sense(Sense::hover())
                .show(ctx, |ui| {
                    ui.set_opacity(a);
                    Frame::new()
                        .fill(fill)
                        .stroke(Stroke::new(STROKE_HAIRLINE, border))
                        .corner_radius(Radius::Control.corner())
                        .inner_margin(pad)
                        .show(ui, |ui| {
                            ui.set_max_width(plate_w - pad * 2.0);
                            ui.label(TypeRole::Body.rich(&it.message).color(text_color));
                        });
                });

            bottom_y -= plate_h + gap;
        }
    }
}
