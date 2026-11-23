//! Unified `Back up your secret key` modal sheet (`Modal::BackupKey`).
//!
//! - 500px centered sheet (`plate_content_shadow` + `sheet_dim`).
//! - `×` and `Later (esc)` close without retiring the sidebar nudge.
//! - Plain-language 2-rule inset plate from `docs/account.md`.
//! - Segmented tabs: `QR code` (default) | `24 words`.
//! - Click-to-reveal privacy gate on BOTH tabs (synthetic placeholder QR before
//!   click; masked 3×8 grid with direct `Copy` without revealing on phrase tab).
//! - Clicking a revealed QR or revealed phrase masks it again while keeping `Done` unlocked.

use egui::{
    Align, Align2, Area, Color32, Id, Layout, Order, Rect, Stroke, StrokeKind, Ui, UiBuilder, pos2,
    vec2,
};
use workspace_rs::widgets::GlyphonLabel;

use crate::components::{
    Button, Radius, STROKE_HAIRLINE, Space, Spacer, Theme, TypeRole, control_height, phosphor,
    plate_content_shadow, segmented, sense_click, sheet_band_centered, sheet_dim, sheet_panel_fit,
    sheet_title, shortcut_enter, shortcut_esc, ui_width, with_pad_fit,
};

use super::ShellApp;
use super::action::{Action as A, BackupKeyTab, Modal};
use super::onboard::phrase_grid::show_display_grid;
use super::settings_account::paint_account_qr_image;

const SHEET_W: f32 = 500.0;
const QR_DISPLAY_SIDE: f32 = 196.0;

pub fn show_backup_key(app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<A>) {
    let state = match &app.modal {
        Some(Modal::BackupKey(s)) => *s,
        _ => return,
    };

    let layer = egui::LayerId::new(Order::Foreground, Id::unique("shell_backup_key"));
    if sheet_dim(ctx, Id::unique("shell_backup_key_dim"), layer) {
        queue.push(A::CloseModal);
    }

    // Export the phrase only as a local temporary while actually revealed on screen.
    let phrase_owned = if state.tab == BackupKeyTab::Phrase && state.phrase_revealed {
        app.session
            .ready()
            .and_then(|r| r.workspace.core.export_account_phrase().ok())
            .unwrap_or_default()
    } else {
        String::new()
    };
    let secret_words: Vec<&str> = phrase_owned.split_whitespace().collect();

    Area::new(Id::unique("shell_backup_key"))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            sheet_panel_fit(ui, t, SHEET_W, |ui| {
                if sheet_title(ui, t, "Back up your secret key") {
                    queue.push(A::CloseModal);
                }
                ui.add(Spacer::new(Space::Md));

                show_rules_plate(ui, t);
                ui.add(Spacer::new(Space::Md));

                let mut seg = match state.tab {
                    BackupKeyTab::Qr => 0usize,
                    BackupKeyTab::Phrase => 1usize,
                };
                sheet_band_centered(ui, control_height(), |ui| {
                    if segmented(ui, t, &["QR code", "24 words"], &mut seg).changed() {
                        let next = if seg == 0 { BackupKeyTab::Qr } else { BackupKeyTab::Phrase };
                        queue.push(A::BackupKeySetTab(next));
                    }
                });
                ui.add(Spacer::new(Space::Md));

                match state.tab {
                    BackupKeyTab::Qr => {
                        show_qr_tab(app, ui, t, state.qr_revealed, state.can_complete(), queue)
                    }
                    BackupKeyTab::Phrase => show_phrase_tab(
                        ui,
                        t,
                        &secret_words,
                        state.phrase_revealed,
                        state.can_complete(),
                        queue,
                    ),
                }
            });
        });
}

fn show_rules_plate(ui: &mut Ui, t: &Theme) {
    let w = ui_width(ui).max(1.0);
    plate_content_shadow(
        ui,
        t.neutral_bg_secondary(),
        egui::Shadow::NONE,
        Radius::Control.corner(),
        |ui| {
            ui.set_width(w);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            with_pad_fit(ui, Space::Md, |ui| {
                let max_w = ui_width(ui).max(1.0);
                let fs = TypeRole::Body.size();
                let lh = TypeRole::Body.line_height();
                ui.add(
                    GlyphonLabel::new_rich(
                        vec![
                            ("1. Keep it secret. ", true),
                            (
                                "Anyone with your key can read, edit, or delete your files. The Lockbook team will never ask for it.",
                                false,
                            ),
                        ],
                        t.neutral_fg(),
                    )
                    .font_size(fs)
                    .line_height(lh)
                    .max_width(max_w),
                );
                ui.add(Spacer::new(Space::Sm));
                ui.add(
                    GlyphonLabel::new_rich(
                        vec![
                            ("2. Don’t lose it. ", true),
                            (
                                "We don’t have your key and can’t recover your files without it. The easy way is to sign in on another device.",
                                false,
                            ),
                        ],
                        t.neutral_fg(),
                    )
                    .font_size(fs)
                    .line_height(lh)
                    .max_width(max_w),
                );
            });
        },
    );
}

