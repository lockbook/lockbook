//! The import card above the sync footer: what's coming in, how far along,
//! and roughly how long is left. After the job is done it offers **Show**.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Align, Color32, FontFamily, FontId, Galley, Layout, Rect, Ui, pos2, vec2};
use workspace_rs::style::layout::PadContent;

use crate::components::{
    Button, Radius, Space, Theme, TypeRole, control_height, icon_button_hit, phosphor,
    phosphor_ui_font_id, place_at, tip_text,
};
use crate::shell::action::Action as A;
use workspace_rs::import::{JobView, Phase};

const PAD: Space = Space::Sm;
const GAP: Space = Space::Xs;
const BAR_H: f32 = Space::Xxs.pts() * 3.0;

pub struct ImportCard<'a> {
    pub view: JobView,
    pub t: &'a Theme,
    pub queue: &'a mut Vec<A>,
}

struct Metrics {
    title: Arc<Galley>,
    lines: Vec<Arc<Galley>>,
    mark: f32,
    height: f32,
}

impl ImportCard<'_> {
    fn running(&self) -> bool {
        self.view.phase != Phase::Done
    }

    fn icon(&self) -> &'static str {
        match self.view.phase {
            Phase::Importing => phosphor::DOWNLOAD_SIMPLE,
            Phase::Done if self.view.failed > 0 || self.view.refused => phosphor::WARNING_CIRCLE,
            Phase::Done => phosphor::CHECK_CIRCLE,
        }
    }

    fn title(&self) -> String {
        let s = &self.view.subject;
        match self.view.phase {
            Phase::Importing => format!("Importing **{s}**"),
            Phase::Done if self.view.refused => format!("**{s}** didn't import."),
            Phase::Done if self.view.failed > 0 => {
                let n = self.view.failed;
                format!("**{}** {} didn't import.", count(n), if n == 1 { "file" } else { "files" })
            }
            Phase::Done => format!("**{s}** is imported."),
        }
    }

    /// Secondary lines under the title (and under the bar while running).
    fn lines(&self) -> Vec<String> {
        let v = &self.view;
        if self.running() {
            let left = match v.eta {
                Some(secs) => eta(secs),
                None => "Estimating time left…".into(),
            };
            let mut out = vec![format!("{} of {} · {left}", count(v.done), count(v.total))];
            out.extend(v.current.clone());
            return out;
        }
        if v.refused {
            return vec!["It no longer fits your account.".into()];
        }
        if v.failed > 0 {
            let mut names = v.failed_names.join(", ");
            if v.failed > v.failed_names.len() {
                names.push_str(&format!(", and {} more", count(v.failed - v.failed_names.len())));
            }
            return vec![names];
        }
        let mut parts = vec![format!("{} files", count(v.total))];
        if v.same > 0 {
            parts.push(format!("{} already here", count(v.same)));
        }
        if v.skipped > 0 {
            let noun = if v.skipped == 1 { "app file" } else { "app files" };
            parts.push(format!("{} {noun} skipped", count(v.skipped)));
        }
        vec![parts.join(" · ")]
    }

    fn measure_parts(&self, ui: &Ui, width: f32) -> Metrics {
        let t = self.t;
        let mark = TypeRole::Body.line_height();
        let (p, g) = (PAD.pts(), GAP.pts());
        let x_col = if self.running() { 0.0 } else { mark + g };
        let text_w = (width - 2.0 * p - mark - g - x_col).max(1.0);
        let title = ui.fonts_mut(|f| f.layout_job(rich(&self.title(), text_w, t.neutral_fg(), 0)));
        let lines: Vec<_> = self
            .lines()
            .iter()
            .map(|l| ui.fonts_mut(|f| f.layout_job(rich(l, text_w, t.neutral_fg_secondary(), 1))))
            .collect();
        let mut h = p + title.size().y.max(mark);
        if self.running() {
            h += Space::Sm.pts() + BAR_H;
        }
        for l in &lines {
            h += GAP.pts() + l.size().y;
        }
        if !self.running() && self.view.root.is_some() {
            h += Space::Sm.pts() + control_height();
        }
        Metrics { title, lines, mark, height: h + p }
    }
}

