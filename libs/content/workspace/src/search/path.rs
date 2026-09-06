use std::sync::Arc;

use egui::{Color32, Context, FontFamily, FontId, Galley, Id, Key, Modifiers, Ui, pos2};
use lb_rs::blocking::Lb;
use lb_rs::search::SearchFilter;

use crate::{
    search::{SearchExecutor, SearchType},
    show::InputStateExt,
    style::{
        FileRow, Space, ThemeExt, TypeRole, file_row_icon, parent_crumbs, phosphor,
        phosphor_ui_font_id, with_overlay_scroll,
    },
};

pub struct PathSearch {
    searcher: lb_rs::search::PathSearcher,
    submitted_query: String,
    selected: Option<usize>,
    activate: bool,
    activate_new_tab: bool,
    kb_mode: bool,
    selected_id: Option<lb_rs::Uuid>,
    scoped: bool,
}

struct Row {
    id: lb_rs::Uuid,
    filename: String,
    parent_path: String,
    is_folder: bool,
}

impl SearchExecutor for PathSearch {
    fn search_type(&self) -> SearchType {
        SearchType::Path
    }

    fn handle_query(&mut self, query: &str) {
        self.submitted_query = query.to_string();
        self.searcher.query(query);
        self.selected = None;
        self.kb_mode = false;
        self.selected_id = None;
    }

    fn update_filter(&mut self, filter: Option<SearchFilter>) {
        self.scoped = filter.is_some();
        self.searcher.update_filter(filter);
        self.selected = None;
        self.selected_id = None;
    }

    fn set_kb_mode(&mut self, kb_mode: bool) {
        self.kb_mode = kb_mode;
    }

    fn request_activate(&mut self) {
        self.activate = true;
    }

    fn request_activate_in_new_tab(&mut self) {
        self.activate = true;
        self.activate_new_tab = true;
    }

    fn has_rows(&self) -> bool {
        !self.searcher.results().is_empty()
    }

    fn show_result_picker(
        &mut self, ui: &mut egui::Ui, allow_kb_nav: bool, scope_name: &str, empty_centered: bool,
    ) -> super::PickerResponse {
        let rows = self.rows();
        let n = rows.len();
        if !self.submitted_query.is_empty() && n > 0 && self.selected.is_none() {
            self.selected = Some(0);
            self.kb_mode = true;
        }

        self.process_keys(ui.ctx(), allow_kb_nav);

        if let Some(i) = self.selected {
            if n == 0 {
                self.selected = None;
            } else if i >= n {
                self.selected = Some(n - 1);
            }
        }

        if self.activate {
            self.activate = false;
            let activated = self.selected.and_then(|i| rows.get(i).map(|r| r.id));
            let in_new_tab = std::mem::take(&mut self.activate_new_tab);
            return super::PickerResponse {
                activated,
                activated_in_new_tab: in_new_tab,
                selected: self.selected_id,
                selected_range: None,
                clear_scope: false,
            };
        }

        if n == 0 {
            let clear_scope = self.show_empty_state(ui, scope_name, empty_centered);
            return super::PickerResponse {
                activated: None,
                activated_in_new_tab: false,
                selected: self.selected_id,
                selected_range: None,
                clear_scope,
            };
        }

        let mut hovered: Option<usize> = None;
        let mut clicked: Option<usize> = None;
        let mut clicked_id: Option<lb_rs::Uuid> = None;
        let mut clicked_new_tab = false;

        const ROW_HEIGHT: f32 = FileRow::height_for(true);

        let highlight = self.selected;
        let t = ui.ctx().get_lb_theme();
        with_overlay_scroll(ui, Id::new("search_path_scroll"), |ui| {
            let out = egui::ScrollArea::vertical()
                .id_salt("search_path_rows")
                .auto_shrink([false, false])
                .show_rows(ui, ROW_HEIGHT, n, |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;

                    for index in range {
                        let Some(row) = rows.get(index) else { continue };

                        let resp = self.show_result_cell(ui, row, index, highlight == Some(index));
                        if self.kb_mode && self.selected == Some(index) {
                            resp.scroll_to_me(None);
                        }
                        if resp.hovered() {
                            hovered = Some(index);
                        }
                        if resp.clicked() {
                            clicked = Some(index);
                            clicked_id = Some(row.id);
                            clicked_new_tab = ui.input(|i| i.modifiers.command);
                        }
                        if let Some(new_tab) = crate::style::context_menu::show(&resp, &t, |e| {
                            e.item(phosphor::ARROW_SQUARE_OUT, "Open", false);
                            if !row.is_folder {
                                e.item(phosphor::APP_WINDOW, "Open in new tab", true);
                            }
                        }) {
                            clicked_id = Some(row.id);
                            clicked_new_tab = new_tab;
                        }
                    }
                });
            ((), out.state.offset.y, out.id)
        });

        if let Some(i) = clicked {
            self.selected = Some(i);
        } else if !self.kb_mode {
            if let Some(i) = hovered {
                self.selected = Some(i);
            }
        }

        let new_id = self.selected.and_then(|i| rows.get(i).map(|r| r.id));
        if new_id != self.selected_id {
            self.selected_id = new_id;
        }

