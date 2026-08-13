//! Single-value menu picker — color theme, landing page, etc.
//!
//! Trigger is a **canvas plate** quiet control ([`quiet_canvas_fills`] +
//! [`interact_fill`]). Menu is a canvas plate too. Keyboard: focus the
//! trigger, Space/Enter open, arrows move highlight, Enter commits, Esc closes.

use egui::{Color32, Key, Modifiers, Popup, PopupCloseBehavior, Response, Stroke, Ui, pos2, vec2};

use crate::components::foundation::chrome::{
    Radius, STROKE_HAIRLINE, canvas_overlay_frame, control_height, phosphor, phosphor_ui_font_id,
};
use crate::components::foundation::color::{STROKE_EMPHASIS, Theme};
use crate::components::foundation::interact::{
    canvas_selected_fills, interact_fill, quiet_canvas_fills,
};
use crate::components::foundation::space::Space;
use crate::components::foundation::space::control as control_space;
use crate::components::foundation::typography::TypeRole;

/// Compact picker (form trailing slot or standalone).
///
/// Shows `options[*selected]` with a caret; click or Space/Enter opens a menu.
/// Marks the response `.changed()` when the selection updates.
pub fn picker(ui: &mut Ui, t: &Theme, options: &[&str], selected: &mut usize) -> Response {
    let n = options.len().max(1);
    *selected = (*selected).min(n - 1);
    let label = options.get(*selected).copied().unwrap_or("—");

    let font = TypeRole::Body.font_id();
    let label_g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), t.neutral_fg());
    let caret_g = ui.painter().layout_no_wrap(
        phosphor::CARET_DOWN.to_owned(),
        phosphor_ui_font_id(),
        Color32::PLACEHOLDER,
    );
    let gap = control_space::ICON_GAP.pts();
    let pad_x = control_space::PAD_X.pts();
    let h = control_height();
    let min_w = Space::Xl.pts() * 3.0;
    let w = (label_g.size().x + gap + caret_g.size().x + pad_x * 2.0).max(min_w);

    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, h), crate::components::sense_click());
    if resp.clicked() {
        resp.request_focus();
    }

    let popup_id = Popup::default_response_id(&resp);
    let mut open = Popup::is_id_open(ui.ctx(), popup_id);
    let highlight_key = popup_id.with("hl");

    // Keyboard on the focused trigger (when menu closed).
    if resp.has_focus() && !open {
        let open_key = ui.ctx().input_mut(|i| {
            i.consume_key(Modifiers::NONE, Key::Enter)
                || i.consume_key(Modifiers::NONE, Key::Space)
                || i.consume_key(Modifiers::NONE, Key::ArrowDown)
        });
        if open_key {
            Popup::open_id(ui.ctx(), popup_id);
            open = true;
            ui.ctx()
                .data_mut(|d| d.insert_temp(highlight_key, *selected));
        }
    }

    // Keyboard while menu open (arrows / enter / esc).
    let mut pick_from_keys: Option<usize> = None;
    let mut close_menu = false;
    if open {
        let mut hl = ui
            .ctx()
            .data(|d| d.get_temp::<usize>(highlight_key))
            .unwrap_or(*selected)
            .min(n - 1);
        ui.ctx().input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                close_menu = true;
            } else if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                hl = (hl + 1) % n;
            } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                hl = if hl == 0 { n - 1 } else { hl - 1 };
            } else if i.consume_key(Modifiers::NONE, Key::Enter)
                || i.consume_key(Modifiers::NONE, Key::Space)
            {
                pick_from_keys = Some(hl);
            }
        });
        if close_menu {
            Popup::close_id(ui.ctx(), popup_id);
            open = false;
            ui.ctx().data_mut(|d| d.remove_temp::<usize>(highlight_key));
        } else {
            ui.ctx().data_mut(|d| d.insert_temp(highlight_key, hl));
        }
    }

    // Quiet canvas interaction — same stack as quiet buttons.
    let pointer_over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect) || open;
    let fills = quiet_canvas_fills(t);
    let fill = interact_fill(
        ui.ctx(),
        resp.id,
        pointer_over,
        resp.is_pointer_button_down_on(),
        resp.clicked(),
        fills,
    );
    let radius = Radius::Control.corner();
    ui.painter().rect_filled(rect, radius, fill);

    let stroke_on = pointer_over || open || resp.has_focus();
    let stroke_t = ui.ctx().animate_bool(resp.id.with("stroke"), stroke_on);
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(
            STROKE_HAIRLINE,
            t.neutral()
                .lerp_to_gamma(t.neutral_fg(), STROKE_EMPHASIS * stroke_t),
        ),
        egui::StrokeKind::Inside,
    );

    if resp.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(STROKE_HAIRLINE, t.neutral_fg()),
            egui::StrokeKind::Outside,
        );
    }

    let hover_t = ui
        .ctx()
        .animate_bool(resp.id.with("ink"), pointer_over || open);
    let caret_ink = t
        .neutral_fg_secondary()
        .lerp_to_gamma(t.neutral_fg(), hover_t);
    let cy = rect.center().y;
    let mut x = rect.left() + pad_x;
    ui.painter()
        .galley(pos2(x, cy - label_g.size().y / 2.0), label_g, t.neutral_fg());
    x = rect.right() - pad_x - caret_g.size().x;
    ui.painter()
        .galley(pos2(x, cy - caret_g.size().y / 2.0), caret_g, caret_ink);

    // Seed highlight when opened by click (toggle path).
    if open && resp.clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(highlight_key, *selected));
    }

    let highlight = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(highlight_key))
        .unwrap_or(*selected)
        .min(n - 1);

    let menu_w = w.max(Space::Xl.pts() * 4.0);
    let mut changed = false;
    let mut pick = pick_from_keys;

    if let Some(inner) = Popup::from_toggle_button_response(&resp)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .width(menu_w)
        .frame(canvas_overlay_frame(t, Space::Xxs))
        .show(|ui| {
            ui.set_min_width(menu_w);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let mut click_pick = None;
            for (i, &opt) in options.iter().enumerate() {
                if menu_row(ui, t, opt, i == *selected, i == highlight) {
                    click_pick = Some(i);
                }
            }
            click_pick
        })
    {
        if let Some(i) = inner.inner {
            pick = Some(i);
        }
    }

    if let Some(i) = pick {
        if i != *selected {
            *selected = i;
            changed = true;
        }
        Popup::close_id(ui.ctx(), popup_id);
        ui.ctx().data_mut(|d| d.remove_temp::<usize>(highlight_key));
    }

    if !Popup::is_id_open(ui.ctx(), popup_id) {
        ui.ctx().data_mut(|d| d.remove_temp::<usize>(highlight_key));
    }

    if changed {
        resp.mark_changed();
    }
    resp
}

/// Menu option on canvas. Idle `fg` (available). `selected` = value wash;
/// `highlighted` = keyboard cursor (hover wash if not selected).
fn menu_row(ui: &mut Ui, t: &Theme, label: &str, selected: bool, highlighted: bool) -> bool {
    let h = control_height();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(crate::components::ui_width(ui).max(1.0), h),
        crate::components::sense_click(),
    );
    let over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect) || highlighted;

    let fills = if selected { canvas_selected_fills(t) } else { quiet_canvas_fills(t) };
    let fill = interact_fill(
        ui.ctx(),
        resp.id,
        over,
        resp.is_pointer_button_down_on(),
        resp.clicked(),
        fills,
    );
    ui.painter().rect_filled(rect, Radius::Sm.corner(), fill);

    let g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), TypeRole::Body.font_id(), t.neutral_fg());
    ui.painter().galley(
        pos2(rect.left() + control_space::PAD_X.pts(), rect.center().y - g.size().y / 2.0),
        g,
        t.neutral_fg(),
    );

    resp.clicked()
}