impl PadContent for ImportCard<'_> {
    fn measure(&self, ui: &Ui, width: f32) -> f32 {
        self.measure_parts(ui, width).height
    }

    fn place(&mut self, ui: &mut Ui, rect: Rect) {
        let t = self.t;
        let m = self.measure_parts(ui, rect.width());
        let (p, g) = (PAD.pts(), GAP.pts());
        let ground = t.neutral_bg();
        ui.painter()
            .rect_filled(rect, Radius::Control.corner(), ground);

        let top = rect.top() + p;
        let left = rect.left() + p;
        let icon = ui.painter().layout_no_wrap(
            self.icon().into(),
            phosphor_ui_font_id(),
            Color32::PLACEHOLDER,
        );
        let icon_ink = if (self.view.failed > 0 || self.view.refused) && !self.running() {
            t.danger()
        } else {
            t.accent()
        };
        let icon_at =
            pos2(left + (m.mark - icon.size().x) / 2.0, top + (m.mark - icon.size().y) / 2.0);
        ui.painter().galley(icon_at, icon, icon_ink);

        let text_left = left + m.mark + g;
        ui.painter()
            .galley(pos2(text_left, top), m.title.clone(), t.neutral_fg());
        let mut y = top + m.title.size().y.max(m.mark);

        if !self.running() {
            let x_rect =
                Rect::from_min_size(pos2(rect.right() - p - m.mark, top), vec2(m.mark, m.mark));
            place_at(ui, x_rect, Layout::top_down(Align::Min), |ui| {
                let x = icon_button_hit(ui, t, phosphor::X, false, ground, m.mark);
                tip_text(ui.ctx(), &x, "Dismiss");
                if x.clicked() {
                    self.queue.push(A::ImportDismiss);
                }
            });
        } else {
            y += Space::Sm.pts();
            let track = Rect::from_min_max(pos2(text_left, y), pos2(rect.right() - p, y + BAR_H));
            let r = BAR_H / 2.0;
            ui.painter().rect_filled(track, r, t.neutral_bg_tertiary());
            let fill = Rect::from_min_size(
                track.min,
                vec2(track.width() * self.view.fraction.clamp(0.0, 1.0), BAR_H),
            );
            ui.painter()
                .with_clip_rect(track)
                .rect_filled(fill, r, t.accent());
            y += BAR_H;
        }

        for l in &m.lines {
            y += GAP.pts();
            ui.painter()
                .galley(pos2(text_left, y), l.clone(), t.neutral_fg_secondary());
            y += l.size().y;
        }

        if let (false, Some(root)) = (self.running(), self.view.root) {
            y += Space::Sm.pts();
            let button =
                Rect::from_min_max(pos2(text_left, y), pos2(rect.right() - p, rect.bottom() - p));
            place_at(ui, button, Layout::left_to_right(Align::Min), |ui| {
                if Button::secondary(t, "Show").show(ui).clicked() {
                    self.queue.push(A::ImportShow(root));
                }
            });
        }
    }
}

/// `n` with thousands separators: 3,214.
pub fn count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn eta(secs: f64) -> String {
    let mins = (secs / 60.0).round() as u64;
    match mins {
        0 => "less than a minute left".into(),
        1 => "about 1 minute left".into(),
        2..=89 => format!("about {mins} minutes left"),
        _ => format!("about {} hours left", (secs / 3600.0).round() as u64),
    }
}

/// `text` wrapped to `width` (or cut to `rows` with an ellipsis), `**…**` bold.
fn rich(text: &str, width: f32, color: Color32, rows: usize) -> LayoutJob {
    let mut job = LayoutJob {
        wrap: TextWrapping {
            max_width: width,
            max_rows: if rows == 0 { usize::MAX } else { rows },
            break_anywhere: rows != 0,
            overflow_character: (rows != 0).then_some('…'),
        },
        ..Default::default()
    };
    let size = TypeRole::Body.size();
    for (i, part) in text.split("**").enumerate() {
        let family = match i % 2 {
            0 => FontFamily::Proportional,
            _ => FontFamily::Name(Arc::from("Bold")),
        };
        job.append(
            part,
            0.0,
            TextFormat {
                font_id: FontId::new(size, family),
                color,
                line_height: Some(TypeRole::Body.line_height()),
                ..Default::default()
            },
        );
    }
    job
}