fn show_qr_tab(
    app: &ShellApp, ui: &mut Ui, t: &Theme, revealed: bool, can_done: bool, queue: &mut Vec<A>,
) {
    let box_side = QR_DISPLAY_SIDE + Space::Md.pts() * 2.0;
    let caption_h = TypeRole::Body.line_height();
    let qr_content_h = box_side + Space::Lg.pts() + caption_h;
    let extra_h = (super::onboard::phrase_grid::grid_height() - qr_content_h).max(0.0);
    let pad_top = (extra_h / 2.0).floor();
    let pad_bot = extra_h - pad_top;

    if pad_top > 0.0 {
        ui.allocate_space(vec2(0.0, pad_top));
    }

    sheet_band_centered(ui, box_side, |ui| {
        if !revealed {
            if paint_qr_placeholder(ui, t, box_side) {
                queue.push(A::BackupKeyRevealQr);
            }
        } else {
            let cache = app
                .account_qr
                .lock()
                .ok()
                .map(|g| g.clone())
                .unwrap_or_default();
            match &cache {
                super::AccountQrCache::Idle | super::AccountQrCache::Loading => {
                    let (rect, resp) =
                        ui.allocate_exact_size(vec2(box_side, box_side), sense_click());
                    ui.painter().rect(
                        rect,
                        Radius::Control.corner(),
                        t.neutral_bg_secondary(),
                        Stroke::new(STROKE_HAIRLINE, t.neutral()),
                        StrokeKind::Inside,
                    );
                    let mut child = ui.new_child(
                        UiBuilder::new()
                            .max_rect(rect)
                            .layout(Layout::centered_and_justified(egui::Direction::TopDown)),
                    );
                    child.spinner();
                    if resp.clicked() {
                        queue.push(A::BackupKeyHideQr);
                    }
                }
                super::AccountQrCache::Ready(png) => {
                    let (rect, resp) =
                        ui.allocate_exact_size(vec2(box_side, box_side), sense_click());
                    // Crisp white quiet zone around the QR so phone cameras scan cleanly in dark mode.
                    ui.painter()
                        .rect_filled(rect, Radius::Control.corner(), Color32::WHITE);
                    let inner = rect.shrink(Space::Md.pts());
                    let mut child = ui.new_child(
                        UiBuilder::new()
                            .max_rect(inner)
                            .layout(Layout::centered_and_justified(egui::Direction::TopDown)),
                    );
                    paint_account_qr_image(&mut child, png, QR_DISPLAY_SIDE);
                    if resp.clicked() {
                        queue.push(A::BackupKeyHideQr);
                    }
                }
                super::AccountQrCache::Err(e) => {
                    ui.label(TypeRole::Body.rich(e).color(t.danger()));
                }
            }
        }
    });

    ui.add(Spacer::new(Space::Lg));
    sheet_band_centered(ui, caption_h, |ui| {
        ui.label(
            TypeRole::Body
                .rich("Scan this QR with Lockbook to sign in on your phone.")
                .color(t.neutral_fg_secondary()),
        );
    });

    if pad_bot > 0.0 {
        ui.allocate_space(vec2(0.0, pad_bot));
    }

    ui.add(Spacer::new(Space::Lg));
    show_backup_footer(
        ui,
        t,
        "Copy compact key",
        Id::unique("shell_backup_key_copy_compact"),
        A::BackupKeyCopyCompactKey,
        can_done,
        queue,
    );
}

/// Synthetic placeholder QR pattern — NEVER renders or blurs actual secret QR pixels.
fn paint_qr_placeholder(ui: &mut Ui, t: &Theme, side: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), sense_click());
    ui.painter().rect(
        rect,
        Radius::Control.corner(),
        t.neutral_bg_secondary(),
        Stroke::new(STROKE_HAIRLINE, t.neutral()),
        StrokeKind::Inside,
    );

    // Draw a subtle decorative 11×11 pseudo-matrix with 3 corner finder squares.
    let grid_rect = rect.shrink(22.0);
    let n = 11usize;
    let step = grid_rect.width() / n as f32;
    let dot_ink = t.neutral_fg_secondary().gamma_multiply(0.18);
    let is_finder = |r: usize, c: usize| -> bool {
        (r < 3 && c < 3) || (r < 3 && c >= n - 3) || (r >= n - 3 && c < 3)
    };
    for r in 0..n {
        for c in 0..n {
            let filled = if is_finder(r, c) {
                let lr = if r < 3 { r } else { r - (n - 3) };
                let lc = if c < 3 { c } else { c - (n - 3) };
                !(lr == 1 && lc == 1)
            } else {
                // Deterministic decorative pattern (zero relation to account key).
                ((r * 7 + c * 13 + r * c) % 3) == 0
            };
            if filled {
                let cell = Rect::from_min_size(
                    pos2(
                        grid_rect.left() + c as f32 * step + 1.5,
                        grid_rect.top() + r as f32 * step + 1.5,
                    ),
                    vec2((step - 3.0).max(2.0), (step - 3.0).max(2.0)),
                );
                ui.painter().rect_filled(cell, 2.0, dot_ink);
            }
        }
    }

    // Centered reveal pill button.
    let btn_h = control_height();
    let btn_rect = Rect::from_center_size(rect.center(), vec2(176.0, btn_h));
    let mut clicked = resp.clicked();
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(btn_rect)
            .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    if Button::primary(t, "Click to reveal QR")
        .icon(phosphor::EYE)
        .height(btn_h)
        .show(&mut child)
        .clicked()
    {
        clicked = true;
    }
    clicked
}

