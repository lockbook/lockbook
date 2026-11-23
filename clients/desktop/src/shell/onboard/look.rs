//! Step 3 (Create-only experiment) — Choose a look (Layout 4c).
//!
//! Centered column on `t.neutral_bg()`:
//! - Live Markdown preview rendered via `MdLabel` inside `plate_content_shadow`
//!   using true `t.neutral_bg()` (never a raised surface) and `overlay_shadow(t)`.
//! - Two focusable rows (`Accent` swatches and `Font` pills): `Tab`/`⇧Tab` and
//!   `↑`/`↓` switch rows; `←`/`→` cycle the active row.
//! - `Back (Esc)` + `Continue (↩)` with no loading spinner.

use std::sync::Arc;

use egui::{
    Align2, Color32, FontFamily, FontId, Id, Key, Modifiers, Rect, Stroke, StrokeKind, Ui, pos2,
    vec2,
};
use workspace_rs::font::slot::Slot;
use workspace_rs::style::swatches::swatches_with_hover;
use workspace_rs::tab::markdown_editor::MdLabel;
use workspace_rs::theme::palette_v2::Palette;
use workspace_rs::widgets::GlyphonLabel;

use crate::components::{
    FG_HOVER, FG_PRESS, Radius, STROKE_HAIRLINE, SheetFooterOpts, Space, Spacer, Theme, TypeRole,
    control_height, overlay_shadow, plate_content_shadow, sense_click, sheet_footer,
    shortcut_enter, ui_width, with_pad_fit,
};
use crate::shell::ShellApp;
use crate::shell::action::{Action as A, OnboardStep};

use super::{centered_column_with_height, column_width, measure_text_h};

const COL_W: f32 = 590.0;
const LABEL_W: f32 = 64.0;
const TITLE: &str = "Choose a look";
const SUBTITLE: &str =
    "Pick an accent color and font for your notes. You can change both anytime in Settings.";

const HUES: [Palette; 6] =
    [Palette::Red, Palette::Yellow, Palette::Green, Palette::Cyan, Palette::Blue, Palette::Magenta];

const SPECIMEN_MD: &str = "\
---
tags: [example, travel]
---
# ⛩️ Kyoto Planning
Subway day pass is **¥800** and pays for itself at *four rides*. Transfer at **Karasuma Oike** and beat the 8am temple rush.

- [x] Flights
- [ ] Ryokan near Gion — the one with the garden
- [ ] Tea ceremony class (*morning slots go fast*)

> Don't overpack — see [packing list](https://lockbook.net).";

