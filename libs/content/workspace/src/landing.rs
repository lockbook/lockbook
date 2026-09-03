//! Empty editor: status, commands, optional recents. One column, no splash.

use egui::{
    Align, Align2, Color32, CursorIcon, FontFamily, FontId, Galley, Id, Layout, Rect, Sense, Ui,
    Vec2, pos2, vec2,
};
use lb_rs::Uuid;
use lb_rs::model::file::File;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::file_cache::{FileCache, FilesExt};
use crate::style::chrome::{KbdPart, Shortcut, shortcut_cmd_n, shortcut_cmd_o};
use crate::style::interact::{interact_fill_response, quiet_canvas_fills};
use crate::style::space::control as control_space;
use crate::style::{
    Radius, SECTION_HEAD_GAP, STROKE_HAIRLINE, Space, Spacer, Theme, ThemeExt, TypeRole, claim,
    control_height, display_file_name, file_row_icon, paint_file_name, paint_list_section,
    phosphor, phosphor_ui_font_id, place_at, sense_click, surface_motion,
};
use crate::workspace::Workspace;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LandingPage {
    #[serde(skip)]
    recent_files: Vec<File>,
    /// Last rows painted — kept while the hide animation runs.
    #[serde(skip)]
    recents_rows: Vec<(Uuid, String)>,
}

impl PartialEq for LandingPage {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

const COL_W: f32 = 280.0;
const RECENTS_VISIBLE: usize = 4;

fn italic_body() -> FontId {
    FontId::new(TypeRole::Body.size(), FontFamily::Name(Arc::from("Italic")))
}

impl LandingPage {
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn update_recent_files(&mut self, files: &FileCache) {
        let mut recent: Vec<_> = files
            .iter_files()
            .filter(|file| file.is_document())
            .cloned()
            .collect();
        recent.sort_by_key(|file| u64::MAX - files.last_modified_recursive(file.id));
        self.recent_files = recent;
    }
}

impl Workspace {
    pub fn show_landing_page(&mut self, ui: &mut Ui) {
        let t = ui.ctx().get_lb_theme();
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let live: Vec<(Uuid, String)> = self
            .landing_page
            .recent_files
            .iter()
            .take(RECENTS_VISIBLE)
            .map(|f| (f.id, f.name.clone()))
            .collect();
        let want_recents = !self.sidebar_open && !live.is_empty();
        if want_recents {
            self.landing_page.recents_rows = live;
        }
        let recents = self.landing_page.recents_rows.clone();
        let motion = surface_motion(ui.ctx(), Id::new("landing_recents"), want_recents);

        let col_w = (ui.max_rect().width() - Space::Xl.pts() * 2.0)
            .min(COL_W)
            .max(0.0);
        let status_h = TypeRole::Body.line_height();
        let cmd_h = control_height() * 2.0;
        let core_h = status_h + Space::Xl.pts() + cmd_h;
        let md = Space::Md.pts();
        let body_h = if recents.is_empty() {
            0.0
        } else {
            TypeRole::Body.line_height()
                + SECTION_HEAD_GAP.pts()
                + control_height() * recents.len() as f32
        };
        // Top surface is always commands only. Hairline is painted Md below
        // it (overlay) so showing the line does not change layout. Recents
        // slide out from that seam; cluster height follows the slide.
        let lower_h = md + body_h;
        let shown_lower = lower_h * motion.slide;
        let cluster_h = core_h + motion.slide * (md + lower_h);

        let max = ui.max_rect();
        let left = max.center().x - col_w / 2.0;
        let top = (max.center().y - cluster_h / 2.0).max(max.top());
        let upper_rect = Rect::from_min_size(pos2(left, top), vec2(col_w, core_h));
        let hairline_y = upper_rect.bottom() + md;
        let cluster = Rect::from_min_size(pos2(left, top), vec2(col_w, cluster_h));

        let mut open: Option<Uuid> = None;
        if shown_lower > 0.5 && !recents.is_empty() {
            let lower_top = hairline_y - lower_h * (1.0 - motion.slide);
            let lower_full = Rect::from_min_size(pos2(left, lower_top), vec2(col_w, lower_h));
            let slot = Rect::from_min_max(
                pos2(left, hairline_y),
                pos2(left + col_w, hairline_y + shown_lower),
            )
            .intersect(ui.clip_rect());
            place_at(ui, lower_full, Layout::top_down(Align::Min), |ui| {
                ui.set_clip_rect(slot);
                ui.set_width(col_w);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.add(Spacer::new(Space::Md));
                let (head, _) = ui
                    .allocate_exact_size(vec2(col_w, TypeRole::Body.line_height()), Sense::hover());
                paint_list_section(ui, &t, "Recents", head.min);
                ui.add(Spacer::new(SECTION_HEAD_GAP));
                for (id, name) in &recents {
                    if recent_row(ui, &t, name, Id::new("landing_recent").with(*id)).clicked() {
                        open = Some(*id);
                    }
                }
            });
        }

        let mut new_note = false;
        let mut search = false;
        place_at(ui, upper_rect, Layout::top_down(Align::Min), |ui| {
            ui.set_width(col_w);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (status, _) = ui.allocate_exact_size(vec2(col_w, status_h), Sense::hover());
            ui.painter().text(
                status.center(),
                Align2::CENTER_CENTER,
                "No file is open",
                italic_body(),
                t.neutral_fg_secondary(),
            );
            ui.add(Spacer::new(Space::Xl));
            if command_row(ui, &t, phosphor::NOTE_PENCIL, "New note", shortcut_cmd_n()).clicked() {
                new_note = true;
            }
            if command_row(ui, &t, phosphor::SEARCH, "Search", shortcut_cmd_o()).clicked() {
                search = true;
            }
        });

        if motion.slide > 0.0 {
            ui.painter().hline(
                upper_rect.x_range(),
                hairline_y,
                egui::Stroke { width: STROKE_HAIRLINE, color: t.neutral() },
            );
        }

        claim(ui, cluster);
        if new_note {
            self.create_doc(false);
        }
        if search {
            self.upsert_search(None);
        }
        if let Some(id) = open {
            self.open_file(id, true, false);
        }
    }
}

fn row_plate(ui: &mut Ui, t: &Theme, id: Option<Id>) -> (Rect, egui::Response) {
    let h = control_height();
    let w = ui.available_width().max(1.0);
    let (rect, resp) = if let Some(id) = id {
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        (rect, ui.interact(rect, id, sense_click()))
    } else {
        ui.allocate_exact_size(vec2(w, h), sense_click())
    };
    let fill = interact_fill_response(ui.ctx(), &resp, quiet_canvas_fills(t));
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);
    if resp.hovered() {
        ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
    }
    (rect, resp)
}

