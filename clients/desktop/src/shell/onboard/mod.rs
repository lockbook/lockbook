//! Full-window onboarding experience on `t.neutral_bg()`.
//!
//! Create flow: Welcome → Choose a username → Choose a look (experiment) → Live workspace.
//! Sign in flow: Welcome → 3×8 phrase grid / compact key → Welcome back syncing → Live workspace.

mod create;
mod import;
pub(crate) mod phrase_grid;
mod welcome;

use egui::{
    Align, Id, Layout, Rect, TextStyle, TextWrapMode, TextureHandle, Ui, UiBuilder, WidgetText,
    pos2, vec2,
};

use crate::components::{Space, Theme};
use crate::shell::ShellApp;
use crate::shell::action::{Action as A, OnboardLookup, OnboardStep};
use crate::shell::apply_onboard::initial_api_url;

use phrase_grid::WORD_COUNT;

/// Hash a secret string for in-session auto-submit deduplication without storing the raw secret.
pub(crate) fn secret_hash(secret: &str) -> u64 {
    use std::hash::{Hash as _, Hasher as _};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    secret.hash(&mut h);
    h.finish()
}

/// Full-window onboarding state held on [`ShellApp::onboard`].
/// Intentionally does not implement `Debug` because it holds secret sign-in inputs.
pub struct OnboardState {
    pub step: OnboardStep,
    pub uname: String,
    pub uname_lookup: OnboardLookup,
    pub uname_lookup_for: String,
    pub uname_lookup_seq: u64,
    pub import_words: [String; WORD_COUNT],
    pub import_focus: usize,
    pub import_compact: bool,
    pub compact_key: String,
    pub compact_visible: bool,
    pub api_url: String,
    pub sync_username: String,
    pub sync_status: String,
    pub busy: bool,
    pub err: Option<String>,
    /// Hash of the last compact secret submitted via import (shared between grid paste & compact auto-submit).
    pub last_auto_submit_hash: Option<u64>,
}

impl OnboardState {
    pub fn new() -> Self {
        Self {
            step: OnboardStep::Welcome,
            uname: String::new(),
            uname_lookup: OnboardLookup::Idle,
            uname_lookup_for: String::new(),
            uname_lookup_seq: 0,
            import_words: Default::default(),
            import_focus: 0,
            import_compact: false,
            compact_key: String::new(),
            compact_visible: false,
            api_url: initial_api_url(),
            sync_username: String::new(),
            sync_status: String::new(),
            busy: false,
            err: None,
            last_auto_submit_hash: None,
        }
    }
}

impl Default for OnboardState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the active full-window onboarding step inside the central canvas.
pub fn show(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<A>) {
    let Some((step, import_compact)) = app.onboard.as_ref().map(|s| (s.step, s.import_compact))
    else {
        return;
    };
    if step == OnboardStep::Create {
        crate::shell::apply::onboard_poll_uname(app, ui.ctx());
    }
    // Give each onboarding view its own egui ID namespace so widget fill/hover
    // animations (e.g. a Primary button on Create vs a Quiet Back button on Import)
    // never collide and cross-fade across step transitions.
    ui.push_id(("onboard_step", step, import_compact), |ui| match step {
        OnboardStep::Welcome => welcome::show(app, ui, t, queue),
        OnboardStep::Create => create::show(app, ui, t, queue),
        OnboardStep::Import => import::show_import(app, ui, t, queue),
        OnboardStep::ImportSyncing => import::show_syncing(app, ui, t, queue),
    });
}

/// Effective column width clamped to the current viewport.
pub(super) fn column_width(ui: &Ui, col_w: f32) -> f32 {
    let avail = ui.max_rect();
    let pad_x = Space::Lg.pts();
    col_w.min((avail.width() - pad_x * 2.0).max(200.0))
}

/// Exact wrapped height of an `egui` label at `max_width` — uses the same
/// `WidgetText::into_galley` layout path as `ui.label(...)`.
pub(super) fn measure_text_h(ui: &Ui, text: impl Into<WidgetText>, max_width: f32) -> f32 {
    text.into()
        .into_galley(ui, Some(TextWrapMode::Wrap), max_width, TextStyle::Body)
        .size()
        .y
}

