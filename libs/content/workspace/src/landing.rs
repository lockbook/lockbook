//! Empty editor: day-zero welcome when the vault is empty; status, commands,
//! and optional recents once files exist.

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Align2, Color32, CornerRadius, FontFamily, FontId, Galley, Id, Key, Layout, Modifiers,
    Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2,
};
use lb_rs::Uuid;
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::filename::NameComponents;
use lb_rs::service::account::MARKDOWN_WELCOME;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::file_cache::{FileCache, FilesExt};
use crate::style::chrome::{KbdPart, Shortcut, shortcut_cmd_n, shortcut_cmd_o};
use crate::style::interact::{interact_fill_response, quiet_canvas_fills};
use crate::style::space::control as control_space;
use crate::style::{
    Radius, SECTION_HEAD_GAP, STROKE_HAIRLINE, Space, Spacer, Theme, ThemeExt, TypeRole, claim,
    context_menu, control_height, display_file_name, file_row_icon, paint_file_name,
    paint_list_section, phosphor, phosphor_font_id, phosphor_ui_font_id, place_at, sense_click,
    snap_surface_motion, surface_motion,
};
use crate::workspace::Workspace;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LandingPage {
    #[serde(skip)]
    recent_files: Vec<Recent>,
    /// Last rows painted — kept while the hide animation runs.
    #[serde(skip)]
    recents_rows: Vec<Recent>,
    /// `None` until fetched; cleared whenever files or pins change.
    #[serde(skip)]
    pinned: Option<Vec<Uuid>>,
    /// Highlighted option on the empty-vault welcome view.
    #[serde(skip)]
    selected_door: usize,
}

#[derive(Clone)]
struct Recent {
    id: Uuid,
    name: String,
    /// False for pending shares, which can't be pinned or duplicated.
    own: bool,
    /// Carries share metadata; same rule as the file tree's people mark.
    shared: bool,
    /// Fewest innermost ancestor folder names needed to disambiguate among visible recents.
    disambig: Option<String>,
    /// Last modified timestamp in ms since Unix epoch.
    last_modified_ms: u64,
}

impl PartialEq for LandingPage {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

const COL_W: f32 = 280.0;
const RECENTS_VISIBLE: usize = 5;
fn italic_body() -> FontId {
    FontId::new(TypeRole::Body.size(), FontFamily::Name(Arc::from("Italic")))
}

fn recent_row_id(id: Uuid) -> Id {
    Id::unique("landing_recent").with(id)
}

/// Ancestor folder names from outermost to innermost; own-tree paths start at `"Home"`.
fn ancestor_crumbs(files: &FileCache, id: Uuid, own: bool) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = files.get_by_id(id).map(|f| f.parent);
    while let Some(pid) = cur {
        if pid == files.root.id {
            break;
        }
        let Some(folder) = files.get_by_id(pid) else {
            break;
        };
        if folder.parent == pid {
            break;
        }
        parts.push(folder.name.clone());
        cur = Some(folder.parent);
    }
    if own {
        parts.push("Home".to_owned());
    }
    parts.reverse();
    parts
}

/// For each `(display_name, folder_crumbs)` item, return the fewest innermost
/// ancestor folder crumbs that distinguish it from every other item with the
/// same display name in a different folder (`None` when unambiguous).
fn disambiguating_folders(items: &[(&str, Vec<String>)]) -> Vec<Option<String>> {
    let trail = |crumbs: &[String], k: usize| crumbs[crumbs.len().saturating_sub(k)..].to_vec();
    items
        .iter()
        .enumerate()
        .map(|(i, (name, crumbs))| {
            let twins: Vec<&Vec<String>> = items
                .iter()
                .enumerate()
                .filter(|(j, (other, c))| *j != i && other == name && c != crumbs)
                .map(|(_, (_, c))| c)
                .collect();
            if twins.is_empty() || crumbs.is_empty() {
                return None;
            }
            let k = (1..=crumbs.len())
                .find(|&k| twins.iter().all(|c| trail(c, k) != trail(crumbs, k)))
                .unwrap_or(crumbs.len());
            Some(trail(crumbs, k).join("/"))
        })
        .collect()
}