fn paint_row_icon(ui: &Ui, glyph: &str, color: Color32, x: f32, cy: f32) -> f32 {
    let g = ui
        .painter()
        .layout_no_wrap(glyph.into(), phosphor_ui_font_id(), color);
    let w = g.size().x;
    ui.painter()
        .galley(pos2(x, cy - g.size().y / 2.0), g, color);
    w
}

fn command_row(
    ui: &mut Ui, t: &Theme, icon: &'static str, label: &str, sc: Shortcut,
) -> egui::Response {
    let (rect, resp) = row_plate(ui, t, None);
    let pad = control_space::PAD_X.pts();
    let icon_gap = control_space::ICON_GAP.pts();
    let kbd = shortcut_galleys(ui, sc);
    let kbd_w = shortcut_width(&kbd);
    let ink = t.neutral_fg();
    let mute = t.neutral_fg_secondary();

    let icon_x = rect.left() + pad;
    let icon_w = paint_row_icon(ui, icon, mute, icon_x, rect.center().y);
    let text_x = icon_x + icon_w + icon_gap;

    let label_g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), TypeRole::Body.font_id(), ink);
    let label_max = (rect.right() - pad - kbd_w - Space::Sm.pts() - text_x).max(0.0);
    let label_g = if label_g.size().x > label_max {
        ui.painter()
            .layout(label.to_owned(), TypeRole::Body.font_id(), ink, label_max)
    } else {
        label_g
    };
    ui.painter()
        .galley(pos2(text_x, rect.center().y - label_g.size().y / 2.0), label_g, ink);

    let mut x = rect.right() - pad - kbd_w;
    let gap = control_space::PART_GAP.pts();
    for (i, g) in kbd.into_iter().enumerate() {
        if i > 0 {
            x += gap;
        }
        let w = g.size().x;
        ui.painter()
            .galley(pos2(x, rect.center().y - g.size().y / 2.0), g, mute);
        x += w;
    }
    resp
}

fn recent_row(ui: &mut Ui, t: &Theme, name: &str, id: Id) -> egui::Response {
    let (rect, resp) = row_plate(ui, t, Some(id));
    let pad = control_space::PAD_X.pts();
    let icon_gap = control_space::ICON_GAP.pts();
    let icon_x = rect.left() + pad;
    let icon_w =
        paint_row_icon(ui, file_row_icon(name, false), t.neutral_fg(), icon_x, rect.center().y);
    let slot = Rect::from_min_max(
        pos2(icon_x + icon_w + icon_gap, rect.top()),
        pos2(rect.right() - pad, rect.bottom()),
    );
    paint_file_name(ui, display_file_name(name), t.neutral_fg(), slot);
    resp
}

fn shortcut_galleys(ui: &Ui, sc: Shortcut) -> Vec<Arc<Galley>> {
    let mono = FontId::new(TypeRole::Body.size(), FontFamily::Monospace);
    let icon = phosphor_ui_font_id();
    sc.parts
        .iter()
        .map(|part| {
            let (text, font) = match *part {
                KbdPart::Icon(s) => (s.to_owned(), icon.clone()),
                KbdPart::Mono(s) | KbdPart::MonoSm(s) => (s.to_owned(), mono.clone()),
            };
            ui.painter()
                .layout_no_wrap(text, font, Color32::PLACEHOLDER)
        })
        .collect()
}

fn shortcut_width(galleys: &[Arc<Galley>]) -> f32 {
    let gap = control_space::PART_GAP.pts();
    let n = galleys.len();
    galleys.iter().map(|g| g.size().x).sum::<f32>() + gap * n.saturating_sub(1) as f32
}