pub(super) fn show(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<A>) {
    let (busy, err, cur_preview_accent, cur_preview_font, mut focus_row) = match &app.onboard {
        Some(s) => (
            s.busy,
            s.err.clone(),
            s.preview_accent,
            s.preview_font.clone(),
            s.look_focus_row.min(1),
        ),
        None => return,
    };

    let slots: Vec<Slot> = Slot::shown().collect();
    let chosen_slot = slot_of(&slots, app.settings.text_font.as_deref());
    let saved_accent = app
        .settings
        .theme_accent
        .unwrap_or_else(|| app.settings.theme_family.build(t.current).accent_hue);
    let mut hue = HUES.iter().position(|&p| p == saved_accent);
    let font_idx = slots.iter().position(|&s| Some(s) == chosen_slot);

    let (toggle_row, up, down, lr_step) = ui.input_mut(|i| {
        let toggle =
            i.consume_key(Modifiers::SHIFT, Key::Tab) || i.consume_key(Modifiers::NONE, Key::Tab);
        let u = i.consume_key(Modifiers::NONE, Key::ArrowUp);
        let d = i.consume_key(Modifiers::NONE, Key::ArrowDown);
        let lr = i.consume_key(Modifiers::NONE, Key::ArrowRight) as i32
            - i.consume_key(Modifiers::NONE, Key::ArrowLeft) as i32;
        (toggle, u, d, lr)
    });
    if toggle_row {
        focus_row = 1 - focus_row;
    } else if up {
        focus_row = 0;
    } else if down {
        focus_row = 1;
    }

    let mut accent_committed = false;
    let mut font_committed = false;
    if lr_step != 0 {
        if focus_row == 0 {
            queue.push(A::SetThemeAccent(HUES[cycle(hue, lr_step, HUES.len())]));
            accent_committed = true;
        } else if !slots.is_empty() {
            let slot = slots[cycle(font_idx, lr_step, slots.len())];
            queue.push(A::SetTextFont(slot_choice(slot)));
            font_committed = true;
        }
    }

    // Ahead-of-time layout: measure the specimen card for the TALLEST among all
    // shown font slots before positioning the column so Frame 1 lands at its
    // exact final y AND hovering across fonts (including Mono) never shifts the
    // Accent/Font controls underneath the pointer.
    let w = column_width(ui, COL_W);
    let row_h = control_height();
    let card_pad = Space::Lg.pts();
    let specimen_w = (w - card_pad * 2.0).max(1.0);
    let dark_mode = ui.style().visuals.dark_mode;
    let raw_specimen_h = {
        let label = app
            .onboard_look_label
            .get_or_insert_with(|| MdLabel::new(ui.ctx().clone()));
        label.renderer.dark_mode = dark_mode;
        label.height(SPECIMEN_MD, specimen_w)
    };
    let max_font_specimen_h = measure_max_specimen_height(ui, specimen_w, dark_mode, &slots);
    let specimen_slot_h = raw_specimen_h.max(max_font_specimen_h);

    let prepared_pills: Vec<PreparedPill> = slots
        .iter()
        .map(|&slot| prepare_font_pill(ui, t, slot))
        .collect();
    let pills_avail_w = (w - LABEL_W).max(1.0);
    let font_row_h = wrapped_pills_height(
        &prepared_pills,
        pills_avail_w,
        Space::Sm.pts(),
        Space::Xs.pts(),
        row_h,
    );
    let err_h = err
        .as_deref()
        .map_or(0.0, |e| Space::Sm.pts() + measure_text_h(ui, TypeRole::Body.rich(e), w));
    let total_h = measure_text_h(ui, TypeRole::Title.rich(TITLE).strong(), w)
        + Space::Xs.pts()
        + measure_text_h(ui, TypeRole::Body.rich(SUBTITLE), w)
        + Space::Lg.pts()
        + (specimen_slot_h + card_pad * 2.0)
        + Space::Lg.pts()
        + row_h
        + Space::Sm.pts()
        + font_row_h
        + err_h
        + Space::Lg.pts()
        + row_h;

    let mut hovered_swatch: Option<usize> = None;
    let mut hovered_font: Option<Slot> = None;

    centered_column_with_height(ui, COL_W, total_h, |ui| {
        ui.label(TypeRole::Title.rich(TITLE).strong().color(t.neutral_fg()));
        ui.add(Spacer::new(Space::Xs));
        ui.label(
            TypeRole::Body
                .rich(SUBTITLE)
                .color(t.neutral_fg_secondary()),
        );
        ui.add(Spacer::new(Space::Lg));

        // Live Markdown specimen card: true neutral_bg() on neutral_bg() with overlay_shadow.
        let card_w = ui_width(ui).max(1.0);
        plate_content_shadow(
            ui,
            t.neutral_bg(),
            overlay_shadow(t),
            Radius::Surface.corner(),
            |ui| {
                ui.set_width(card_w);
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                with_pad_fit(ui, Space::Lg, |ui| {
                    ui.set_min_height(specimen_slot_h);
                    paint_specimen(app, ui);
                });
            },
        );

        ui.add(Spacer::new(Space::Lg));

        // Accent row (hover previews live; click commits and focuses row).
        ui.horizontal(|ui| {
            ui.set_min_height(row_h);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            paint_row_label(ui, t, "Accent", row_h, focus_row == 0);
            let colors = HUES.map(|p| t.fg().get_color(p));
            let (resp, hovered) = swatches_with_hover(ui, t, &colors, &mut hue);
            hovered_swatch = hovered;
            if resp.changed() {
                focus_row = 0;
                if let Some(i) = hue {
                    queue.push(A::SetThemeAccent(HUES[i]));
                    accent_committed = true;
                }
            }
        });

        ui.add(Spacer::new(Space::Sm));

        // Font row (hover previews live; click commits and focuses row).
        // Placed deterministically using the same wrapped flow used for ahead-of-time
        // height measurement so Frame 1 matches to the subpixel.
        ui.horizontal(|ui| {
            ui.set_min_height(font_row_h);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            paint_row_label(ui, t, "Font", row_h, focus_row == 1);
            let (pills_rect, _) =
                ui.allocate_exact_size(vec2(pills_avail_w, font_row_h), egui::Sense::hover());
            let gap_x = Space::Sm.pts();
            let gap_y = Space::Xs.pts();
            let mut x = 0.0_f32;
            let mut y = 0.0_f32;
            for pill in prepared_pills {
                if x > 0.0 && x + gap_x + pill.width > pills_avail_w + 0.5 {
                    x = 0.0;
                    y += row_h + gap_y;
                } else if x > 0.0 {
                    x += gap_x;
                }
                let slot = pill.slot;
                let pill_rect = Rect::from_min_size(
                    pos2(pills_rect.left() + x, pills_rect.top() + y),
                    vec2(pill.width, row_h),
                );
                let pill_w = pill.width;
                let (clicked, hovered) =
                    paint_font_pill_at(ui, t, pill_rect, pill, chosen_slot == Some(slot));
                if hovered {
                    hovered_font = Some(slot);
                }
                if clicked {
                    focus_row = 1;
                    queue.push(A::SetTextFont(slot_choice(slot)));
                    font_committed = true;
                }
                x += pill_w;
            }
        });

        if let Some(e) = &err {
            ui.add(Spacer::new(Space::Sm));
            ui.label(TypeRole::Body.rich(e).color(t.danger()));
        }

        ui.add(Spacer::new(Space::Lg));
        let foot = sheet_footer(
            ui,
            t,
            if busy { "Creating…" } else { "Continue" },
            SheetFooterOpts::default()
                .divider(false)
                .cancel_label("Back")
                .primary_enabled(!busy)
                .primary_shortcut(shortcut_enter()),
        );
        if foot.cancel && !busy {
            queue.push(A::OnboardSetStep(OnboardStep::Create));
        }
        if foot.primary {
            queue.push(A::OnboardFinishLook);
        }
    });

    if let Some(s) = &mut app.onboard {
        s.look_focus_row = focus_row;
    }

    // Reconcile hover previews without writing to Settings until clicked.
    // Skip while `busy` so a resting cursor over a swatch/pill cannot re-apply
    // an unsaved preview after `onboard_finish_look` restores saved values.
    if !busy {
        if !accent_committed {
            let want = hovered_swatch
                .map(|i| HUES[i])
                .filter(|&h| h != saved_accent);
            if want != cur_preview_accent {
                queue.push(A::PreviewThemeAccent(want));
            }
        }
        if !font_committed {
            let want = hovered_font
                .filter(|&s| Some(s) != chosen_slot)
                .map(slot_choice);
            if want != cur_preview_font {
                queue.push(A::PreviewTextFont(want));
            }
        }
    }
}