fn format_relative_time(ms: u64) -> String {
    use chrono::{Datelike, Local, TimeZone};
    let now = Local::now();
    let Some(then) = Local.timestamp_millis_opt(ms as i64).single() else {
        return String::new();
    };
    let secs = (now.timestamp() - then.timestamp()).max(0);
    if secs < 60 {
        "Just now".into()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 && then.date_naive() == now.date_naive() {
        format!("{}h ago", secs / 3600)
    } else if now
        .date_naive()
        .signed_duration_since(then.date_naive())
        .num_days()
        == 1
    {
        "Yesterday".into()
    } else if then.year() == now.year() {
        then.format("%b %-d").to_string()
    } else {
        then.format("%b %-d, %Y").to_string()
    }
}

impl LandingPage {
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn update_recent_files(&mut self, files: &FileCache) {
        let mut recent: Vec<_> = files
            .iter_files()
            .filter(|file| file.is_document())
            .collect();
        recent.sort_by_key(|file| u64::MAX - files.last_modified_recursive(file.id));
        let top: Vec<_> = recent.into_iter().take(RECENTS_VISIBLE).collect();

        let keys: Vec<(String, Vec<String>)> = top
            .iter()
            .map(|f| {
                let own = files.tree_root(f.id) == files.root.id;
                let disp = display_file_name(&f.name).to_owned();
                (disp, ancestor_crumbs(files, f.id, own))
            })
            .collect();
        let key_refs: Vec<(&str, Vec<String>)> =
            keys.iter().map(|(n, c)| (n.as_str(), c.clone())).collect();
        let disambigs = disambiguating_folders(&key_refs);

        self.recent_files = top
            .into_iter()
            .zip(disambigs)
            .map(|(f, disambig)| Recent {
                id: f.id,
                name: f.name.clone(),
                own: files.tree_root(f.id) == files.root.id,
                shared: !f.shares.is_empty(),
                disambig,
                last_modified_ms: files.last_modified_recursive(f.id),
            })
            .collect();
        self.pinned = None;
    }
}

impl Workspace {
    pub fn show_landing_page(&mut self, ui: &mut Ui) {
        if self.empty_vault_welcome {
            let vault_empty = {
                let files = self.files.read().unwrap();
                files.children(files.root.id).is_empty()
            };
            if vault_empty {
                self.show_empty_vault_landing(ui);
            } else {
                self.show_desktop_landing(ui);
            }
            return;
        }

        let t = ui.ctx().get_lb_theme();
        let ctx = ui.ctx().clone();
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let motion_id = Id::unique("landing_recents");
        let want_recents = !self.sidebar_open && !self.landing_page.recent_files.is_empty();
        // The sidebar may have toggled while tabs were showing; don't animate that.
        if self.landing_page_first_frame {
            snap_surface_motion(&ctx, motion_id, want_recents);
        }
        if want_recents {
            self.landing_page.recents_rows = self.landing_page.recent_files.clone();
            if self.landing_page.pinned.is_none() {
                self.landing_page.pinned = Some(self.core.list_pinned().unwrap_or_default());
            }
        }
        let recents = self.landing_page.recents_rows.clone();
        let pinned_ids = self.landing_page.pinned.clone().unwrap_or_default();
        let motion = surface_motion(&ctx, motion_id, want_recents);

        let col_w = (ui.max_rect().width() - Space::Xl.pts() * 2.0).clamp(0.0, COL_W);
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

        let mut file_cmd: Option<(Uuid, RecentCmd)> = None;
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
                for r in &recents {
                    let pinned = pinned_ids.contains(&r.id);
                    let resp = recent_row(ui, &t, r, pinned);
                    if resp.clicked() {
                        file_cmd = Some((r.id, RecentCmd::Open));
                    }
                    if let Some(cmd) = context_menu::show(&resp, &t, |e| {
                        e.item(phosphor::ARROW_SQUARE_OUT, "Open", RecentCmd::Open);
                        e.item(phosphor::APP_WINDOW, "Open in New Tab", RecentCmd::OpenNewTab);
                        e.item(phosphor::LINK, "Copy Link", RecentCmd::CopyLink);
                        if r.own {
                            e.separator();
                            e.item(
                                phosphor::PUSH_PIN,
                                if pinned { "Unpin" } else { "Pin" },
                                RecentCmd::Pin,
                            );
                            e.item(phosphor::COPY, "Duplicate", RecentCmd::Duplicate);
                        }
                    }) {
                        file_cmd = Some((r.id, cmd));
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
                "No file open",
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
            self.upsert_search(Some(crate::search::SearchType::Path));
        }
        if let Some((id, cmd)) = file_cmd {
            match cmd {
                RecentCmd::Open => self.open_file(id, true, false),
                RecentCmd::OpenNewTab => self.open_file(id, true, true),
                RecentCmd::CopyLink => {
                    if let Ok(url) = self.core.get_file_link_url(id) {
                        ctx.copy_text(url);
                    }
                }
                RecentCmd::Pin => self.toggle_pin(id),
                RecentCmd::Duplicate => self.duplicate_doc(id),
            }
        }
    }
}

#[derive(Clone, Copy)]
enum RecentCmd {
    Open,
    OpenNewTab,
    CopyLink,
    Pin,
    Duplicate,
}

impl Workspace {
    pub(crate) fn toggle_pin(&mut self, id: Uuid) {
        let pinned = self.core.list_pinned().unwrap_or_default();
        let res =
            if pinned.contains(&id) { self.core.unpin_file(id) } else { self.core.pin_file(id) };
        match res {
            Ok(()) => {
                self.landing_page.pinned = None;
                self.out.file_cache_updated = true;
                self.ctx.request_repaint();
            }
            Err(err) => self.out.failure_messages.push(format!("Pin failed: {err}")),
        }
    }

    fn duplicate_doc(&mut self, id: Uuid) {
        match self.core.duplicate_file(&id) {
            Ok(file) => {
                self.files
                    .write()
                    .unwrap()
                    .insert_created_file(file.clone());
                self.out.file_cache_updated = true;
                self.out.file_created = Some(Ok(file.clone()));
                if file.is_document() {
                    self.open_file(file.id, true, false);
                }
                self.ctx.request_repaint();
            }
            Err(err) => self
                .out
                .failure_messages
                .push(format!("Duplicate failed: {err}")),
        }
    }

    pub(crate) fn create_sample_note(&mut self) {
        let parent = self.files.read().unwrap().root.id;
        let children = self.core.get_children(&parent).unwrap_or_default();
        let name = NameComponents::from("welcome.md").free_number_in(&children);

        let created = self
            .core
            .create_file(&name, &parent, FileType::Document)
            .and_then(|file| {
                self.core
                    .write_document(file.id, MARKDOWN_WELCOME.as_bytes())?;
                Ok(file)
            });

        match created {
            Ok(file) => {
                self.files
                    .write()
                    .unwrap()
                    .insert_created_file(file.clone());
                self.out.file_cache_updated = true;
                self.out.file_created = Some(Ok(file.clone()));
                self.open_file(file.id, true, false);
                self.ctx.request_repaint();
            }
            Err(err) => self
                .out
                .failure_messages
                .push(format!("Couldn’t create sample note: {err}")),
        }
    }

    fn show_empty_vault_landing(&mut self, ui: &mut Ui) {
        let t = ui.ctx().get_lb_theme();
        let ctx = ui.ctx().clone();
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let doors: [EmptyDoor; 3] = [
            EmptyDoor {
                action: EmptyAction::NewNote,
                icon: phosphor::NOTE_PENCIL,
                title: "New note",
                desc: "Start writing in Markdown",
            },
            EmptyDoor {
                action: EmptyAction::Import,
                icon: phosphor::DOWNLOAD_SIMPLE,
                title: "Import files or folders",
                desc: "Add documents or anything else",
            },
            EmptyDoor {
                action: EmptyAction::SampleNote,
                icon: phosphor::FILE_TEXT,
                title: "Open a sample note",
                desc: "A short guide to writing in Markdown",
            },
        ];

        if self.landing_page_first_frame {
            self.landing_page.selected_door = 0;
        }
        let n = doors.len();
        self.landing_page.selected_door = self.landing_page.selected_door.min(n - 1);

        let mut triggered: Option<EmptyAction> = None;
        let can_keys =
            ui.is_enabled() && !ctx.egui_wants_keyboard_input() && !egui::Popup::is_any_open(&ctx);
        if can_keys {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                self.landing_page.selected_door = (self.landing_page.selected_door + 1) % n;
            } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                self.landing_page.selected_door = (self.landing_page.selected_door + n - 1) % n;
            } else if ctx.input_mut(|i| {
                // Ignore key-repeat so holding Enter across account creation doesn't open a note.
                let fresh = i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Key {
                            key: Key::Enter,
                            pressed: true,
                            repeat: false,
                            modifiers,
                            ..
                        } if *modifiers == Modifiers::NONE
                    )
                });
                fresh && i.consume_key(Modifiers::NONE, Key::Enter)
            }) {
                triggered = Some(doors[self.landing_page.selected_door].action);
            }
        }

        let max = ui.max_rect();
        let col_w = (max.width() - Space::Xl.pts() * 2.0).clamp(180.0, EMPTY_COL_W);
        let title_h = 28.0_f32;
        let head_to_rows = 24.0_f32;
        let row_h = 64.0_f32;
        let row_gap = 10.0_f32;
        let rows_h = row_h * n as f32 + row_gap * n.saturating_sub(1) as f32;
        let cluster_h = title_h + head_to_rows + rows_h;

        let left = max.center().x - col_w / 2.0;
        let top = (max.center().y - cluster_h / 2.0 - 8.0).max(max.top() + Space::Lg.pts());
        let cluster = Rect::from_min_size(pos2(left, top), vec2(col_w, cluster_h));

        let accent = t.accent();
        let is_dark = ui.visuals().dark_mode;
        let bold_title = FontId::new(22.0, FontFamily::Name(Arc::from("Bold")));
        let bold_row = FontId::new(14.0, FontFamily::Name(Arc::from("Bold")));
        let body_sm = FontId::new(12.5, FontFamily::Proportional);

        // Title only
        ui.painter().text(
            pos2(left, top),
            Align2::LEFT_TOP,
            "Welcome to Lockbook",
            bold_title,
            t.neutral_fg(),
        );

        let pointer_moved = ctx.input(|i| i.pointer.delta().length_sq() > 0.0);
        let mut y = top + title_h + head_to_rows;
        for (idx, door) in doors.iter().enumerate() {
            let rect = Rect::from_min_size(pos2(left, y), vec2(col_w, row_h));
            let resp = ui.interact(rect, Id::unique("landing_empty_door").with(idx), sense_click());
            if resp.hovered() && pointer_moved && ui.is_enabled() {
                self.landing_page.selected_door = idx;
            }
            let selected = self.landing_page.selected_door == idx;
            let corner = CornerRadius::same(10);

            if selected {
                let press = resp.is_pointer_button_down_on();
                let fill_t = match (is_dark, press) {
                    (true, true) => 0.14,
                    (true, false) => 0.08,
                    (false, true) => 0.10,
                    (false, false) => 0.05,
                };
                let fill = t.neutral_bg().lerp_to_gamma(accent, fill_t);
                ui.painter().rect_filled(rect, corner, fill);
            }

            // 34x34 icon badge
            let badge =
                Rect::from_center_size(pos2(rect.left() + 30.0, rect.center().y), vec2(34.0, 34.0));
            let badge_fill = if selected {
                t.neutral_bg()
                    .lerp_to_gamma(accent, if is_dark { 0.18 } else { 0.12 })
            } else if is_dark {
                t.neutral_bg_tertiary().lerp_to_gamma(t.neutral_bg(), 0.25)
            } else {
                t.neutral_bg_secondary()
            };
            ui.painter()
                .rect_filled(badge, CornerRadius::same(7), badge_fill);
            let icon_col = if selected { accent } else { t.neutral_fg_secondary() };
            paint_centered_phosphor(ui.painter(), badge.center(), door.icon, 16.5, icon_col);

            // Title + description
            let tx = badge.right() + 14.0;
            let text_max = (rect.right() - 36.0 - tx).max(0.0);

            let mut title_job = LayoutJob::simple_singleline(
                door.title.to_owned(),
                bold_row.clone(),
                t.neutral_fg(),
            );
            title_job.wrap = TextWrapping::truncate_at_width(text_max);
            let title_g = ui.painter().layout_job(title_job);
            ui.painter().galley(
                pos2(tx, rect.center().y - 9.0 - title_g.size().y / 2.0),
                title_g,
                t.neutral_fg(),
            );

            let mut desc_job = LayoutJob::simple_singleline(
                door.desc.to_owned(),
                body_sm.clone(),
                t.neutral_fg_secondary(),
            );
            desc_job.wrap = TextWrapping::truncate_at_width(text_max);
            let desc_g = ui.painter().layout_job(desc_job);
            ui.painter().galley(
                pos2(tx, rect.center().y + 10.0 - desc_g.size().y / 2.0),
                desc_g,
                t.neutral_fg_secondary(),
            );

            // Trailing chevron
            paint_centered_phosphor(
                ui.painter(),
                pos2(rect.right() - 20.0, rect.center().y),
                phosphor::CARET_RIGHT,
                13.0,
                if selected { accent } else { t.neutral_fg_secondary() },
            );

            if resp.clicked() {
                triggered = Some(door.action);
            }
            y += row_h + row_gap;
        }

        claim(ui, cluster);

        if let Some(act) = triggered {
            match act {
                EmptyAction::NewNote => self.create_doc(false),
                EmptyAction::Import => self.out.open_import = true,
                EmptyAction::SampleNote => self.create_sample_note(),
            }
        }
    }

    /// Desktop "no file open" landing once the vault has files:
    /// `START` section (New note / New drawing / Search — same row language as
    /// `Welcome to Lockbook`, no shortcut badge on Search) + hairline + `RECENTS`
    /// with folder crumbs only when they disambiguate duplicate names. One unified
    /// keyboard + pointer selection walks seamlessly across both sections.
    fn show_desktop_landing(&mut self, ui: &mut Ui) {
        let t = ui.ctx().get_lb_theme();
        let ctx = ui.ctx().clone();
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        if self.landing_page.pinned.is_none() {
            self.landing_page.pinned = Some(self.core.list_pinned().unwrap_or_default());
        }
        let recents = self.landing_page.recent_files.clone();
        let pinned_ids = self.landing_page.pinned.clone().unwrap_or_default();

        let doors: [DesktopDoor; 3] = [
            DesktopDoor {
                action: DesktopDoorAction::NewNote,
                icon: phosphor::NOTE_PENCIL,
                title: "New note",
                desc: "Start writing in Markdown",
            },
            DesktopDoor {
                action: DesktopDoorAction::Import,
                icon: phosphor::DOWNLOAD_SIMPLE,
                title: "Import files or folders",
                desc: "Add documents or anything else",
            },
            DesktopDoor {
                action: DesktopDoorAction::Search,
                icon: phosphor::SEARCH,
                title: "Search",
                desc: "Jump to any note or folder",
            },
        ];

        let door_count = doors.len();
        let total_items = door_count + recents.len();
        if self.landing_page_first_frame {
            self.landing_page.selected_door = 0;
        }
        self.landing_page.selected_door = self.landing_page.selected_door.min(total_items - 1);

        let mut door_triggered: Option<DesktopDoorAction> = None;
        let mut file_cmd: Option<(Uuid, RecentCmd)> = None;

        let can_keys =
            ui.is_enabled() && !ctx.egui_wants_keyboard_input() && !egui::Popup::is_any_open(&ctx);
        if can_keys {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                self.landing_page.selected_door =
                    (self.landing_page.selected_door + 1) % total_items;
            } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                self.landing_page.selected_door =
                    (self.landing_page.selected_door + total_items - 1) % total_items;
            } else if ctx.input_mut(|i| {
                let fresh = i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Key {
                            key: Key::Enter,
                            pressed: true,
                            repeat: false,
                            modifiers,
                            ..
                        } if *modifiers == Modifiers::NONE
                    )
                });
                fresh && i.consume_key(Modifiers::NONE, Key::Enter)
            }) {
                let sel = self.landing_page.selected_door;
                if sel < door_count {
                    door_triggered = Some(doors[sel].action);
                } else if let Some(r) = recents.get(sel - door_count) {
                    file_cmd = Some((r.id, RecentCmd::Open));
                }
            }
        }

        let max = ui.max_rect();
        let col_w = (max.width() - Space::Xl.pts() * 2.0).clamp(180.0, EMPTY_COL_W);
        let head_h = TypeRole::Body.line_height();
        let head_gap = SECTION_HEAD_GAP.pts();
        let door_h = 56.0_f32;
        let door_gap = 6.0_f32;
        let doors_h = door_h * door_count as f32 + door_gap * (door_count - 1) as f32;

        let rec_h = 34.0_f32;
        let rec_gap = 2.0_f32;
        let hr_top = 18.0_f32;
        let hr_bot = 14.0_f32;
        let recents_block_h = if recents.is_empty() {
            0.0
        } else {
            hr_top
                + hr_bot
                + head_h
                + head_gap
                + rec_h * recents.len() as f32
                + rec_gap * recents.len().saturating_sub(1) as f32
        };

        let cluster_h = head_h + head_gap + doors_h + recents_block_h;
        let left = max.center().x - col_w / 2.0;
        let top = (max.center().y - cluster_h / 2.0 - 6.0).max(max.top() + Space::Lg.pts());
        let cluster = Rect::from_min_size(pos2(left, top), vec2(col_w, cluster_h));

        let accent = t.accent();
        let is_dark = ui.visuals().dark_mode;
        let bold_row = FontId::new(13.5, FontFamily::Name(Arc::from("Bold")));
        let body_sm = FontId::new(12.0, FontFamily::Proportional);
        let meta_font = FontId::new(11.5, FontFamily::Proportional);
        let pointer_moved = ctx.input(|i| i.pointer.delta().length_sq() > 0.0);

        let paint_selection = |ui: &Ui, rect: Rect, corner: CornerRadius, down: bool| {
            let fill_t = match (is_dark, down) {
                (true, true) => 0.14,
                (true, false) => 0.08,
                (false, true) => 0.10,
                (false, false) => 0.05,
            };
            let fill = t.neutral_bg().lerp_to_gamma(accent, fill_t);
            ui.painter().rect_filled(rect, corner, fill);
        };

        let mut y = top;

        // Section 1: START
        paint_list_section(ui, &t, "Start", pos2(left + 4.0, y));
        y += head_h + head_gap;

        for (idx, door) in doors.iter().enumerate() {
            let rect = Rect::from_min_size(pos2(left, y), vec2(col_w, door_h));
            let resp =
                ui.interact(rect, Id::unique("landing_desktop_door").with(idx), sense_click());
            if resp.hovered() && pointer_moved && ui.is_enabled() {
                self.landing_page.selected_door = idx;
            }
            let selected = self.landing_page.selected_door == idx;

            if selected {
                paint_selection(ui, rect, CornerRadius::same(10), resp.is_pointer_button_down_on());
            }

            // 32x32 icon badge
            let badge =
                Rect::from_center_size(pos2(rect.left() + 28.0, rect.center().y), vec2(32.0, 32.0));
            let badge_fill = if selected {
                t.neutral_bg()
                    .lerp_to_gamma(accent, if is_dark { 0.18 } else { 0.12 })
            } else if is_dark {
                t.neutral_bg_tertiary().lerp_to_gamma(t.neutral_bg(), 0.25)
            } else {
                t.neutral_bg_secondary()
            };
            ui.painter()
                .rect_filled(badge, CornerRadius::same(7), badge_fill);
            let icon_col = if selected { accent } else { t.neutral_fg_secondary() };
            paint_centered_phosphor(ui.painter(), badge.center(), door.icon, 15.5, icon_col);

            // Title + description
            let tx = badge.right() + 13.0;
            let text_max = (rect.right() - 34.0 - tx).max(0.0);

            let mut title_job = LayoutJob::simple_singleline(
                door.title.to_owned(),
                bold_row.clone(),
                t.neutral_fg(),
            );
            title_job.wrap = TextWrapping::truncate_at_width(text_max);
            let title_g = ui.painter().layout_job(title_job);
            ui.painter().galley(
                pos2(tx, rect.center().y - 8.5 - title_g.size().y / 2.0),
                title_g,
                t.neutral_fg(),
            );

            let mut desc_job = LayoutJob::simple_singleline(
                door.desc.to_owned(),
                body_sm.clone(),
                t.neutral_fg_secondary(),
            );
            desc_job.wrap = TextWrapping::truncate_at_width(text_max);
            let desc_g = ui.painter().layout_job(desc_job);
            ui.painter().galley(
                pos2(tx, rect.center().y + 9.0 - desc_g.size().y / 2.0),
                desc_g,
                t.neutral_fg_secondary(),
            );

            // Trailing chevron (no shortcut badge on Search or other doors)
            paint_centered_phosphor(
                ui.painter(),
                pos2(rect.right() - 18.0, rect.center().y),
                phosphor::CARET_RIGHT,
                13.0,
                if selected { accent } else { t.neutral_fg_secondary() },
            );

            if resp.clicked() {
                door_triggered = Some(door.action);
            }
            y += door_h + door_gap;
        }

        // Section 2: Hairline + RECENTS
        if !recents.is_empty() {
            y -= door_gap;
            y += hr_top;
            ui.painter().hline(
                (left + 4.0)..=(left + col_w - 4.0),
                y,
                Stroke::new(STROKE_HAIRLINE, t.neutral()),
            );
            y += hr_bot;

            paint_list_section(ui, &t, "Recents", pos2(left + 4.0, y));
            y += head_h + head_gap;

            for (ri, r) in recents.iter().enumerate() {
                let item_idx = door_count + ri;
                let rect = Rect::from_min_size(pos2(left, y), vec2(col_w, rec_h));
                let resp = ui.interact(rect, recent_row_id(r.id), sense_click());
                if resp.hovered() && pointer_moved && ui.is_enabled() {
                    self.landing_page.selected_door = item_idx;
                }
                let selected = self.landing_page.selected_door == item_idx;
                let pinned = pinned_ids.contains(&r.id);

                if selected {
                    paint_selection(
                        ui,
                        rect,
                        CornerRadius::same(8),
                        resp.is_pointer_button_down_on(),
                    );
                }

                let pad = 12.0_f32;
                let icon_gap = 9.0_f32;
                let icon_col = if selected { accent } else { t.neutral_fg_secondary() };
                let icon_x = rect.left() + pad;
                let icon_w = paint_row_icon(
                    ui,
                    file_row_icon(&r.name, false),
                    icon_col,
                    icon_x,
                    rect.center().y,
                );

                // Trailing right-to-left: relative time (with disambiguating folder if needed),
                // then optional pin & share marks.
                let mut right = rect.right() - pad;
                let rel = format_relative_time(r.last_modified_ms);
                let meta = match (&r.disambig, rel.is_empty()) {
                    (Some(folder), false) => format!("{folder} · {rel}"),
                    (Some(folder), true) => folder.clone(),
                    (None, false) => rel,
                    (None, true) => String::new(),
                };
                if !meta.is_empty() {
                    let meta_g = ui.painter().layout_no_wrap(
                        meta,
                        meta_font.clone(),
                        t.neutral_fg_secondary(),
                    );
                    let mw = meta_g.size().x;
                    ui.painter().galley(
                        pos2(right - mw, rect.center().y - meta_g.size().y / 2.0),
                        meta_g,
                        t.neutral_fg_secondary(),
                    );
                    right -= mw + 10.0;
                }

                let marks = [(pinned, phosphor::PUSH_PIN), (r.shared, phosphor::USERS)];
                for (_, glyph) in marks.into_iter().filter(|(on, _)| *on) {
                    let font = phosphor_ui_font_id();
                    let w = ui
                        .painter()
                        .layout_no_wrap(glyph.into(), font, Color32::PLACEHOLDER)
                        .size()
                        .x;
                    paint_row_icon(
                        ui,
                        glyph,
                        if selected { accent } else { t.neutral_fg_secondary() },
                        right - w,
                        rect.center().y,
                    );
                    right -= w + 8.0;
                }

                let slot = Rect::from_min_max(
                    pos2(icon_x + icon_w + icon_gap, rect.top()),
                    pos2(right.max(icon_x + icon_w + icon_gap), rect.bottom()),
                );
                paint_file_name(ui, display_file_name(&r.name), t.neutral_fg(), slot);

                if resp.clicked() {
                    file_cmd = Some((r.id, RecentCmd::Open));
                }
                if let Some(cmd) = context_menu::show(&resp, &t, |e| {
                    e.item(phosphor::ARROW_SQUARE_OUT, "Open", RecentCmd::Open);
                    e.item(phosphor::APP_WINDOW, "Open in New Tab", RecentCmd::OpenNewTab);
                    e.item(phosphor::LINK, "Copy Link", RecentCmd::CopyLink);
                    if r.own {
                        e.separator();
                        e.item(
                            phosphor::PUSH_PIN,
                            if pinned { "Unpin" } else { "Pin" },
                            RecentCmd::Pin,
                        );
                        e.item(phosphor::COPY, "Duplicate", RecentCmd::Duplicate);
                    }
                }) {
                    file_cmd = Some((r.id, cmd));
                }

                y += rec_h + rec_gap;
            }
        }

        claim(ui, cluster);

        if let Some(act) = door_triggered {
            match act {
                DesktopDoorAction::NewNote => self.create_doc(false),
                DesktopDoorAction::Import => self.out.open_import = true,
                DesktopDoorAction::Search => {
                    self.upsert_search(Some(crate::search::SearchType::Path));
                }
            }
        }
        if let Some((id, cmd)) = file_cmd {
            match cmd {
                RecentCmd::Open => self.open_file(id, true, false),
                RecentCmd::OpenNewTab => self.open_file(id, true, true),
                RecentCmd::CopyLink => {
                    if let Ok(url) = self.core.get_file_link_url(id) {
                        ctx.copy_text(url);
                    }
                }
                RecentCmd::Pin => self.toggle_pin(id),
                RecentCmd::Duplicate => self.duplicate_doc(id),
            }
        }
    }
}

