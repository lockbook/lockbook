//! Transcript column: header, scroll of session items, composer.

use egui::{
    Align, FontFamily, FontId, Id, Key, Layout, Modifiers, Popup, PopupCloseBehavior, Rect,
    ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2, vec2,
};

use lb_rs::model::chat::{Item, ItemKind, Status};

use lb_rs::Uuid;

use tracing::debug;

use super::{AuthUi, Chat, tools};
use crate::show::InputStateExt;
use crate::style::chrome::row_wash_inset;
use crate::style::space::control as control_space;
use crate::style::{
    Button, Radius, STROKE_HAIRLINE, Space, Spacer, ThemeExt, TypeRole, canvas_overlay_frame,
    canvas_selected_fills, claim, control_height, icon_button, icon_button_circle, interact_fill,
    paint_plate, phosphor, phosphor_ui_font_id, place_at, quiet_canvas_fills,
    quiet_secondary_fills, sense_click, tip_text, with_overlay_scroll,
};

const COMPOSER_MAX: f32 = 160.0;
const COL_MAX: f32 = 720.0;
/// User bubbles stay inside the column; 78% matches common chat UIs.
const USER_BUBBLE_FRAC: f32 = 0.78;

pub fn show(chat: &mut Chat, ui: &mut Ui) -> (bool, Rect, bool, bool) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let t = ui.ctx().get_lb_theme();
    let full = ui.max_rect();
    ui.set_clip_rect(full.intersect(ui.clip_rect()));
    ui.advance_cursor_after_rect(full);

    let pad = Space::Lg.pts();
    let col_w = (full.width() - pad * 2.0).clamp(0.0, COL_MAX).round();
    let col_left = (full.center().x - col_w / 2.0).round();

    let composer_id = Id::new((chat.id, "composer"));
    if !chat.initialized {
        ui.memory_mut(|m| m.request_focus(composer_id));
        chat.initialized = true;
    }

    let completions_open =
        chat.composer.emoji_completions.active || chat.composer.link_completions.active;
    let composer_focused = ui.memory(|m| m.has_focus(composer_id));
    let can_send = chat.signed_in() && !chat.busy();
    if (chat.busy() || chat.on_call()) && !completions_open && !Popup::is_any_open(ui.ctx()) {
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            chat.cancel_turn();
        }
    }
    let send_requested = can_send
        && composer_focused
        && !completions_open
        && ui.input_mut(|i| {
            i.consume_key(Modifiers::COMMAND, Key::Enter)
                || i.consume_key_exact(Modifiers::NONE, Key::Enter)
        });

    let ws = chat.composer.drain_workspace_events(ui.ctx());
    chat.composer.event.internal_events.extend(ws);
    let input = chat.composer.handle_input(ui.ctx(), composer_id);

    let mut sent = false;
    let mut text_updated = input.text_updated;
    if send_requested {
        sent |= take_send(chat);
        if sent {
            text_updated = true;
        }
    }

    let pad_l = Space::Md.pts();
    let pad_r_single = Space::Sm.pts();
    let pad_x_wrap = Space::Md.pts();
    let pad_y = Space::Sm.pts();
    let send_hit = control_height();
    let gap = Space::Xs.pts();
    let model_label = picker_label(chat);
    let model_g =
        ui.painter()
            .layout_no_wrap(model_label, TypeRole::Body.font_id(), t.neutral_fg());
    let caret_g = ui.painter().layout_no_wrap(
        phosphor::CARET_DOWN.into(),
        phosphor_ui_font_id(),
        t.neutral_fg(),
    );
    let model_w = (model_g.size().x
        + control_space::ICON_GAP.pts()
        + caret_g.size().x
        + control_space::PAD_X.pts() * 2.0)
        .max(1.0);
    let call_w = if chat.voice_ok() || chat.on_call() { send_hit + gap } else { 0.0 };
    let trailing = call_w + model_w + gap + send_hit;
    let wrap_w = (col_w - pad_x_wrap * 2.0).max(0.0);
    let inner_w = (col_w - pad_l - pad_r_single - gap - trailing).max(0.0);
    let row = chat.composer.row_height();
    let wide_h = chat.composer.measure_height(wrap_w);
    let measured = if wide_h <= row + 1.0 { chat.composer.measure_height(inner_w) } else { wide_h };
    let single = measured <= row + 1.0;
    let pad_r = if single { pad_r_single } else { pad_x_wrap };
    let text_w = if single { inner_w } else { wrap_w };
    let content_h = if single { measured.max(send_hit) } else { measured + gap + send_hit };
    let field_h = (content_h + pad_y * 2.0).clamp(send_hit + pad_y * 2.0, COMPOSER_MAX);
    let signed_in = chat.signed_in();
    let bottom_h = if signed_in { field_h } else { auth_height(&chat.auth_ui) };

    let bottom = Rect::from_min_max(
        pos2(col_left, (full.bottom() - bottom_h - Space::Lg.pts()).round()),
        pos2(col_left + col_w, (full.bottom() - Space::Lg.pts()).round()),
    );
    let transcript_rect = Rect::from_min_max(
        pos2(col_left, full.min.y.round()),
        pos2(col_left + col_w, (bottom.top() - Space::Md.pts()).round()),
    );

    paint_transcript(chat, ui, transcript_rect);

    let mut interaction = Rect::NOTHING;
    if signed_in {
        let plate = t.neutral_bg_secondary();
        paint_plate(ui, bottom, Radius::Control.corner(), plate, t.neutral());
        Spacer::paint_at(
            ui,
            Space::Md,
            Rect::from_min_size(bottom.min, vec2(pad_l, bottom.height())),
        );
        Spacer::paint_at(
            ui,
            if single { Space::Sm } else { Space::Md },
            Rect::from_min_max(pos2(bottom.max.x - pad_r, bottom.min.y), bottom.max),
        );
        Spacer::paint_at(
            ui,
            Space::Sm,
            Rect::from_min_size(bottom.min, vec2(bottom.width(), pad_y)),
        );
        Spacer::paint_at(
            ui,
            Space::Sm,
            Rect::from_min_max(pos2(bottom.min.x, bottom.max.y - pad_y), bottom.max),
        );
        let band = Rect::from_min_max(
            pos2(bottom.min.x + pad_l, bottom.min.y + pad_y),
            pos2(bottom.max.x - pad_r, bottom.max.y - pad_y),
        );
        let text_h = if single {
            measured.min(band.height()).max(row)
        } else {
            (band.height() - gap - send_hit).max(row)
        };
        let text_top = if single { band.center().y - text_h / 2.0 } else { band.min.y };
        let text_rect = Rect::from_min_size(pos2(band.min.x, text_top), vec2(text_w, text_h));
        chat.composer.show(ui, text_rect, composer_id);
        chat.composer_rect = text_rect;
        interaction = text_rect;

        let controls_cy = if single { band.center().y } else { band.max.y - send_hit / 2.0 };
        let send_rect = Rect::from_center_size(
            pos2(band.max.x - send_hit / 2.0, controls_cy),
            vec2(send_hit, send_hit),
        );
        let model_rect = Rect::from_center_size(
            pos2(send_rect.min.x - gap - model_w / 2.0, controls_cy),
            vec2(model_w, send_hit),
        );
        let call_rect = if call_w > 0.0 {
            Some(Rect::from_center_size(
                pos2(model_rect.min.x - gap - send_hit / 2.0, controls_cy),
                vec2(send_hit, send_hit),
            ))
        } else {
            None
        };
        if single {
            let cut = call_rect.map(|r| r.min.x).unwrap_or(model_rect.min.x);
            Spacer::paint_at(
                ui,
                Space::Xs,
                Rect::from_min_max(pos2(cut - gap, band.min.y), pos2(cut, band.max.y)),
            );
        } else {
            Spacer::paint_at(
                ui,
                Space::Xs,
                Rect::from_min_max(
                    pos2(band.min.x, text_rect.max.y),
                    pos2(band.max.x, send_rect.min.y),
                ),
            );
        }

        if let Some(call_rect) = call_rect {
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(call_rect)
                    .layout(Layout::left_to_right(Align::Center)),
                |ui| {
                    let on = chat.on_call();
                    let glyph = if on { phosphor::PHONE_SLASH } else { phosphor::PHONE };
                    let resp = icon_button_circle(
                        ui,
                        &t,
                        glyph,
                        true,
                        plate,
                        send_hit,
                        TypeRole::Body.size(),
                    );
                    tip_text(ui.ctx(), &resp, if on { "Hang up" } else { "Call" });
                    if resp.clicked() {
                        chat.toggle_call();
                    }
                },
            );
        }

        ui.scope_builder(
            UiBuilder::new()
                .max_rect(model_rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                paint_model_picker(ui, &t, chat);
            },
        );

        let mut send_clicked = false;
        let mut stop_clicked = false;
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(send_rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                if chat.busy() {
                    let resp = icon_button_circle(
                        ui,
                        &t,
                        phosphor::SQUARE,
                        true,
                        plate,
                        send_hit,
                        TypeRole::Body.size(),
                    );
                    tip_text(ui.ctx(), &resp, "Stop · Esc");
                    if resp.clicked() {
                        stop_clicked = true;
                    }
                } else {
                    let empty = chat.composer.renderer.buffer.current.text.trim().is_empty();
                    let active = can_send && !empty;
                    let resp = icon_button_circle(
                        ui,
                        &t,
                        phosphor::PAPER_PLANE_TILT,
                        active,
                        plate,
                        send_hit,
                        TypeRole::Body.size(),
                    );
                    tip_text(ui.ctx(), &resp, "Send message · Enter");
                    if resp.clicked() && active {
                        send_clicked = true;
                    }
                }
            },
        );
        if stop_clicked {
            chat.cancel_turn();
        }
        if send_clicked {
            sent |= take_send(chat);
            if sent {
                text_updated = true;
            }
        }
        chat.composer.show_completions(ui);
    } else {
        chat.composer_rect = Rect::NOTHING;
        paint_auth(chat, ui, bottom);
    }

    (sent, interaction, input.selection_user_moved || text_updated, text_updated)
}