/// Measure `SPECIMEN_MD` across every shown font slot and return the tallest
/// height, cached per `(width, dark_mode, families)`. Because the card always
/// reserves this height, hovering any font (e.g. a wider monospace face that
/// wraps onto an extra line) never pushes the pills down out from under the cursor.
fn measure_max_specimen_height(ui: &Ui, width: f32, dark_mode: bool, slots: &[Slot]) -> f32 {
    #[derive(Clone, PartialEq, Eq)]
    struct CacheKey {
        width_bits: u32,
        dark_mode: bool,
        families: Vec<&'static str>,
    }

    let families: Vec<&'static str> = slots.iter().map(|s| s.drawn_family()).collect();
    let key = CacheKey { width_bits: width.to_bits(), dark_mode, families };
    let cache_id = Id::unique("onboard_look_max_specimen_h");
    if let Some((cached_key, cached_h)) = ui.ctx().data(|d| d.get_temp::<(CacheKey, f32)>(cache_id))
    {
        if cached_key == key {
            return cached_h;
        }
    }

    let mut measurer = MdLabel::new(ui.ctx().clone());
    measurer.renderer.dark_mode = dark_mode;
    let mut max_h = 0.0_f32;
    let mut all_loaded = true;
    for &family in &key.families {
        if let Some(h) = measurer.height_in_family(SPECIMEN_MD, width, family) {
            max_h = max_h.max(h);
        } else {
            all_loaded = false;
        }
    }
    if all_loaded || !workspace_rs::font::loader::busy() {
        ui.ctx().data_mut(|d| d.insert_temp(cache_id, (key, max_h)));
    }
    max_h
}

fn paint_specimen(app: &mut ShellApp, ui: &mut Ui) {
    let width = ui_width(ui).max(1.0);
    let label = app
        .onboard_look_label
        .get_or_insert_with(|| MdLabel::new(ui.ctx().clone()));
    let top_left = ui.next_widget_position();
    let (areas, rect) = label.paint_at(ui, "onboard_look_specimen", SPECIMEN_MD, top_left, width);
    ui.advance_cursor_after_rect(rect);

    let visible = rect.intersect(ui.clip_rect());
    if !areas.is_empty() && visible.is_positive() {
        ui.painter()
            .add(egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                visible,
                workspace_rs::GlyphonRendererCallback::new(areas),
            ));
    }
}

fn paint_row_label(ui: &mut Ui, t: &Theme, text: &str, h: f32, focused: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(LABEL_W, h), egui::Sense::hover());
    let (font, color) = if focused {
        (FontId::new(TypeRole::Body.size(), FontFamily::Name(Arc::from("Bold"))), t.neutral_fg())
    } else {
        (TypeRole::Body.font_id(), t.neutral_fg_secondary())
    };
    ui.painter()
        .text(pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, text, font, color);
}

