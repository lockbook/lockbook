//! Step 1 — Welcome (shared entry for Create account & Sign in).
//!
//! Centered monumental layout on `t.neutral_bg()`, positioned with explicit rect
//! geometry (no nested egui alignment layouts):
//! - 60px Lockbook mark, 32px display headline, balanced 2-line 15.5px subtitle
//!   with 24px leading, and side-by-side `Create account` / `Sign in` buttons.
//! - `⌘N` and `⌘I` shortcuts remain wired in `shell/mod.rs`.
//! - Centered bottom server indicator (rest: dot + host; click: inline `Field`).

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, Color32, FontFamily, FontId, Id, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2,
};

use crate::components::{
    Button, Field, Space, Theme, TypeRole, control_height, sense_click, shortcut_enter, shortcut_s,
};
use crate::shell::ShellApp;
use crate::shell::action::{Action as A, OnboardStep};
use crate::shell::apply_onboard::{
    display_server_host, onboard_server_editing_key, onboard_server_snap_key,
};

use super::{centered_column_with_height, column_width, logo_texture};

const COL_W: f32 = 520.0;
const LOGO_SIDE: f32 = 60.0;
const TITLE_SIZE: f32 = 32.0;
const SUBTITLE_SIZE: f32 = 15.5;
const SUBTITLE_LINE_H: f32 = 24.0;
const BTN_H: f32 = 38.0;
const BTN_GAP: f32 = 12.0;

const GAP_LOGO_TITLE: f32 = 24.0;
const GAP_TITLE_SUB: f32 = 14.0;
const GAP_SUB_BTNS: f32 = 30.0;

const TITLE: &str = "Notes that stay yours.";
const SUBTITLE_WIDE: &str = "Open-source Markdown notes and drawings,\nencrypted on your device and shared only by invitation.";
const SUBTITLE_NARROW: &str = "Open-source Markdown notes and drawings, encrypted on your device and shared only by invitation.";

fn title_font() -> FontId {
    FontId::new(TITLE_SIZE, FontFamily::Name(Arc::from("Bold")))
}

fn subtitle_galley(ui: &Ui, text: &str, max_w: f32, color: Color32) -> Arc<egui::Galley> {
    let mut job = LayoutJob { halign: Align::Center, ..Default::default() };
    job.wrap.max_width = max_w;
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::proportional(SUBTITLE_SIZE),
            color,
            line_height: Some(SUBTITLE_LINE_H),
            ..Default::default()
        },
    );
    ui.painter().layout_job(job)
}