fn picker_label(chat: &Chat) -> String {
    if chat.on_call() {
        voice_title(&chat.current_voice())
    } else {
        super::grok::model_label(&chat.model).to_string()
    }
}

fn voice_title(id: &str) -> String {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
        None => id.to_string(),
    }
}

fn picker_items(chat: &Chat) -> (Vec<(String, String)>, String, &'static str) {
    if chat.on_call() {
        let items = super::tools::VOICES
            .iter()
            .map(|&id| (voice_title(id), id.to_string()))
            .collect();
        (items, chat.current_voice(), "Voice")
    } else {
        let items = super::grok::MODELS
            .iter()
            .map(|&(name, id)| (name.to_string(), id.to_string()))
            .collect();
        (items, chat.model.clone(), "Model")
    }
}

fn paint_model_picker(ui: &mut Ui, t: &crate::style::Theme, chat: &mut Chat) {
    let (items, current, tip) = picker_items(chat);
    let n = items.len().max(1);
    let selected = items
        .iter()
        .position(|(_, id)| id == &current)
        .unwrap_or(0)
        .min(n - 1);
    let label = picker_label(chat);
    let pad_x = control_space::PAD_X.pts();
    let gap = control_space::ICON_GAP.pts();
    let h = ui.available_height().max(control_height());
    let w = ui.available_width().max(1.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense_click());
    if resp.clicked() {
        resp.request_focus();
    }

    let popup_id = Popup::default_response_id(&resp);
    let mut open = Popup::is_id_open(ui.ctx(), popup_id);
    let highlight_key = popup_id.with("hl");

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
                .data_mut(|d| d.insert_temp(highlight_key, selected));
        }
    }

    let mut pick_from_keys: Option<usize> = None;
    let mut close_menu = false;
    if open {
        let mut hl = ui
            .ctx()
            .data(|d| d.get_temp::<usize>(highlight_key))
            .unwrap_or(selected)
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

    let pointer_over = ui.ctx().rect_contains_pointer(ui.layer_id(), rect) || open;
    let fills = quiet_secondary_fills(t);
    let fill = interact_fill(
        ui.ctx(),
        resp.id,
        pointer_over,
        resp.is_pointer_button_down_on(),
        resp.clicked(),
        fills,
    );
    let radius = Radius::Control.corner();
    if fill != fills.rest {
        ui.painter()
            .rect_filled(rect.shrink(row_wash_inset()), radius, fill);
    }

    let hover_t = ui
        .ctx()
        .animate_bool(resp.id.with("ink"), pointer_over || open);
    let caret_ink = t
        .neutral_fg_secondary()
        .lerp_to_gamma(t.neutral_fg(), hover_t);
    let label_g =
        ui.painter()
            .layout_no_wrap(label.to_owned(), TypeRole::Body.font_id(), t.neutral_fg());
    let caret_g =
        ui.painter()
            .layout_no_wrap(phosphor::CARET_DOWN.into(), phosphor_ui_font_id(), caret_ink);
    let cy = rect.center().y;
    ui.painter().galley(
        pos2(rect.left() + pad_x, cy - label_g.size().y / 2.0),
        label_g,
        t.neutral_fg(),
    );
    ui.painter().galley(
        pos2(rect.right() - pad_x - caret_g.size().x, cy - caret_g.size().y / 2.0),
        caret_g,
        caret_ink,
    );

    if resp.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(STROKE_HAIRLINE, t.neutral_fg()),
            egui::StrokeKind::Outside,
        );
    }

    tip_text(ui.ctx(), &resp, tip);

    if open && resp.clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(highlight_key, selected));
    }

    let highlight = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(highlight_key))
        .unwrap_or(selected)
        .min(n - 1);

    let check_slot = crate::style::tree_metrics::ICON_SLOT;
    let longest = items
        .iter()
        .map(|(name, _)| {
            ui.painter()
                .layout_no_wrap(name.clone(), TypeRole::Body.font_id(), t.neutral_fg())
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let menu_w = rect.width().max(pad_x * 2.0 + check_slot + gap + longest);

    let mut pick = pick_from_keys;
    if let Some(inner) = Popup::from_toggle_button_response(&resp)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .width(menu_w)
        .frame(canvas_overlay_frame(t, Space::Xxs))
        .show(|ui| {
            ui.set_min_width(menu_w);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let mut click_pick = None;
            for (i, (name, _)) in items.iter().enumerate() {
                if model_menu_row(ui, t, name, i == selected, i == highlight, check_slot) {
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
        if let Some((_, id)) = items.get(i) {
            if chat.on_call() {
                chat.set_voice(id);
            } else {
                chat.set_text_model(id);
            }
        }
        Popup::close_id(ui.ctx(), popup_id);
        ui.ctx().data_mut(|d| d.remove_temp::<usize>(highlight_key));
    }

    if !Popup::is_id_open(ui.ctx(), popup_id) {
        ui.ctx().data_mut(|d| d.remove_temp::<usize>(highlight_key));
    }
}

/// Menu option: check in a reserved slot marks the current value; idle rows
/// keep the same ink (never disabled / dimmed). Selected wash; hover wash.
fn model_menu_row(
    ui: &mut Ui, t: &crate::style::Theme, label: &str, selected: bool, highlighted: bool,
    check_slot: f32,
) -> bool {
    let h = control_height();
    let pad_x = control_space::PAD_X.pts();
    let (rect, resp) =
        ui.allocate_exact_size(vec2(ui.available_width().max(1.0), h), sense_click());
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
    if fill != t.neutral_bg() {
        ui.painter()
            .rect_filled(rect.shrink(row_wash_inset()), Radius::Sm.corner(), fill);
    }

    let ink = t.neutral_fg();
    if selected {
        let check = ui
            .painter()
            .layout_no_wrap(phosphor::CHECK.into(), phosphor_ui_font_id(), ink);
        ui.painter().galley(
            pos2(
                rect.left() + pad_x + (check_slot - check.size().x).max(0.0) / 2.0,
                rect.center().y - check.size().y / 2.0,
            ),
            check,
            ink,
        );
    }
    let label_g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), TypeRole::Body.font_id(), ink);
    ui.painter().galley(
        pos2(rect.left() + pad_x + check_slot, rect.center().y - label_g.size().y / 2.0),
        label_g,
        ink,
    );
    resp.clicked()
}

fn take_send(chat: &mut Chat) -> bool {
    let text = chat.composer.renderer.buffer.current.text.clone();
    if text.trim().is_empty() {
        return false;
    }
    chat.say(text);
    chat.composer.set_text("");
    true
}

fn auth_height(auth: &AuthUi) -> f32 {
    let line = TypeRole::Body.line_height();
    let pad = Space::Sm.pts() * 2.0;
    match auth {
        AuthUi::Pending { user_code, .. } if !user_code.is_empty() => {
            pad + line * 2.0 + control_height() + Space::Xs.pts()
        }
        AuthUi::Error(_) => pad + line * 3.0 + control_height(),
        _ => pad + line + control_height() + Space::Xs.pts(),
    }
}

fn paint_auth(chat: &mut Chat, ui: &mut Ui, rect: Rect) {
    let t = ui.ctx().get_lb_theme();
    paint_plate(ui, rect, Radius::Control.corner(), t.neutral_bg_secondary(), t.neutral());
    paint_inset_spacers(ui, rect, Space::Sm);
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink(Space::Sm.pts()))
            .layout(Layout::top_down(Align::LEFT)),
        |ui| match &chat.auth_ui {
            AuthUi::SignedOut => {
                ui.label(
                    TypeRole::Body
                        .rich("Sign in with SuperGrok to talk.")
                        .color(t.neutral_fg()),
                );
                ui.add(Spacer::new(Space::Xs));
                let resp = Button::primary(&t, "Sign in with Grok").show(ui);
                tip_text(ui.ctx(), &resp, "Opens xAI device sign-in");
                if resp.clicked() {
                    chat.start_login();
                }
            }
            AuthUi::Pending { user_code, .. } if user_code.is_empty() => {
                ui.label(
                    TypeRole::Body
                        .rich("Starting sign-in…")
                        .color(t.neutral_fg_secondary()),
                );
            }
            AuthUi::Pending { user_code, url } => {
                let code = user_code.clone();
                let url = url.clone();
                ui.label(
                    TypeRole::Body
                        .rich(format!("Enter {code} at xAI"))
                        .color(t.neutral_fg()),
                );
                ui.add(Spacer::new(Space::Xs));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let copy = Button::secondary(&t, "Copy code")
                        .copy_feedback(Id::new("chat_copy_code"))
                        .show(ui);
                    tip_text(ui.ctx(), &copy, "Copy the device code");
                    if copy.clicked() {
                        ui.ctx().copy_text(code);
                    }
                    ui.add(Spacer::new(Space::Xs).fill_cross(control_height()));
                    let open = Button::primary(&t, "Open xAI").show(ui);
                    tip_text(ui.ctx(), &open, "Open the xAI sign-in page");
                    if open.clicked() {
                        ui.ctx().open_url(egui::OpenUrl { url, new_tab: true });
                    }
                });
            }
            AuthUi::Error(e) => {
                let msg = e.clone();
                ui.label(TypeRole::Body.rich(msg).color(t.danger()));
                ui.add(Spacer::new(Space::Xs));
                let resp = Button::primary(&t, "Try again").show(ui);
                tip_text(ui.ctx(), &resp, "Restart SuperGrok sign-in");
                if resp.clicked() {
                    chat.start_login();
                }
            }
            AuthUi::Ready => {}
        },
    );
}

