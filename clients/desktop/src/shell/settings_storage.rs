use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Align2, Color32, FontId, Galley, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use lb::Uuid;
use lb::model::api::FREE_TIER_USAGE_SIZE;
use lb::model::usage::bytes_to_human;
use lb::service::usage::UsageMetrics;
use workspace_rs::theme::palette_v2::Palette;

use crate::components::interact::interact_fill_response;
use crate::components::interact::quiet_canvas_fills;
use crate::components::{
    Button, Radius, Space, Spacer, Theme, TypeRole, control_height, display_file_name,
    file_row_icon, footnote, form_group, form_row_detail, phosphor, phosphor_ui_font_id,
    section_label, segmented, sense_click, ui_width,
};

use super::ShellApp;
use super::action::Action;
use super::action::Action as A;
use super::storage_report::{Change, Kind, Row, Size, StorageReport};

#[derive(Default)]
pub struct StorageView {
    pub tab: usize,
    pub folder: Option<Uuid>,
    cache: Option<(UsageMetrics, u64, Arc<StorageReport>)>,
}

const TABS: [&str; 4] = ["Folders", "Types", "Largest", "Unsynced"];
const MAX_ROWS: usize = 60;
const BAR_H: f32 = 12.0;
const ROW_BAR_W: f32 = 84.0;
const ROW_BAR_H: f32 = 6.0;
const VALUE_W: f32 = 64.0;
const DELTA_W: f32 = 64.0;
const ICON_W: f32 = 18.0;

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
    let epoch = ready.files_epoch;

    let stale = match &app.storage.cache {
        Some((cached, cached_epoch, _)) => *cached != usage || *cached_epoch != epoch,
        None => true,
    };
    if stale {
        let entries = ready
            .workspace
            .core
            .get_usage_breakdown()
            .unwrap_or_default();
        let files = ready.workspace.files.read().unwrap();
        let report = StorageReport::new(files.root.id, files.all_files(), &entries);
        app.storage.cache = Some((usage.clone(), epoch, Arc::new(report)));
    }
    let report = app.storage.cache.as_ref().unwrap().2.clone();
    let inks = Inks::new(t);

    summary(ui, t, &inks, &usage);
    callout(ui, t, &usage, out_of_space, queue);

    ui.add(Spacer::new(Space::Lg));
    section_label(ui, t, "Breakdown");
    segmented(ui, t, &TABS, &mut app.storage.tab);
    ui.add(Spacer::new(Space::Sm));
    form_group(ui, t, |ui| match app.storage.tab {
        0 => folders(ui, t, &inks, &report, &mut app.storage.folder, queue),
        1 => kinds(ui, t, &inks, &report),
        2 => largest(ui, t, &inks, &report, queue),
        _ => changes(ui, t, &inks, &report),
    });
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

fn folders(
    ui: &mut Ui, t: &Theme, inks: &Inks, report: &StorageReport, folder: &mut Option<Uuid>,
    queue: &mut Vec<Action>,
) {
    let focus = folder
        .filter(|id| report.contains_folder(*id))
        .unwrap_or(report.root);
    let rows = report.children(focus);
    let changed = rows.iter().any(|row| row.size.delta() != 0);

    if focus != report.root {
        let trail = report.trail(focus);
        let path = trail
            .iter()
            .skip(1)
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        let parent = trail
            .iter()
            .rev()
            .nth(1)
            .map(|(id, _)| *id)
            .unwrap_or(report.root);
        let size = report.row(focus).map(|row| row.size).unwrap_or_default();
        let resp = usage_row(
            ui,
            t,
            RowSpec {
                icon: phosphor::ARROW_LEFT,
                title: &path,
                detail: Some("Back to the folder above".into()),
                frac: None,
                bar: inks.synced,
                value: bytes_to_human(size.local),
                delta: delta(t, inks, size),
                changed,
                clickable: true,
            },
        );
        if resp.clicked() {
            *folder = Some(parent);
        }
    }

    if rows.is_empty() {
        empty(ui, t, "Nothing stored here yet.");
        return;
    }
    let max = rows
        .iter()
        .map(|row| row.size.local)
        .max()
        .unwrap_or(1)
        .max(1);
    for row in rows.iter().take(MAX_ROWS) {
        let detail = row.is_folder.then(|| items(row.count.saturating_sub(1)));
        let resp = file_row(ui, t, inks, row, detail, max, changed);
        if resp.clicked() {
            if row.is_folder {
                *folder = Some(row.id);
            } else {
                queue.push(A::CloseModal);
                queue.push(A::OpenFile(row.id));
            }
        }
    }
    more(ui, t, rows.len());
}