fn show_phrase_tab(
    ui: &mut Ui, t: &Theme, secret_words: &[&str], revealed: bool, can_done: bool,
    queue: &mut Vec<A>,
) {
    let grid_rect = show_display_grid(ui, t, secret_words, revealed);

    if !revealed {
        // Subtle veil + centered reveal pill over the masked 3×8 grid.
        ui.painter().rect_filled(
            grid_rect,
            Radius::Control.corner(),
            t.neutral_bg().gamma_multiply(0.45),
        );
        let hit = ui.interact(grid_rect, Id::unique("backup_phrase_reveal_hit"), sense_click());
        let btn_h = control_height();
        let btn_rect = Rect::from_center_size(grid_rect.center(), vec2(204.0, btn_h));
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(btn_rect)
                .layout(Layout::centered_and_justified(egui::Direction::LeftToRight)),
        );
        let btn_clicked = Button::primary(t, "Click to reveal phrase")
            .icon(phosphor::EYE)
            .height(btn_h)
            .show(&mut child)
            .clicked();
        if btn_clicked || hit.clicked() {
            queue.push(A::BackupKeyRevealPhrase);
        }
    } else {
        // Clicking the revealed phrase grid masks it again.
        let hit = ui.interact(grid_rect, Id::unique("backup_phrase_hide_hit"), sense_click());
        if hit.clicked() {
            queue.push(A::BackupKeyHidePhrase);
        }
    }

    ui.add(Spacer::new(Space::Lg));
    show_backup_footer(
        ui,
        t,
        "Copy",
        Id::unique("shell_backup_key_copy"),
        A::BackupKeyCopyPhrase,
        can_done,
        queue,
    );
}

fn show_backup_footer(
    ui: &mut Ui, t: &Theme, copy_label: &str, copy_feedback_id: Id, copy_action: A, can_done: bool,
    queue: &mut Vec<A>,
) {
    let row_w = ui_width(ui);
    let row_h = control_height();
    ui.horizontal(|ui| {
        ui.set_min_height(row_h);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        // Copy works immediately even before revealing the key on screen.
        let copy = Button::secondary(t, copy_label)
            .icon(phosphor::COPY)
            .copy_feedback(copy_feedback_id)
            .height(row_h)
            .show(ui);
        if copy.clicked() {
            queue.push(copy_action);
        }
        let right_w = (row_w - copy.rect.width() - Space::Sm.pts()).max(160.0);
        ui.add(Spacer::new(Space::Sm).fill_cross(row_h));
        ui.allocate_ui_with_layout(
            vec2(right_w, row_h),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let mut done = Button::primary(t, "Done").height(row_h).enabled(can_done);
                if can_done {
                    done = done.shortcut(shortcut_enter());
                }
                if done.show(ui).clicked() {
                    queue.push(A::BackupKeyComplete);
                }
                ui.add(Spacer::new(Space::Sm).fill_cross(row_h));
                if Button::quiet(t, "Later")
                    .shortcut(shortcut_esc())
                    .height(row_h)
                    .show(ui)
                    .clicked()
                {
                    queue.push(A::CloseModal);
                }
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::action::BackupKeyState;

    #[test]
    fn esc_closes_backup_key_modal_and_clears_qr() {
        let ctx = egui::Context::default();
        let state =
            BackupKeyState { tab: BackupKeyTab::Qr, qr_revealed: true, ..Default::default() };
        let mut app = ShellApp {
            session: crate::shell::Session::Error("test".into()),
            modal: Some(Modal::BackupKey(state)),
            ..Default::default()
        };
        *app.account_qr.lock().unwrap() = crate::shell::AccountQrCache::Ready(vec![1, 2, 3]);

        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000.0, 720.0))),
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        ctx.run_pass(input, |ctx| app.ui(ctx))
            .textures_delta
            .clear();

        assert!(app.modal.is_none(), "Esc must close the backup sheet");
        assert!(
            matches!(*app.account_qr.lock().unwrap(), crate::shell::AccountQrCache::Idle),
            "closing backup sheet must clear cached QR bytes"
        );
    }
}