/// Tools of a visible assistant paint first so search rows stack above the
/// streaming answer. Scroll-follow then tracks the text, not the tool list.
/// The log stays Open order; this is display only.
fn display_indices(items: &[Item]) -> Vec<usize> {
    let mut used = vec![false; items.len()];
    let mut out = Vec::with_capacity(items.len());
    for i in 0..items.len() {
        if used[i] || skip_item(items, i) {
            continue;
        }
        if items[i].kind == ItemKind::Assistant {
            for (j, it) in items.iter().enumerate() {
                if used[j] || skip_item(items, j) {
                    continue;
                }
                if it.kind == ItemKind::Tool && it.parent == Some(items[i].id) {
                    out.push(j);
                    used[j] = true;
                }
            }
        }
        out.push(i);
        used[i] = true;
    }
    out
}

fn skip_item(items: &[Item], i: usize) -> bool {
    let item = &items[i];
    match item.kind {
        ItemKind::Error => !live_error(items, i),
        // Tool-only rounds leave an empty Done assistant between batches.
        ItemKind::Assistant if !item.has_text() && !item.status.in_flight() => true,
        ItemKind::Assistant if item.status == Status::Failed => {
            items[i + 1..].iter().any(|x| x.kind == ItemKind::Assistant)
        }
        ItemKind::Thought if !item.has_text() && !item.status.in_flight() => true,
        _ => false,
    }
}

