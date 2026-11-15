use std::sync::Arc;

use egui::{Color32, FontId, Galley, Rect, Sense, Stroke, Ui, pos2, vec2};
use lb::model::api::FREE_TIER_USAGE_SIZE;
use lb::model::usage::bytes_to_human;
use lb::service::usage::UsageMetrics;
use workspace_rs::theme::palette_v2::Palette;

use crate::components::{
    Button, Space, Spacer, Theme, TypeRole, footnote, form_group, form_row_detail, ui_width,
};

use super::ShellApp;
use super::action::Action;
use super::action::Action as A;

const BAR_H: f32 = 12.0;

struct Inks {
    synced: Color32,
    added: Color32,
    freed: Color32,
    over: Color32,
    track: Color32,
}

impl Inks {
    fn new(t: &Theme) -> Self {
        Self {
            synced: t.accent(),
            added: t.fg().get_color(Palette::Yellow),
            freed: t.accent().gamma_multiply(0.3),
            over: t.danger(),
            track: t.neutral_bg_tertiary(),
        }
    }
}

pub(crate) fn page_storage(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    let Some(ready) = app.session.ready() else {
        footnote(ui, t, "Sign in to see what your account stores.");
        return;
    };
    let usage = ready.status.usage.clone();
    let out_of_space = ready.status.out_of_space;
    let inks = Inks::new(t);

    summary(ui, t, &inks, &usage);
    callout(ui, t, &usage, out_of_space, queue);
}

fn summary(ui: &mut Ui, t: &Theme, inks: &Inks, usage: &UsageMetrics) {
    form_group(ui, t, |ui| {
        let w = ui_width(ui);
        let pad = Space::Md.pts();

        let (head, _) =
            ui.allocate_exact_size(vec2(w, TypeRole::Title.size() * 1.5), Sense::hover());
        let big = text(ui, &usage.local.readable, TypeRole::Title.font_id(), t.neutral_fg());
        let big_w = big.size().x;
        let big_pos = pos2(head.left() + pad, head.center().y - big.size().y / 2.0);
        ui.painter().galley(big_pos, big, t.neutral_fg());
        let (of, side, side_ink) = match &usage.data_cap {
            Some(cap) => {
                let side = if usage.over_cap() {
                    format!("{} over", bytes_to_human(usage.local.exact - cap.exact))
                } else {
                    format!("{} free", bytes_to_human(cap.exact - usage.local.exact))
                };
                let ink = if usage.over_cap() { inks.over } else { t.neutral_fg_secondary() };
                (format!("of {}", cap.readable), side, ink)
            }
            None => ("used".into(), "Cap not loaded".into(), t.neutral_fg_secondary()),
        };
        let of = text(ui, &of, TypeRole::Body.font_id(), t.neutral_fg_secondary());
        let of_pos = pos2(
            big_pos.x + big_w + Space::Xs.pts(),
            big_pos.y + TypeRole::Title.size() - TypeRole::Body.size() - 1.0,
        );
        ui.painter().galley(of_pos, of, t.neutral_fg_secondary());
        let side = text(ui, &side, TypeRole::Body.font_id(), side_ink);
        let side_pos = pos2(head.right() - pad - side.size().x, of_pos.y);
        ui.painter().galley(side_pos, side, side_ink);

        ui.add(Spacer::new(Space::Sm));
        let (band, _) = ui.allocate_exact_size(vec2(w, BAR_H), Sense::hover());
        let track = Rect::from_min_max(
            pos2(band.left() + pad, band.top()),
            pos2(band.right() - pad, band.bottom()),
        );
        paint_bar(ui, t, inks, track, usage);

        ui.add(Spacer::new(Space::Sm));
        legend(ui, t, inks, usage, pad);
    });
}

fn paint_bar(ui: &Ui, t: &Theme, inks: &Inks, track: Rect, usage: &UsageMetrics) {
    let synced = usage.synced.exact as f32;
    let local = usage.local.exact as f32;
    let added = usage.added.exact as f32;
    let freed = usage.freed.exact as f32;
    let cap = usage.data_cap.as_ref().map(|cap| cap.exact as f32);
    let scale = cap.unwrap_or(0.0).max(synced + added).max(1.0);
    let x = |v: f32| track.left() + track.width() * (v / scale).clamp(0.0, 1.0);
    let r = track.height() / 2.0;

    let painter = ui.painter();
    painter.rect_filled(track, r, inks.track);

    let kept = local - added;
    let drawn = Rect::from_min_max(track.min, pos2(x(local + freed), track.bottom()));
    let segment = |from: f32, to: f32, ink: Color32| {
        if to > from {
            let clip = Rect::from_min_max(pos2(x(from), track.top()), pos2(x(to), track.bottom()));
            painter.with_clip_rect(clip).rect_filled(drawn, r, ink);
        }
    };
    segment(0.0, kept, inks.synced);
    segment(kept, local, inks.added);
    segment(local, local + freed, inks.freed);
    if let Some(cap) = cap {
        segment(cap, local, inks.over);
        if scale > cap {
            let cx = x(cap);
            painter.vline(
                cx,
                (track.top() - 3.0)..=(track.bottom() + 3.0),
                Stroke::new(1.5, t.neutral_fg()),
            );
        }
    }
}