fn kinds(ui: &mut Ui, t: &Theme, inks: &Inks, report: &StorageReport) {
    if report.kinds.is_empty() {
        empty(ui, t, "Nothing stored yet.");
        return;
    }
    let max = report
        .kinds
        .iter()
        .map(|row| row.size.local)
        .max()
        .unwrap_or(1)
        .max(1);
    let changed = report.kinds.iter().any(|row| row.size.delta() != 0);
    for row in &report.kinds {
        let noun = if row.kind == Kind::Folders { "item" } else { "file" };
        usage_row(
            ui,
            t,
            RowSpec {
                icon: kind_icon(row.kind),
                title: row.kind.label(),
                detail: Some(plural(row.count, noun)),
                frac: Some(row.size.local as f32 / max as f32),
                bar: inks.synced,
                value: bytes_to_human(row.size.local),
                delta: delta(t, inks, row.size),
                changed,
                clickable: false,
            },
        );
    }
}

fn largest(ui: &mut Ui, t: &Theme, inks: &Inks, report: &StorageReport, queue: &mut Vec<Action>) {
    if report.largest.is_empty() {
        empty(ui, t, "No documents yet.");
        return;
    }
    let max = report.largest[0].size.local.max(1);
    let changed = report.largest.iter().any(|row| row.size.delta() != 0);
    for row in &report.largest {
        let detail = Some(format!("in {}", report.parent_path(row.id)));
        if file_row(ui, t, inks, row, detail, max, changed).clicked() {
            queue.push(A::CloseModal);
            queue.push(A::OpenFile(row.id));
        }
    }
}

fn changes(ui: &mut Ui, t: &Theme, inks: &Inks, report: &StorageReport) {
    if report.changes.is_empty() {
        empty(ui, t, "Everything is synced.");
        return;
    }
    let max = report
        .changes
        .iter()
        .map(|row| row.size.delta().unsigned_abs())
        .max()
        .unwrap_or(1)
        .max(1);
    for row in report.changes.iter().take(MAX_ROWS) {
        let grows = row.size.delta() > 0;
        let icon = match row.change {
            Change::Deleted => phosphor::TRASH,
            _ => file_row_icon(&row.name, row.is_folder),
        };
        let detail = match row.id {
            Some(id) if row.change != Change::Deleted => {
                format!("{} · in {}", row.change.label(), report.parent_path(id))
            }
            _ => "Space returns once the deletes sync".into(),
        };
        usage_row(
            ui,
            t,
            RowSpec {
                icon,
                title: display_file_name(&row.name),
                detail: Some(detail),
                frac: Some(row.size.delta().unsigned_abs() as f32 / max as f32),
                bar: if grows { inks.added } else { inks.freed },
                value: signed(row.size.delta()),
                delta: None,
                changed: false,
                clickable: false,
            },
        );
    }
    more(ui, t, report.changes.len());
}

fn file_row(
    ui: &mut Ui, t: &Theme, inks: &Inks, row: &Row, detail: Option<String>, max: u64, changed: bool,
) -> Response {
    usage_row(
        ui,
        t,
        RowSpec {
            icon: file_row_icon(&row.name, row.is_folder),
            title: display_file_name(&row.name),
            detail,
            frac: Some(row.size.local as f32 / max as f32),
            bar: inks.synced,
            value: bytes_to_human(row.size.local),
            delta: delta(t, inks, row.size),
            changed,
            clickable: true,
        },
    )
}

struct RowSpec<'a> {
    icon: &'static str,
    title: &'a str,
    detail: Option<String>,
    frac: Option<f32>,
    bar: Color32,
    value: String,
    delta: Option<(String, Color32)>,
    changed: bool,
    clickable: bool,
}