fn gap_before(prev: ItemKind, cur: ItemKind) -> Space {
    match (prev, cur) {
        (ItemKind::Tool, ItemKind::Tool) | (ItemKind::Plan, ItemKind::Plan) => Space::Xs,
        (ItemKind::Thought, ItemKind::Assistant) => Space::Xs,
        (ItemKind::Assistant, ItemKind::Tool | ItemKind::Plan | ItemKind::Thought) => Space::Sm,
        _ => Space::Md,
    }
}

fn claim_v_space(ui: &mut Ui, token: Space, x: f32, y: f32, w: f32) -> f32 {
    let h = token.pts();
    let r = Rect::from_min_size(pos2(x, y), vec2(w, h));
    Spacer::paint_at(ui, token, r);
    claim(ui, r);
    h
}

/// Place a transcript column at `y` and claim only the used height.
fn place_column(
    ui: &mut Ui, host: Rect, y: f32, pad: f32, content_w: f32, add: impl FnOnce(&mut Ui),
) -> f32 {
    let slot = Rect::from_min_size(pos2(host.left() + pad, y), vec2(content_w, 4096.0));
    let (_, used) = place_at(ui, slot, Layout::top_down(Align::Min), add);
    let h = used.height().max(0.0);
    if h > 0.0 {
        Spacer::paint_at(ui, Space::Md, Rect::from_min_size(pos2(host.left(), y), vec2(pad, h)));
        Spacer::paint_at(
            ui,
            Space::Md,
            Rect::from_min_size(pos2(host.right() - pad, y), vec2(pad, h)),
        );
        claim(ui, Rect::from_min_size(pos2(host.left(), y), vec2(host.width(), h)));
    }
    h
}