pub(super) fn show(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<A>) {
    let full = ui.max_rect();
    let (busy, api_url) = match &app.onboard {
        Some(s) => (s.busy, s.api_url.clone()),
        None => return,
    };

    let w = column_width(ui, COL_W);
    let title_galley = ui
        .painter()
        .layout_no_wrap(TITLE.to_owned(), title_font(), t.neutral_fg());
    let title_h = title_galley.size().y;

    // Explicit balanced 2-line break on desktop widths; natural centered wrap on narrow windows.
    let sub_text = if w >= 440.0 { SUBTITLE_WIDE } else { SUBTITLE_NARROW };
    let sub_galley = subtitle_galley(ui, sub_text, w, t.neutral_fg_secondary());
    let sub_h = sub_galley.size().y;

    let create_btn = Button::primary(t, "Create account")
        .shortcut(shortcut_enter())
        .height(BTN_H)
        .enabled(!busy);
    let signin_btn = Button::secondary(t, "Sign in")
        .shortcut(shortcut_s())
        .height(BTN_H)
        .enabled(!busy);
    let create_w = create_btn.measure_width(ui);
    let signin_w = signin_btn.measure_width(ui);
    let btns_total_w = create_w + BTN_GAP + signin_w;

    let total_h =
        LOGO_SIDE + GAP_LOGO_TITLE + title_h + GAP_TITLE_SUB + sub_h + GAP_SUB_BTNS + BTN_H;

    centered_column_with_height(ui, COL_W, total_h, |ui| {
        let (area, _) = ui.allocate_exact_size(vec2(w, total_h), Sense::hover());
        let cx = area.center().x;
        let mut y = area.top();

        // 1. Centered logo mark
        if let Some(tex) = logo_texture(ui.ctx()) {
            let logo_rect =
                Rect::from_min_size(pos2(cx - LOGO_SIDE / 2.0, y), vec2(LOGO_SIDE, LOGO_SIDE));
            ui.painter().image(
                tex.id(),
                logo_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        y += LOGO_SIDE + GAP_LOGO_TITLE;

        // 2. Centered hero headline
        let title_pos = pos2(cx - title_galley.size().x / 2.0, y);
        ui.painter().galley(title_pos, title_galley, t.neutral_fg());
        y += title_h + GAP_TITLE_SUB;

        // 3. Centered 2-line subtitle with generous leading
        let sub_pos = pos2(cx, y);
        ui.painter()
            .galley(sub_pos, sub_galley, t.neutral_fg_secondary());
        y += sub_h + GAP_SUB_BTNS;

        // 4. Side-by-side buttons, explicitly centered horizontally
        let btn_left = cx - btns_total_w / 2.0;
        let create_rect = Rect::from_min_size(pos2(btn_left, y), vec2(create_w, BTN_H));
        let signin_rect =
            Rect::from_min_size(pos2(btn_left + create_w + BTN_GAP, y), vec2(signin_w, BTN_H));

        let create_clicked = ui
            .scope_builder(
                UiBuilder::new()
                    .max_rect(create_rect)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| create_btn.show(ui).clicked(),
            )
            .inner;
        if create_clicked {
            queue.push(A::OnboardSetStep(OnboardStep::Create));
        }

        let signin_clicked = ui
            .scope_builder(
                UiBuilder::new()
                    .max_rect(signin_rect)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| signin_btn.show(ui).clicked(),
            )
            .inner;
        if signin_clicked {
            queue.push(A::OnboardSetStep(OnboardStep::Import));
        }
    });

    // Centered bottom server selector (rest: dot + muted host caption, click: inline Field).
    server_footer(app, ui, t, full, &api_url);
}

fn onboard_server_edit_id() -> Id {
    Id::unique("onboard_server").with("edit")
}

fn onboard_server_need_focus_key() -> Id {
    Id::unique("onboard_server_need_focus")
}

fn server_footer(app: &mut ShellApp, ui: &mut Ui, t: &Theme, full: Rect, api_url: &str) {
    let margin = Space::Lg.pts();
    let h = control_height();
    let field_w = (full.width() - margin * 2.0).clamp(160.0, 280.0);
    let cy = full.bottom() - margin - h / 2.0;
    let cx = full.center().x;

    let editing_key = onboard_server_editing_key();
    let snap_key = onboard_server_snap_key();
    let mut editing = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(editing_key))
        .unwrap_or(false);
    let mut need_focus = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(onboard_server_need_focus_key()))
        .unwrap_or(false);

    if editing {
        let field_rect = Rect::from_center_size(pos2(cx, cy), vec2(field_w, h));
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(field_rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                let Some(state) = &mut app.onboard else {
                    return;
                };
                let _ = Field::new(t, &mut state.api_url)
                    .hint(lb::DEFAULT_API_LOCATION)
                    .width(field_w)
                    .id(Id::unique("onboard_server"))
                    .clearable(true)
                    .sticky(false)
                    .show(ui);
                if need_focus {
                    ui.memory_mut(|m| m.request_focus(onboard_server_edit_id()));
                    if ui.memory(|m| m.has_focus(onboard_server_edit_id())) {
                        need_focus = false;
                    }
                }
            },
        );
        let focused = ui.memory(|m| m.has_focus(onboard_server_edit_id()));
        if !need_focus && !focused {
            editing = false;
        }
    } else if server_caption(
        ui,
        t,
        pos2(cx, cy),
        h,
        &display_server_host(api_url),
        !api_url.trim().is_empty(),
    ) {
        editing = true;
        need_focus = true;
        ui.ctx()
            .data_mut(|d| d.insert_temp(snap_key, api_url.to_owned()));
    }

    ui.ctx().data_mut(|d| {
        d.insert_temp(editing_key, editing);
        d.insert_temp(onboard_server_need_focus_key(), need_focus);
    });
}

fn server_caption(
    ui: &mut Ui, t: &Theme, center: egui::Pos2, h: f32, host: &str, custom: bool,
) -> bool {
    let font = TypeRole::Caption.font_id();
    let text_w = ui
        .painter()
        .layout_no_wrap(host.to_owned(), font.clone(), t.neutral_fg_secondary())
        .size()
        .x;
    let dot_r = 3.0_f32;
    let dot_gap = 7.0_f32;
    let pad_x = 10.0_f32;
    let total_w = pad_x * 2.0 + dot_r * 2.0 + dot_gap + text_w;
    let rect = Rect::from_center_size(center, vec2(total_w, h));
    let resp = ui.interact(rect, Id::unique("onboard_server_caption"), sense_click());
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    let ink = if over { t.neutral_fg() } else { t.neutral_fg_secondary() };
    let dot_color = if custom { t.accent() } else { t.fg().green.gamma_multiply(0.85) };

    let dot_center = pos2(rect.left() + pad_x + dot_r, rect.center().y);
    ui.painter().circle_filled(dot_center, dot_r, dot_color);
    ui.painter().text(
        pos2(dot_center.x + dot_r + dot_gap, rect.center().y),
        Align2::LEFT_CENTER,
        host,
        font,
        ink,
    );
    resp.clicked()
}