#[derive(Clone, Copy)]
enum DesktopDoorAction {
    NewNote,
    Import,
    Search,
}

struct DesktopDoor {
    action: DesktopDoorAction,
    icon: &'static str,
    title: &'static str,
    desc: &'static str,
}

const EMPTY_COL_W: f32 = 468.0;

#[derive(Clone, Copy)]
enum EmptyAction {
    NewNote,
    Import,
    SampleNote,
}

struct EmptyDoor {
    action: EmptyAction,
    icon: &'static str,
    title: &'static str,
    desc: &'static str,
}

fn paint_centered_phosphor(
    painter: &egui::Painter, center: Pos2, glyph: &str, size: f32, color: Color32,
) {
    let g = painter.layout_no_wrap(glyph.into(), phosphor_font_id(size), Color32::PLACEHOLDER);
    let ink_center = if g.mesh_bounds.width() > 0.0 && g.mesh_bounds.height() > 0.0 {
        g.mesh_bounds.center()
    } else {
        (g.size() / 2.0).to_pos2()
    };
    painter.galley(pos2(center.x - ink_center.x, center.y - ink_center.y), g, color);
}

fn row_plate(ui: &mut Ui, t: &Theme, id: Id) -> (Rect, egui::Response) {
    let (rect, _) = ui
        .allocate_exact_size(vec2(ui.available_width().max(1.0), control_height()), Sense::hover());
    let resp = ui.interact(rect, id, sense_click());
    let fill = interact_fill_response(ui.ctx(), &resp, quiet_canvas_fills(t));
    ui.painter()
        .rect_filled(rect, Radius::Control.corner(), fill);
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
    let (rect, resp) = row_plate(ui, t, Id::unique("landing_cmd").with(label));
    let pad = control_space::PAD_X.pts();
    let icon_gap = control_space::ICON_GAP.pts();
    let ink = t.neutral_fg();
    let mute = t.neutral_fg_secondary();

    let icon_x = rect.left() + pad;
    let icon_w = paint_row_icon(ui, icon, mute, icon_x, rect.center().y);
    let text_x = icon_x + icon_w + icon_gap;
    let text_max = (rect.right() - pad - text_x).max(0.0);

    let font = TypeRole::Body.font_id();
    let label_w = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), ink)
        .size()
        .x;
    // The hint yields to the label when both don't fit.
    let kbd = shortcut_galleys(ui, sc);
    let kbd_w = shortcut_width(&kbd);
    let kbd = if label_w + Space::Sm.pts() + kbd_w <= text_max { kbd } else { vec![] };

    let mut job = LayoutJob::simple_singleline(label.to_owned(), font, ink);
    job.wrap = TextWrapping::truncate_at_width(text_max);
    let label_g = ui.painter().layout_job(job);
    ui.painter()
        .galley(pos2(text_x, rect.center().y - label_g.size().y / 2.0), label_g, ink);

    let mut x = rect.right() - pad - shortcut_width(&kbd);
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