fn paint_transcript(chat: &mut Chat, ui: &mut Ui, rect: Rect) {
    let t = ui.ctx().get_lb_theme();
    let pad = Space::Md.pts();
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.set_clip_rect(rect.intersect(ui.clip_rect()));
        if chat.transcript.items.is_empty() {
            if chat.signed_in() {
                let msg = "Message Grok";
                let g = ui.painter().layout_no_wrap(
                    msg.into(),
                    TypeRole::Body.font_id(),
                    t.neutral_fg_secondary(),
                );
                let pos =
                    pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0);
                ui.painter().galley(pos, g, t.neutral_fg_secondary());
            }
            return;
        }

        with_overlay_scroll(ui, Id::new((chat.id, "scroll")), |ui| {
            let out = ScrollArea::vertical()
                .id_salt((chat.id, "messages"))
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let host = ui.max_rect();
                    let content_w = (host.width() - pad * 2.0).max(0.0);
                    let mut y = host.top();
                    y += claim_v_space(ui, Space::Md, host.left(), y, host.width());
                    let items = chat.transcript.items.clone();
                    let me = chat.account.username.clone();
                    let show_from = items
                        .iter()
                        .any(|i| i.kind == ItemKind::User && i.from != me);
                    let mut prev_kind: Option<ItemKind> = None;
                    let log_key = Id::new((chat.id, "layout_seq"));
                    let log_layout =
                        ui.ctx().data(|d| d.get_temp::<usize>(log_key)) != Some(chat.seq);
                    let mut layout_trace = String::new();
                    for i in display_indices(&items) {
                        let item = &items[i];
                        if skip_item(&items, i) {
                            if log_layout {
                                if !layout_trace.is_empty() {
                                    layout_trace.push(' ');
                                }
                                layout_trace.push_str(&format!(
                                    "{}:{}:skip",
                                    kind_tag(item.kind),
                                    status_tag(item.status)
                                ));
                            }
                            continue;
                        }
                        let gap = prev_kind.map(|prev| gap_before(prev, item.kind));
                        if let Some(g) = gap {
                            y += claim_v_space(ui, g, host.left(), y, host.width());
                        }
                        prev_kind = Some(item.kind);
                        let retryable = item.kind == ItemKind::Error && live_error(&items, i);
                        let h = place_column(ui, host, y, pad, content_w, |ui| {
                            paint_item(chat, ui, item, show_from, retryable);
                        });
                        if log_layout {
                            if !layout_trace.is_empty() {
                                layout_trace.push(' ');
                            }
                            layout_trace.push_str(&format!(
                                "{}:{}:gap={}:h={:.0}:text={}",
                                kind_tag(item.kind),
                                status_tag(item.status),
                                gap.map(space_tag).unwrap_or("none"),
                                h,
                                item.has_text() as u8
                            ));
                        }
                        y += h;
                    }
                    if log_layout {
                        debug!(seq = chat.seq, n = items.len(), layout = %layout_trace, "chat transcript layout");
                        ui.ctx().data_mut(|d| d.insert_temp(log_key, chat.seq));
                    }
                    let has_live_error = items
                        .iter()
                        .enumerate()
                        .any(|(i, it)| it.kind == ItemKind::Error && live_error(&items, i));
                    if chat.needs_turn()
                        && chat.signed_in()
                        && !chat.busy()
                        && !chat.on_call()
                        && !has_live_error
                    {
                        y += claim_v_space(ui, Space::Md, host.left(), y, host.width());
                        y += place_column(ui, host, y, pad, content_w, |ui| {
                            paint_notice(
                                chat,
                                ui,
                                phosphor::ARROW_COUNTER_CLOCKWISE,
                                "This turn didn’t finish",
                                "",
                                false,
                                true,
                            );
                        });
                    }
                    y += claim_v_space(ui, Space::Md, host.left(), y, host.width());
                    let _ = y;
                });
            ((), out.state.offset.y, out.id)
        });
    });
}

fn live_error(items: &[Item], i: usize) -> bool {
    !items[i + 1..]
        .iter()
        .any(|x| x.kind == ItemKind::Assistant || x.kind == ItemKind::Error)
}

fn paint_item(chat: &mut Chat, ui: &mut Ui, item: &Item, show_from: bool, retryable: bool) {
    let t = ui.ctx().get_lb_theme();
    match item.kind {
        ItemKind::User => {
            if show_from {
                let name =
                    if item.from == chat.account.username { "You" } else { item.from.as_str() };
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    ui.label(TypeRole::Body.rich(name).color(t.neutral_fg_secondary()));
                });
                ui.add(Spacer::new(Space::Xxs));
            }
            let body = item.text();
            if body.is_empty() && item.status.in_flight() {
                paint_user_bubble(chat, ui, item.id, "…");
            } else if !body.is_empty() {
                paint_user_bubble(chat, ui, item.id, &body);
            }
        }
        ItemKind::Assistant => {
            let body = item.text();
            if !body.is_empty() {
                let rect = paint_markdown(chat, ui, item.id, &body, ui.available_width());
                paint_copy_overlay(ui, item.id, &body, rect);
            } else if item.status.in_flight() {
                ui.label(
                    TypeRole::Body
                        .rich("Thinking…")
                        .color(t.neutral_fg_secondary()),
                );
            }
        }
        ItemKind::Thought => {
            paint_disclosure(chat, ui, item);
        }
        ItemKind::Tool | ItemKind::Plan => {
            paint_disclosure(chat, ui, item);
        }
        ItemKind::Ask => {
            paint_notice(chat, ui, phosphor::INFO, "Ask", &item.title(), false, false);
        }
        ItemKind::Error => {
            let detail = item.title();
            paint_notice(
                chat,
                ui,
                phosphor::WARNING_CIRCLE,
                "Couldn't complete that turn",
                &detail,
                true,
                retryable,
            );
        }
    }
}

fn paint_markdown(chat: &mut Chat, ui: &mut Ui, id: Uuid, md: &str, width: f32) -> Rect {
    let width = width.max(24.0);
    let h = chat.md_label(id).height(md, width);
    let (rect, _) = ui.allocate_exact_size(vec2(width, h.max(1.0)), Sense::hover());
    let (areas, _) = chat.md_label(id).paint_at(ui, md, rect.min, width);
    if !areas.is_empty() {
        ui.painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                ui.clip_rect(),
                crate::GlyphonRendererCallback::new(areas),
            ));
    }
    rect
}

fn paint_user_bubble(chat: &mut Chat, ui: &mut Ui, id: Uuid, text: &str) {
    let t = ui.ctx().get_lb_theme();
    let pad = Space::Sm.pts();
    let max_w = (ui.available_width() * USER_BUBBLE_FRAC).max(80.0);
    let inner_w = (max_w - pad * 2.0).max(24.0);
    let h = chat.md_label(id).height(text, inner_w);
    let size = vec2(inner_w + pad * 2.0, h + pad * 2.0);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), size.y),
        Layout::right_to_left(Align::TOP),
        |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            paint_plate(ui, rect, Radius::Control.corner(), t.neutral_bg_secondary(), t.neutral());
            paint_inset_spacers(ui, rect, Space::Sm);
            let origin = rect.min + vec2(pad, pad);
            let (areas, _) = chat.md_label(id).paint_at(ui, text, origin, inner_w);
            if !areas.is_empty() {
                ui.painter()
                    .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                        ui.clip_rect(),
                        crate::GlyphonRendererCallback::new(areas),
                    ));
            }
        },
    );
}