fn usage_row(ui: &mut Ui, t: &Theme, spec: RowSpec) -> Response {
    let w = ui_width(ui);
    let h = if spec.detail.is_some() {
        TypeRole::Body.line_height() + TypeRole::Mono.line_height() + Space::Sm.pts()
    } else {
        control_height()
    };
    let sense = if spec.clickable { sense_click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    if spec.clickable {
        let fill = interact_fill_response(ui.ctx(), &resp, quiet_canvas_fills(t));
        let wash = rect.shrink2(vec2(Space::Xs.pts(), 0.0));
        ui.painter()
            .rect_filled(wash, Radius::Control.corner(), fill);
    }

    let pad = Space::Md.pts();
    let gap = Space::Sm.pts();
    let mut right = rect.right() - pad;
    let delta_rect =
        Rect::from_min_max(pos2(right - DELTA_W, rect.top()), pos2(right, rect.bottom()));
    if spec.changed {
        right -= DELTA_W + gap;
    }
    let value_rect =
        Rect::from_min_max(pos2(right - VALUE_W, rect.top()), pos2(right, rect.bottom()));
    right -= VALUE_W + gap;
    let bar_rect = Rect::from_center_size(
        pos2(right - ROW_BAR_W / 2.0, rect.center().y),
        vec2(ROW_BAR_W, ROW_BAR_H),
    );
    right -= ROW_BAR_W + gap;

    let painter = ui.painter();
    let icon_x = rect.left() + pad + ICON_W / 2.0;
    painter.text(
        pos2(icon_x, rect.center().y),
        Align2::CENTER_CENTER,
        spec.icon,
        phosphor_ui_font_id(),
        t.neutral_fg_secondary(),
    );

    let text_left = rect.left() + pad + ICON_W + gap;
    let text_w = (right - text_left).max(1.0);
    let title = truncated(ui, spec.title, TypeRole::Body.font_id(), t.neutral_fg(), text_w);
    match &spec.detail {
        Some(detail) => {
            let detail =
                truncated(ui, detail, TypeRole::Mono.font_id(), t.neutral_fg_secondary(), text_w);
            let block = title.size().y + Space::Xxs.pts() + detail.size().y;
            let top = rect.center().y - block / 2.0;
            let title_h = title.size().y;
            painter.galley(pos2(text_left, top), title, t.neutral_fg());
            painter.galley(
                pos2(text_left, top + title_h + Space::Xxs.pts()),
                detail,
                t.neutral_fg_secondary(),
            );
        }
        None => {
            painter.galley(
                pos2(text_left, rect.center().y - title.size().y / 2.0),
                title,
                t.neutral_fg(),
            );
        }
    }

    if let Some(frac) = spec.frac {
        let r = ROW_BAR_H / 2.0;
        painter.rect_filled(bar_rect, r, t.neutral_bg_tertiary());
        let fill_w = (bar_rect.width() * frac.clamp(0.0, 1.0)).max(ROW_BAR_H);
        let fill = Rect::from_min_size(bar_rect.min, vec2(fill_w, ROW_BAR_H));
        painter.rect_filled(fill, r, spec.bar);
    }

    painter.text(
        pos2(value_rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        &spec.value,
        TypeRole::Body.font_id(),
        t.neutral_fg(),
    );
    if let Some((delta, ink)) = &spec.delta {
        painter.text(
            pos2(delta_rect.right(), rect.center().y),
            Align2::RIGHT_CENTER,
            delta,
            TypeRole::Mono.font_id(),
            *ink,
        );
    }

    resp
}

fn empty(ui: &mut Ui, t: &Theme, message: &str) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui_width(ui), control_height() * 2.0), Sense::hover());
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        message,
        TypeRole::Body.font_id(),
        t.neutral_fg_secondary(),
    );
}

fn more(ui: &mut Ui, t: &Theme, total: usize) {
    if total > MAX_ROWS {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui_width(ui), control_height()), Sense::hover());
        ui.painter().text(
            pos2(rect.left() + Space::Md.pts(), rect.center().y),
            Align2::LEFT_CENTER,
            format!("and {} more", total - MAX_ROWS),
            TypeRole::Mono.font_id(),
            t.neutral_fg_secondary(),
        );
    }
}

fn delta(t: &Theme, inks: &Inks, size: Size) -> Option<(String, Color32)> {
    match size.delta() {
        0 => None,
        d if d > 0 => Some((signed(d), inks.added)),
        d => Some((signed(d), t.neutral_fg_secondary())),
    }
}

fn signed(delta: i64) -> String {
    let human = bytes_to_human(delta.unsigned_abs());
    if delta >= 0 { format!("+{human}") } else { format!("−{human}") }
}

fn items(n: usize) -> String {
    plural(n, "item")
}

fn plural(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Notes => phosphor::MARKDOWN_LOGO,
        Kind::Drawings => phosphor::PENCIL,
        Kind::Images => phosphor::IMAGE_SQUARE,
        Kind::Pdfs => phosphor::FILE_PDF,
        Kind::Text => phosphor::FILE_TEXT,
        Kind::Other => phosphor::FILE,
        Kind::Folders => phosphor::FOLDERS,
    }
}

fn text(ui: &Ui, text: &str, font: FontId, color: Color32) -> Arc<Galley> {
    ui.painter().layout_no_wrap(text.to_owned(), font, color)
}

fn truncated(ui: &Ui, text: &str, font: FontId, color: Color32, max_w: f32) -> Arc<Galley> {
    let mut job = LayoutJob {
        wrap: TextWrapping {
            max_width: max_w,
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        },
        ..Default::default()
    };
    job.append(text, 0.0, TextFormat { font_id: font, color, ..Default::default() });
    ui.fonts_mut(|f| f.layout_job(job))
}