struct PreparedPill {
    slot: Slot,
    name: &'static str,
    shaped: Option<workspace_rs::widgets::glyphon_label::ShapedLabel>,
    width: f32,
}

fn prepare_font_pill(ui: &Ui, t: &Theme, slot: Slot) -> PreparedPill {
    let family = slot.drawn_family();
    let name = workspace_rs::font::system::shown_name(family);
    let pad_x = Space::Sm.pts();
    let ink = t.neutral_fg();
    let shaped = workspace_rs::font::load_family(ui.ctx(), family).map(|adjust| {
        let size = TypeRole::Body.size() * adjust;
        GlyphonLabel::new(name, ink)
            .family(family)
            .font_size(size)
            .line_height(TypeRole::Body.line_height().max(size * 1.4))
            .max_lines(1)
            .build(ui.ctx())
    });
    let text_w = match &shaped {
        Some(s) => s.size.x,
        None => {
            ui.painter()
                .layout_no_wrap(name.to_owned(), TypeRole::Body.font_id(), ink)
                .size()
                .x
        }
    };
    let width = (text_w.ceil() + pad_x * 2.0).max(52.0);
    PreparedPill { slot, name, shaped, width }
}

fn wrapped_pills_height(
    pills: &[PreparedPill], avail_w: f32, gap_x: f32, gap_y: f32, row_h: f32,
) -> f32 {
    if pills.is_empty() {
        return row_h;
    }
    let mut rows = 1usize;
    let mut x = 0.0_f32;
    for pill in pills {
        let next = if x == 0.0 { pill.width } else { x + gap_x + pill.width };
        if x > 0.0 && next > avail_w + 0.5 {
            rows += 1;
            x = pill.width;
        } else {
            x = next;
        }
    }
    rows as f32 * row_h + (rows - 1) as f32 * gap_y
}

fn paint_font_pill_at(
    ui: &mut Ui, t: &Theme, rect: Rect, pill: PreparedPill, selected: bool,
) -> (bool, bool) {
    let ink = t.neutral_fg();
    let resp =
        ui.interact(rect, ui.scope_id().with(("onboard_font_pill", pill.name)), sense_click());

    let over = resp.hovered() || ui.ctx().rect_contains_pointer(ui.layer_id(), rect);
    let base_fill = if selected { t.neutral_bg_secondary() } else { t.neutral_bg() };
    let fill = if over {
        let wash = if resp.is_pointer_button_down_on() { FG_PRESS } else { FG_HOVER };
        t.wash_toward_neutral_fg(base_fill, wash)
    } else {
        base_fill
    };
    let (stroke_color, stroke_w) =
        if selected { (t.accent(), 1.5) } else { (t.neutral(), STROKE_HAIRLINE) };

    ui.painter().rect(
        rect,
        Radius::Control.corner(),
        fill,
        Stroke::new(stroke_w, stroke_color),
        StrokeKind::Inside,
    );

    // Align every font face to the same typographic baseline (matching Settings' font picker),
    // and center horizontally inside the pill using its own measured width.
    let body =
        ui.painter()
            .layout_no_wrap("Hg".into(), TypeRole::Body.font_id(), Color32::PLACEHOLDER);
    let body_baseline = body
        .rows
        .first()
        .map_or(0.0, |r| r.pos.y + r.row.glyphs.first().map_or(0.0, |gl| gl.pos.y));
    let target_baseline = rect.center().y - body.size().y / 2.0 + body_baseline;

    match pill.shaped {
        Some(shaped) => {
            let x = rect.center().x - shaped.size.x / 2.0;
            let top = target_baseline - shaped.baseline(ui.ctx()).unwrap_or(0.0);
            let at = Rect::from_min_size(pos2(x, top), shaped.size);
            let clip = rect.intersect(ui.clip_rect());
            let area = shaped.text_area(at, ui.ctx(), clip);
            workspace_rs::paint_text(ui.painter(), clip, vec![area]);
        }
        None => {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                pill.name,
                TypeRole::Body.font_id(),
                ink,
            );
        }
    }

    (resp.clicked(), over)
}

fn cycle(at: Option<usize>, step: i32, len: usize) -> usize {
    match at {
        Some(i) => (i as i32 + step).rem_euclid(len as i32) as usize,
        None if step > 0 => 0,
        None => len - 1,
    }
}

fn slot_choice(slot: Slot) -> Option<String> {
    if slot == Slot::platform() { None } else { Some(slot.drawn_family().to_owned()) }
}

fn slot_of(slots: &[Slot], chosen: Option<&str>) -> Option<Slot> {
    match chosen {
        None => Some(Slot::platform()),
        Some(name) => slots.iter().copied().find(|s| s.drawn_family() == name),
    }
}