fn row_icon(item: &Item) -> &'static str {
    match item.kind {
        ItemKind::Thought => phosphor::LIGHTBULB,
        ItemKind::Plan => phosphor::LIST_NUMBERS,
        ItemKind::Tool => match item.meta.tool_kind.as_deref() {
            Some("list") => phosphor::LIST_BULLETS,
            Some("read") => phosphor::FILE_TEXT,
            Some("info") => phosphor::INFO,
            Some("this") => phosphor::CHAT,
            Some("tabs") => phosphor::TABS,
            Some("edit") => phosphor::PENCIL,
            Some("search") => phosphor::SEARCH,
            Some(n) if n.starts_with("x_") || n == "view_x_video" => phosphor::X_LOGO,
            Some(n) if n.starts_with("code_") => phosphor::CODE,
            Some(n) if tools::is_server(n) => phosphor::GLOBE,
            Some("create") => {
                let folder = item
                    .meta
                    .args
                    .as_ref()
                    .and_then(|a| a.get("path"))
                    .and_then(|p| p.as_str())
                    .is_some_and(|p| p.ends_with('/'));
                if folder { phosphor::FOLDER_PLUS } else { phosphor::FILE_PLUS }
            }
            Some("rename") => phosphor::TEXT_T,
            Some("move") => phosphor::FOLDER,
            Some("delete") => phosphor::TRASH,
            Some("recent") => phosphor::CLOCK,
            Some("pin") => phosphor::PUSH_PIN,
            Some("duplicate") => phosphor::COPY,
            Some("share") => phosphor::USERS,
            Some("contacts") => phosphor::USER_CHECK,
            Some("account") => phosphor::USER,
            Some("now") => phosphor::CLOCK,
            Some("hangup") => phosphor::PHONE_SLASH,
            Some("imagine") => phosphor::IMAGE,
            Some("look") => phosphor::EYE,
            Some("download") => phosphor::DOWNLOAD_SIMPLE,
            Some("caption") => phosphor::TEXT_AA,
            Some("transcribe") => phosphor::CHAT,
            Some("transcript") => phosphor::CHAT,
            Some("record") => phosphor::MEGAPHONE,
            Some("settings") => phosphor::GEAR,
            _ => phosphor::GEAR,
        },
        _ => phosphor::GEAR,
    }
}

fn row_summary(item: &Item) -> String {
    match item.kind {
        ItemKind::Thought => "Thought".into(),
        ItemKind::Plan => item
            .meta
            .title
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Plan".into()),
        _ => item
            .meta
            .title
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| item.meta.tool_kind.clone().unwrap_or_else(|| "tool".into())),
    }
}

fn row_metric(status: Status) -> &'static str {
    match status {
        Status::Running | Status::Pending | Status::Open => "…",
        Status::Done => "",
        Status::Failed => "failed",
        Status::Cancelled => "cancelled",
    }
}

fn tool_summary_parts(summary: &str) -> (&str, Option<&str>) {
    let Some((verb, rest)) = summary.split_once(' ') else {
        return (summary, None);
    };
    let rest = rest.trim();
    if rest.starts_with('/') { (verb, Some(rest)) } else { (summary, None) }
}

fn metric_pulse(ui: &Ui, base: egui::Color32) -> egui::Color32 {
    if ui.ctx().style().animation_time < 0.01 {
        return base;
    }
    ui.ctx().request_repaint();
    let t = ui.input(|i| i.time);
    let s = ((t * std::f64::consts::TAU / 1.4).sin() * 0.5 + 0.5) as f32;
    base.gamma_multiply(0.4 + 0.6 * s)
}

fn disclosure_detail(item: &Item) -> String {
    match item.kind {
        ItemKind::Tool => tools::expand_md(
            item.meta.tool_kind.as_deref().unwrap_or(""),
            item.meta.args.as_ref(),
            &item.text(),
        ),
        _ => item.text(),
    }
}