/// Center a column of width `col_w` horizontally and vertically within `ui.max_rect()`
/// using an ahead-of-time height measurement (`measure_h`) so Frame 1 renders in
/// its exact final position with zero immediate-mode jump.
pub(super) fn centered_column(
    ui: &mut Ui, col_w: f32, measure_h: impl FnOnce(&Ui, f32) -> f32, add: impl FnOnce(&mut Ui),
) {
    let w = column_width(ui, col_w);
    let content_h = measure_h(ui, w);
    centered_column_with_height(ui, col_w, content_h, add);
}

/// Like [`centered_column`], taking a pre-measured `content_h` directly.
pub(super) fn centered_column_with_height(
    ui: &mut Ui, col_w: f32, content_h: f32, add: impl FnOnce(&mut Ui),
) {
    let avail = ui.max_rect();
    let pad_top = (crate::shell::titlebar::HEADER_H + Space::Md.pts()).max(48.0);
    let pad_bot = Space::Xl.pts() + Space::Lg.pts();
    let w = column_width(ui, col_w);
    let left = avail.center().x - w / 2.0;

    let usable_h = (avail.height() - pad_top - pad_bot).max(120.0);
    let needs_scroll = content_h > usable_h;

    if needs_scroll {
        let scroll_rect = Rect::from_min_max(
            pos2(avail.left(), avail.top() + pad_top),
            pos2(avail.right(), avail.bottom() - Space::Md.pts()),
        );
        ui.scope_builder(UiBuilder::new().max_rect(scroll_rect), |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let top = ui.cursor().top();
                    let col =
                        Rect::from_min_size(pos2(left, top), vec2(w, content_h.max(usable_h)));
                    let mut child = ui.new_child(
                        UiBuilder::new()
                            .max_rect(col)
                            .layout(Layout::top_down(Align::Min)),
                    );
                    child.set_width(w);
                    child.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    add(&mut child);
                    let used_h = (child.min_rect().bottom() - top).max(content_h);
                    ui.allocate_space(vec2(avail.width(), used_h + Space::Lg.pts()));
                });
        });
    } else {
        // Optical center sits slightly above exact geometric midpoint.
        let top =
            (avail.top() + pad_top + (usable_h - content_h) * 0.46).max(avail.top() + pad_top);
        let col =
            Rect::from_min_max(pos2(left, top), pos2(left + w, avail.bottom() - Space::Md.pts()));
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(col)
                .layout(Layout::top_down(Align::Min)),
        );
        child.set_width(w);
        child.spacing_mut().item_spacing = vec2(0.0, 0.0);
        add(&mut child);
    }
}

/// Decode the Lockbook logo mark (`clients/desktop/lockbook.png`) once per context.
pub(super) fn logo_texture(ctx: &egui::Context) -> Option<TextureHandle> {
    #[derive(Clone)]
    struct LogoTex(TextureHandle);

    let key = Id::unique("onboard_lockbook_logo_tex");
    ctx.data(|d| d.get_temp::<LogoTex>(key))
        .map(|l| l.0)
        .or_else(|| {
            let png = include_bytes!("../../../lockbook.png");
            let img = image::load_from_memory(png).ok()?.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
            let handle =
                ctx.load_texture("onboard_lockbook_logo", color, egui::TextureOptions::LINEAR);
            ctx.data_mut(|d| d.insert_temp(key, LogoTex(handle.clone())));
            Some(handle)
        })
}

/// Paint the Lockbook logo mark (`clients/desktop/lockbook.png`), decoded once per context.
pub(super) fn paint_logo(ui: &mut Ui, side: f32) {
    if let Some(tex) = logo_texture(ui.ctx()) {
        ui.add(egui::Image::from_texture((tex.id(), vec2(side, side))));
    } else {
        ui.allocate_exact_size(vec2(side, side), egui::Sense::hover());
    }
}