fn recent_row(ui: &mut Ui, t: &Theme, r: &Recent, pinned: bool) -> egui::Response {
    let (rect, resp) = row_plate(ui, t, recent_row_id(r.id));
    let pad = control_space::PAD_X.pts();
    let icon_gap = control_space::ICON_GAP.pts();
    let icon_x = rect.left() + pad;
    let icon_w =
        paint_row_icon(ui, file_row_icon(&r.name, false), t.neutral_fg(), icon_x, rect.center().y);
    // Trailing marks, right to left: pin, then share (as in the file tree).
    let mut right = rect.right() - pad;
    let marks = [(pinned, phosphor::PUSH_PIN), (r.shared, phosphor::USERS)];
    for (_, glyph) in marks.into_iter().filter(|(on, _)| *on) {
        let font = phosphor_ui_font_id();
        let w = ui
            .painter()
            .layout_no_wrap(glyph.into(), font, Color32::PLACEHOLDER)
            .size()
            .x;
        paint_row_icon(ui, glyph, t.neutral_fg_secondary(), right - w, rect.center().y);
        right -= w + icon_gap;
    }
    let slot = Rect::from_min_max(
        pos2(icon_x + icon_w + icon_gap, rect.top()),
        pos2(right, rect.bottom()),
    );
    paint_file_name(ui, display_file_name(&r.name), t.neutral_fg(), slot);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn crumbs(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn disambiguating_folders_shows_only_when_names_collide() {
        let items = [
            ("roadmap", crumbs(&["Home", "Projects"])),
            ("todo", crumbs(&["Home"])),
            ("todo", crumbs(&["Home", "Work"])),
            ("notes", crumbs(&["Home", "a", "x"])),
            ("notes", crumbs(&["Home", "b", "x"])),
            ("plan", crumbs(&["Home", "Work"])),
            ("plan", crumbs(&["Home", "Work"])),
        ];
        assert_eq!(
            disambiguating_folders(&items),
            [
                None,
                Some("Home".into()),
                Some("Work".into()),
                Some("a/x".into()),
                Some("b/x".into()),
                None,
                None,
            ]
        );
    }
}