fn paint_disclosure(chat: &mut Chat, ui: &mut Ui, item: &Item) {
    let t = ui.ctx().get_lb_theme();
    let running = item.status.in_flight();
    let detail = disclosure_detail(item);
    let can_expand = !detail.is_empty();
    let icon = row_icon(item);
    let summary = row_summary(item);
    let metric = row_metric(item.status);
    let muted = t.neutral_fg_secondary();
    let ink = if item.status == Status::Failed { t.danger() } else { t.neutral_fg() };
    let mut metric_ink = if item.status == Status::Failed { t.danger() } else { muted };
    if running {
        metric_ink = metric_pulse(ui, muted);
    }

    let h = control_height();
    let sense = if can_expand { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), sense);
    if resp.hovered() {
        ui.painter().rect_filled(
            rect.shrink(crate::style::chrome::row_wash_inset()),
            Radius::Control.corner(),
            t.neutral_bg_secondary(),
        );
    }
    if resp.clicked() && can_expand {
        if !chat.expanded.insert(item.id) {
            chat.expanded.remove(&item.id);
        }
    }
    let open = match item.kind {
        ItemKind::Tool => can_expand && chat.expanded.contains(&item.id),
        _ => (running && can_expand) || chat.expanded.contains(&item.id),
    };
    if can_expand {
        let tip = if open { "Hide details" } else { "Show details" };
        tip_text(ui.ctx(), &resp, tip);
    }
    let caret = if !can_expand {
        ""
    } else if open {
        phosphor::CARET_DOWN
    } else {
        phosphor::CARET_RIGHT
    };

    let mut x = rect.left();
    let mid_y = rect.center().y;
    let caret_g = ui.painter().layout_no_wrap(
        if caret.is_empty() { phosphor::CARET_RIGHT.into() } else { caret.into() },
        crate::style::phosphor_ui_font_id(),
        muted,
    );
    if !caret.is_empty() {
        ui.painter()
            .galley(pos2(x, mid_y - caret_g.size().y / 2.0), caret_g.clone(), muted);
    }
    x += caret_g.size().x + Space::Xs.pts();

    let icon_g =
        ui.painter()
            .layout_no_wrap(icon.into(), crate::style::phosphor_ui_font_id(), muted);
    ui.painter()
        .galley(pos2(x, mid_y - icon_g.size().y / 2.0), icon_g.clone(), muted);
    x += icon_g.size().x + Space::Xs.pts();
    let text_x = x;

    let metric_g = if metric.is_empty() {
        None
    } else {
        Some(
            ui.painter()
                .layout_no_wrap(metric.into(), TypeRole::Body.font_id(), metric_ink),
        )
    };
    let metric_w = metric_g
        .as_ref()
        .map(|g| g.size().x + Space::Xs.pts())
        .unwrap_or(0.0);
    let summary_max = (rect.right() - x - metric_w).max(24.0);

    let (verb, path) = if item.kind == ItemKind::Tool {
        tool_summary_parts(&summary)
    } else {
        (summary.as_str(), None)
    };
    let verb_g = if path.is_some() {
        ui.painter()
            .layout_no_wrap(verb.to_owned(), TypeRole::Body.font_id(), ink)
    } else {
        fit_line(ui, verb.to_owned(), TypeRole::Body.font_id(), ink, summary_max)
    };
    let path_g = path.map(|p| {
        let remain = (summary_max - verb_g.size().x - Space::Xs.pts()).max(24.0);
        fit_line(
            ui,
            p.to_owned(),
            FontId::new(TypeRole::Mono.size(), FontFamily::Monospace),
            ink,
            remain,
        )
    });
    let block_h = verb_g
        .size()
        .y
        .max(path_g.as_ref().map(|g| g.size().y).unwrap_or(0.0));
    let block_top = mid_y - block_h / 2.0;
    ui.painter()
        .galley(pos2(x, block_top + block_h - verb_g.size().y), verb_g.clone(), ink);
    x += verb_g.size().x;
    if let Some(g) = path_g {
        x += Space::Xs.pts();
        ui.painter()
            .galley(pos2(x, block_top + block_h - g.size().y), g, ink);
    }

    if let Some(g) = metric_g {
        ui.painter().galley(
            pos2(rect.right() - g.size().x, mid_y - g.size().y / 2.0),
            g,
            metric_ink,
        );
    }

    if open && !detail.is_empty() {
        ui.add(Spacer::new(Space::Xxs));
        let indent = text_x - rect.left();
        let inner_w = (ui.available_width() - indent).max(24.0);
        let id = item.id;
        let h = chat.md_label(id).height(&detail, inner_w);
        let (body_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), h.max(1.0)), Sense::hover());
        let origin = pos2((body_rect.left() + indent).round(), body_rect.top());
        let (areas, _) = chat.md_label(id).paint_at(ui, &detail, origin, inner_w);
        if !areas.is_empty() {
            ui.painter()
                .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    ui.clip_rect(),
                    crate::GlyphonRendererCallback::new(areas),
                ));
        }
    }
}

fn paint_copy_overlay(ui: &mut Ui, id: Uuid, text: &str, host: Rect) {
    let t = ui.ctx().get_lb_theme();
    let hit = control_height();
    let btn = Rect::from_min_size(
        pos2((host.right() - hit).min(host.right()).max(host.left()), host.top()),
        vec2(hit, hit),
    );
    let fid = Id::new(("chat_copy", id));
    let now = ui.input(|i| i.time);
    let until = ui
        .ctx()
        .data(|d| d.get_temp::<f64>(fid.with("copied_until")))
        .unwrap_or(0.0);
    let copied = now < until;
    if copied {
        ui.ctx().request_repaint();
    }
    let layer = ui.layer_id();
    let over =
        ui.ctx().rect_contains_pointer(layer, host) || ui.ctx().rect_contains_pointer(layer, btn);
    if !over && !copied {
        return;
    }
    let icon = if copied { phosphor::CHECK } else { phosphor::COPY };
    let (resp, _) = place_at(ui, btn, Layout::left_to_right(Align::Center), |ui| {
        icon_button(ui, &t, icon, copied, t.neutral_bg())
    });
    tip_text(ui.ctx(), &resp, "Copy");
    if resp.clicked() {
        ui.ctx().copy_text(text.to_owned());
        ui.ctx().data_mut(|d| {
            d.insert_temp(fid.with("copied_until"), now + 1.2);
        });
    }
}

fn paint_notice(
    chat: &mut Chat, ui: &mut Ui, icon: &'static str, title: &str, detail: &str, danger: bool,
    retryable: bool,
) {
    let t = ui.ctx().get_lb_theme();
    let fill = if danger { t.danger().gamma_multiply(0.12) } else { t.neutral_bg_secondary() };
    let title_ink = if danger { t.danger() } else { t.neutral_fg() };
    let body_ink = t.neutral_fg_secondary();
    let pad = Space::Sm.pts();
    let icon_g =
        ui.painter()
            .layout_no_wrap(icon.into(), crate::style::phosphor_ui_font_id(), title_ink);
    let title_max =
        (ui.available_width() - pad * 2.0 - icon_g.size().x - Space::Xs.pts()).max(24.0);
    let title_g =
        ui.painter()
            .layout(title.to_owned(), TypeRole::Body.font_id(), title_ink, title_max);
    let detail = detail.trim();
    let show_detail = !detail.is_empty() && detail != title;
    let detail_g = if show_detail {
        Some(ui.painter().layout(
            detail.to_owned(),
            TypeRole::Body.font_id(),
            body_ink,
            (ui.available_width() - pad * 2.0).max(24.0),
        ))
    } else {
        None
    };

    let title_h = title_g.size().y.max(icon_g.size().y);
    let mut h = pad * 2.0 + title_h;
    if let Some(g) = &detail_g {
        h += Space::Xs.pts() + g.size().y;
    }
    if retryable {
        h += Space::Xs.pts() + control_height();
    }
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    paint_plate(ui, rect, Radius::Control.corner(), fill, t.neutral());
    paint_inset_spacers(ui, rect, Space::Sm);

    let icon_pos = pos2(rect.left() + pad, rect.top() + pad + (title_h - icon_g.size().y) / 2.0);
    let icon_w = icon_g.size().x;
    ui.painter().galley(icon_pos, icon_g, title_ink);
    ui.painter().galley(
        pos2(icon_pos.x + icon_w + Space::Xs.pts(), rect.top() + pad),
        title_g,
        title_ink,
    );
    if let Some(g) = detail_g {
        ui.painter().galley(
            pos2(rect.left() + pad, rect.top() + pad + title_h + Space::Xs.pts()),
            g,
            body_ink,
        );
    }

    if retryable {
        let btn = Rect::from_min_size(
            pos2(rect.left() + pad, rect.bottom() - pad - control_height()),
            vec2((rect.width() - pad * 2.0).max(1.0), control_height()),
        );
        let (clicked, _) = place_at(ui, btn, Layout::left_to_right(Align::Center), |ui| {
            let resp = Button::secondary(&t, "Retry")
                .enabled(chat.signed_in() && !chat.busy())
                .show(ui);
            tip_text(ui.ctx(), &resp, "Send this turn again");
            resp.clicked()
        });
        if clicked {
            chat.retry();
        }
    }
}