        super::PickerResponse {
            activated: clicked_id,
            activated_in_new_tab: clicked_new_tab,
            selected: self.selected_id,
            selected_range: None,
            clear_scope: false,
        }
    }
}

impl PathSearch {
    pub fn new(lb: &Lb) -> Self {
        Self {
            searcher: lb.path_searcher(),
            submitted_query: String::new(),
            selected: None,
            activate: false,
            activate_new_tab: false,
            kb_mode: false,
            selected_id: None,
            scoped: false,
        }
    }

    fn rows(&self) -> Vec<Row> {
        self.searcher
            .results()
            .iter()
            .map(|r| Row {
                id: r.id,
                filename: r.filename.clone(),
                parent_path: r.parent_path.clone(),
                is_folder: r.is_folder,
            })
            .collect()
    }

    fn process_keys(&mut self, ctx: &Context, allow_kb_nav: bool) {
        const NUM_KEYS: [Key; 9] = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];

        if allow_kb_nav {
            ctx.input_mut(|i| {
                if i.consume_key_exact(Modifiers::NONE, Key::ArrowDown) {
                    self.selected = Some(self.selected.map_or(0, |i| i.saturating_add(1)));
                    self.kb_mode = true;
                }
                if i.consume_key_exact(Modifiers::NONE, Key::ArrowUp) {
                    self.selected = Some(self.selected.map_or(0, |i| i.saturating_sub(1)));
                    self.kb_mode = true;
                }
                if i.consume_key_exact(Modifiers::COMMAND, Key::Enter) && self.selected.is_some() {
                    self.activate = true;
                    self.activate_new_tab = true;
                }
                if i.consume_key_exact(Modifiers::NONE, Key::Enter) && self.selected.is_some() {
                    self.activate = true;
                }
                for (idx, &k) in NUM_KEYS.iter().enumerate() {
                    if i.consume_key_exact(Modifiers::COMMAND, k) {
                        self.selected = Some(idx);
                        self.activate = true;
                    }
                }
            });
        }

        if ctx.input(|i| i.pointer.delta().length_sq() > 16.0) {
            self.kb_mode = false;
        }
    }

    fn show_empty_state(&self, ui: &mut Ui, scope_name: &str, center: bool) -> bool {
        let (title, subtitle, in_folder) = if self.submitted_query.is_empty() {
            ("No recent files", "Start typing to search by name", None)
        } else {
            ("No files found", "", Some(scope_name))
        };
        super::paint_search_empty(ui, title, subtitle, self.scoped, in_folder, center)
    }

    fn show_result_cell(
        &self, ui: &mut Ui, row: &Row, index: usize, selected: bool,
    ) -> egui::Response {
        let t = ui.ctx().get_lb_theme();
        let trail = if index < 9 { shortcut_trail_w(ui, &t) } else { 0.0 };
        let icon =
            if row.is_folder { phosphor::FOLDER } else { file_row_icon(&row.filename, false) };
        let resp = FileRow::new(&t, &row.filename)
            .icon(icon)
            .subtitle(parent_crumbs(&row.parent_path))
            .selected(selected)
            .trail_reserve(trail)
            .show(ui, Id::new("search_row").with(row.id));
        if index < 9 {
            paint_row_shortcut(ui, &t, resp.rect, index + 1);
        }
        resp
    }
}

fn shortcut_mod(ui: &Ui, ink: Color32) -> Arc<Galley> {
    // Same language as button badges: Phosphor ⌘ at body, or “Ctrl” in body mono.
    let (text, font) = if cfg!(any(target_os = "macos", target_os = "ios")) {
        (phosphor::COMMAND.to_owned(), phosphor_ui_font_id())
    } else {
        ("Ctrl".to_owned(), FontId::new(TypeRole::Body.size(), FontFamily::Monospace))
    };
    ui.painter().layout_no_wrap(text, font, ink)
}

fn shortcut_num_font() -> FontId {
    FontId::new(TypeRole::Body.size(), FontFamily::Monospace)
}

pub(crate) fn shortcut_trail_w(ui: &Ui, t: &crate::style::Theme) -> f32 {
    let ink = t.neutral_fg_secondary();
    let m = shortcut_mod(ui, ink);
    let n = ui
        .painter()
        .layout_no_wrap("9".into(), shortcut_num_font(), ink);
    // Same outer inset as the leading file icon (`INDENT_BASE`).
    m.size().x + n.size().x + Space::Xxs.pts() + crate::style::tree_metrics::INDENT_BASE
}

pub(crate) fn paint_row_shortcut(ui: &Ui, t: &crate::style::Theme, row: egui::Rect, n: usize) {
    let ink = t.neutral_fg_secondary();
    let ng = ui
        .painter()
        .layout_no_wrap(n.to_string(), shortcut_num_font(), ink);
    let mg = shortcut_mod(ui, ink);
    let cy = row.center().y;
    let mut x = row.right() - crate::style::tree_metrics::INDENT_BASE;
    x -= ng.size().x;
    ui.painter()
        .galley(pos2(x, cy - ng.size().y / 2.0), ng, ink);
    x -= Space::Xxs.pts() + mg.size().x;
    ui.painter()
        .galley(pos2(x, cy - mg.size().y / 2.0), mg, ink);
}