fn legend(ui: &mut Ui, t: &Theme, inks: &Inks, usage: &UsageMetrics, pad: f32) {
    let mut items: Vec<(Color32, &str, String)> =
        vec![(inks.synced, "Synced", usage.synced.readable.clone())];
    if usage.added.exact > 0 {
        items.push((inks.added, "Unsynced", format!("+{}", usage.added.readable)));
    }
    if usage.freed.exact > 0 {
        items.push((inks.freed, "Freed on sync", format!("−{}", usage.freed.readable)));
    }
    if let Some(cap) = &usage.data_cap {
        if usage.over_cap() {
            items.push((inks.over, "Over cap", bytes_to_human(usage.local.exact - cap.exact)));
        }
    }

    let swatch = 9.0;
    let gap = Space::Xs.pts();
    let w = ui_width(ui);
    let line = TypeRole::Body.line_height();
    let laid: Vec<_> = items
        .into_iter()
        .map(|(ink, label, value)| {
            let label = text(ui, label, TypeRole::Body.font_id(), t.neutral_fg_secondary());
            let value = text(ui, &value, TypeRole::Body.font_id(), t.neutral_fg());
            let width = swatch + gap + label.size().x + gap + value.size().x;
            (ink, label, value, width)
        })
        .collect();

    let mut places = vec![];
    let (mut x, mut y) = (pad, 0.0);
    for (_, _, _, width) in &laid {
        if x > pad && x + width > w - pad {
            x = pad;
            y += line + gap;
        }
        places.push(pos2(x, y));
        x += width + Space::Md.pts();
    }
    let (rect, _) = ui.allocate_exact_size(vec2(w, y + line), Sense::hover());

    for ((ink, label, value, _), at) in laid.into_iter().zip(places) {
        let center_y = rect.top() + at.y + line / 2.0;
        let mut cursor = rect.left() + at.x;
        let dot =
            Rect::from_center_size(pos2(cursor + swatch / 2.0, center_y), vec2(swatch, swatch));
        ui.painter().rect_filled(dot, 2.5, ink);
        cursor += swatch + gap;
        let label_w = label.size().x;
        ui.painter().galley(
            pos2(cursor, center_y - label.size().y / 2.0),
            label,
            t.neutral_fg_secondary(),
        );
        cursor += label_w + gap;
        ui.painter()
            .galley(pos2(cursor, center_y - value.size().y / 2.0), value, t.neutral_fg());
    }
}

fn callout(
    ui: &mut Ui, t: &Theme, usage: &UsageMetrics, out_of_space: bool, queue: &mut Vec<Action>,
) {
    let Some(cap) = &usage.data_cap else {
        ui.add(Spacer::new(Space::Sm));
        footnote(
            ui,
            t,
            "Usage is counted on this device. Your cap shows once Lockbook reaches the server.",
        );
        return;
    };
    let free_tier = cap.exact <= FREE_TIER_USAGE_SIZE;
    let frac = usage.local.exact as f32 / cap.exact.max(1) as f32;
    let message: Option<(&str, String)> = if usage.over_cap() {
        Some((
            "Over your data cap",
            format!(
                "Changes that add space won't sync until you free {}{}.",
                bytes_to_human(usage.local.exact - cap.exact),
                if free_tier { " or upgrade" } else { "" }
            ),
        ))
    } else if out_of_space {
        Some((
            "A sync ran out of space",
            "Your storage has room, so a folder someone shared with you may be full.".to_string(),
        ))
    } else if frac >= 0.8 {
        Some((
            "Almost full",
            format!(
                "{} left before changes stop syncing.",
                bytes_to_human(cap.exact - usage.local.exact)
            ),
        ))
    } else {
        None
    };
    let Some((title, detail)) = message else { return };

    ui.add(Spacer::new(Space::Md));
    form_group(ui, t, |ui| {
        form_row_detail(ui, t, title, &detail, |ui| {
            if free_tier && Button::primary(t, "Upgrade").show(ui).clicked() {
                queue.push(A::OpenUpgrade);
            }
        });
    });
}

fn text(ui: &Ui, text: &str, font: FontId, color: Color32) -> Arc<Galley> {
    ui.painter().layout_no_wrap(text.to_owned(), font, color)
}