fn paint_inset_spacers(ui: &Ui, rect: Rect, token: Space) {
    let p = token.pts();
    if p <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    Spacer::paint_at(ui, token, Rect::from_min_size(rect.min, vec2(p, rect.height())));
    Spacer::paint_at(ui, token, Rect::from_min_max(pos2(rect.max.x - p, rect.min.y), rect.max));
    Spacer::paint_at(ui, token, Rect::from_min_size(rect.min, vec2(rect.width(), p)));
    Spacer::paint_at(ui, token, Rect::from_min_max(pos2(rect.min.x, rect.max.y - p), rect.max));
}

fn fit_line(
    ui: &Ui, text: String, font: FontId, color: egui::Color32, max_w: f32,
) -> std::sync::Arc<egui::Galley> {
    let full = ui
        .painter()
        .layout_no_wrap(text.clone(), font.clone(), color);
    if full.size().x <= max_w {
        return full;
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return full;
    }
    let mut lo = 1usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let candidate: String = chars[..mid].iter().chain(['…'].iter()).collect();
        let g = ui.painter().layout_no_wrap(candidate, font.clone(), color);
        if g.size().x <= max_w {
            lo = mid;
        } else {
            hi = mid.saturating_sub(1);
        }
    }
    let n = lo.max(1).min(chars.len());
    let candidate: String = chars[..n].iter().chain(['…'].iter()).collect();
    ui.painter().layout_no_wrap(candidate, font, color)
}

fn kind_tag(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::User => "user",
        ItemKind::Assistant => "asst",
        ItemKind::Thought => "thought",
        ItemKind::Tool => "tool",
        ItemKind::Plan => "plan",
        ItemKind::Ask => "ask",
        ItemKind::Error => "error",
    }
}

fn status_tag(status: Status) -> &'static str {
    match status {
        Status::Open => "open",
        Status::Pending => "pending",
        Status::Running => "run",
        Status::Done => "done",
        Status::Failed => "fail",
        Status::Cancelled => "cancel",
    }
}

fn space_tag(space: Space) -> &'static str {
    match space {
        Space::Xxs => "xxs",
        Space::Xs => "xs",
        Space::Sm => "sm",
        Space::Md => "md",
        Space::Lg => "lg",
        Space::Xl => "xl",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_rs::model::chat::{Content, ItemMeta};

    fn item(kind: ItemKind, status: Status, text: &str) -> Item {
        Item {
            id: Uuid::new_v4(),
            parent: None,
            from: "x".into(),
            kind,
            status,
            blocks: if text.is_empty() {
                Vec::new()
            } else {
                vec![Content::Text { text: text.into() }]
            },
            meta: ItemMeta::default(),
            usage: None,
        }
    }

    fn visible(items: &[Item]) -> Vec<ItemKind> {
        items
            .iter()
            .enumerate()
            .filter(|(i, _)| !skip_item(items, *i))
            .map(|(_, it)| it.kind)
            .collect()
    }

    fn gaps(items: &[Item]) -> Vec<Space> {
        let kinds = visible(items);
        kinds.windows(2).map(|w| gap_before(w[0], w[1])).collect()
    }

    #[test]
    fn skip_empty_assistants_collapses_tool_batches() {
        let items = vec![
            item(ItemKind::User, Status::Done, "hi"),
            item(ItemKind::Assistant, Status::Done, ""),
            item(ItemKind::Tool, Status::Done, "a"),
            item(ItemKind::Tool, Status::Done, "b"),
            item(ItemKind::Assistant, Status::Done, ""),
            item(ItemKind::Tool, Status::Done, "c"),
            item(ItemKind::Assistant, Status::Done, "answer"),
        ];
        assert_eq!(
            visible(&items),
            vec![
                ItemKind::User,
                ItemKind::Tool,
                ItemKind::Tool,
                ItemKind::Tool,
                ItemKind::Assistant,
            ]
        );
        assert_eq!(gaps(&items), vec![Space::Md, Space::Xs, Space::Xs, Space::Md]);
    }

    #[test]
    fn keep_thinking_placeholder() {
        let items = vec![
            item(ItemKind::User, Status::Done, "hi"),
            item(ItemKind::Assistant, Status::Running, ""),
        ];
        assert_eq!(visible(&items), vec![ItemKind::User, ItemKind::Assistant]);
    }

    #[test]
    fn tools_paint_above_their_assistant() {
        let mut asst = item(ItemKind::Assistant, Status::Running, "hello");
        let id = asst.id;
        asst.status = Status::Running;
        let mut t1 = item(ItemKind::Tool, Status::Done, "web");
        t1.parent = Some(id);
        let mut t2 = item(ItemKind::Tool, Status::Done, "web2");
        t2.parent = Some(id);
        let items = vec![item(ItemKind::User, Status::Done, "hi"), asst, t1, t2];
        let kinds: Vec<_> = display_indices(&items)
            .into_iter()
            .map(|i| items[i].kind)
            .collect();
        assert_eq!(
            kinds,
            vec![ItemKind::User, ItemKind::Tool, ItemKind::Tool, ItemKind::Assistant]
        );
    }
}
