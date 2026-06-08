use std::sync::{Arc, RwLock};

use egui::{Context, CornerRadius, Frame, Key, Margin, Modifiers, Ui};
use lb_rs::blocking::Lb;

use crate::{
    file_cache::{FileCache, FilesExt},
    search::{SearchExecutor, SearchType},
    show::{DocType, InputStateExt},
    theme::{
        icons::Icon,
        palette_v2::{Palette, ThemeExt},
    },
    widgets::GlyphonLabel,
};

pub struct PathSearch {
    searcher: lb_rs::search::PathSearcher,
    submitted_query: String,
    selected: usize,
    activate: bool,
    kb_mode: bool,
    selected_id: Option<lb_rs::Uuid>,
    /// Whether the user has engaged the list (hover/arrow) since the last query
    /// change. Suggested docs start un-engaged so nothing is preselected.
    interacted: bool,
    files: Arc<RwLock<FileCache>>,
}

/// A single row in the picker. Sourced from search results, or from suggested
/// documents when no query has been typed.
struct Row {
    id: lb_rs::Uuid,
    filename: String,
    parent_path: String,
    /// Char indices (into the full path) to bold; empty for suggested docs.
    path_indices: Vec<u32>,
}

impl SearchExecutor for PathSearch {
    fn search_type(&self) -> SearchType {
        SearchType::Path
    }

    fn handle_query(&mut self, query: &str) {
        self.submitted_query = query.to_string();
        self.searcher.query(query);
        self.selected = 0;
        self.kb_mode = true;
        self.interacted = false;
    }

    fn set_kb_mode(&mut self, kb_mode: bool) {
        self.kb_mode = kb_mode;
    }

    fn show_result_picker(&mut self, ui: &mut egui::Ui) -> super::PickerResponse {
        self.process_keys(ui.ctx());

        // Suggested docs (shown before a query is typed) don't auto-select the
        // first row the way query results do — there's no selection (and so no
        // preview) until the user hovers or arrows into the list.
        let showing_suggested = self.submitted_query.is_empty();

        let rows = self.rows();
        let n = rows.len();

        if n > 0 && self.selected >= n {
            self.selected = n - 1;
        }

        if self.activate {
            self.activate = false;
            // Enter with no active selection (un-engaged suggestions) opens nothing.
            let has_selection = !showing_suggested || self.interacted;
            let activated =
                if has_selection { rows.get(self.selected).map(|r| r.id) } else { None };
            return super::PickerResponse {
                activated,
                selected: self.selected_id,
                selected_range: None,
            };
        }

        if n == 0 {
            self.show_empty_state(ui);
            return super::PickerResponse {
                activated: None,
                selected: self.selected_id,
                selected_range: None,
            };
        }

        let mut hovered: Option<usize> = None;
        let mut clicked: Option<usize> = None;
        let mut clicked_id: Option<lb_rs::Uuid> = None;

        const ROW_HEIGHT: f32 = 16.0 * 1.3 + 13.0 * 1.3 + 6.0;

        ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
        ui.style_mut().spacing.scroll.floating = true;
        ui.style_mut().spacing.scroll.bar_width *= 2.0;
        ui.spacing_mut().scroll.floating_width = 12.0;
        ui.spacing_mut().scroll.dormant_handle_opacity = 0.5;

        // Only highlight a row if there's an active selection.
        let highlight =
            if !showing_suggested || self.interacted { Some(self.selected) } else { None };

        egui::ScrollArea::vertical()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show_rows(ui, ROW_HEIGHT, n, |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;

                for index in range {
                    let Some(row) = rows.get(index) else { continue };

                    let resp = self.show_result_cell(ui, row, index, highlight == Some(index));
                    if resp.hovered() {
                        hovered = Some(index);
                    }
                    if resp.clicked() {
                        clicked = Some(index);
                        clicked_id = Some(row.id);
                    }
                }
            });

        // Hovering or clicking the list counts as engaging it.
        if hovered.is_some() || clicked.is_some() {
            self.interacted = true;
        }

        if let Some(i) = clicked {
            self.selected = i;
        } else if !self.kb_mode {
            if let Some(i) = hovered {
                self.selected = i;
            }
        }

        let has_selection = !showing_suggested || self.interacted;
        let new_id = if has_selection { rows.get(self.selected).map(|r| r.id) } else { None };
        if new_id != self.selected_id {
            self.selected_id = new_id;
        }

        super::PickerResponse {
            activated: clicked_id,
            selected: self.selected_id,
            selected_range: None,
        }
    }
}

impl PathSearch {
    pub fn new(lb: &Lb, _ctx: &Context, files: Arc<RwLock<FileCache>>) -> Self {
        Self {
            searcher: lb.path_searcher(),
            submitted_query: String::new(),
            selected: 0,
            activate: false,
            kb_mode: true,
            selected_id: None,
            interacted: false,
            files,
        }
    }

    /// The rows to display: live search results, or suggested documents before
    /// any query is typed.
    fn rows(&self) -> Vec<Row> {
        if self.submitted_query.is_empty() {
            let files = self.files.read().unwrap();
            files
                .suggested
                .iter()
                .filter_map(|id| {
                    let f = files.get_by_id(*id)?;
                    if !f.is_document() {
                        return None;
                    }
                    Some(Row {
                        id: *id,
                        filename: f.name.clone(),
                        parent_path: files.path(f.parent),
                        path_indices: Vec::new(),
                    })
                })
                .collect()
        } else {
            self.searcher
                .results()
                .iter()
                .map(|r| Row {
                    id: r.id,
                    filename: r.filename.clone(),
                    parent_path: r.parent_path.clone(),
                    path_indices: r.path_indices.clone(),
                })
                .collect()
        }
    }

    fn process_keys(&mut self, ctx: &Context) {
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

        // In suggested mode there's no selection until the user engages, so the
        // first arrow press lands on the first row rather than stepping past it.
        let has_selection = !self.submitted_query.is_empty() || self.interacted;

        ctx.input_mut(|i| {
            if i.consume_key_exact(Modifiers::NONE, Key::ArrowDown) {
                self.selected = if has_selection { self.selected.saturating_add(1) } else { 0 };
                self.interacted = true;
                self.kb_mode = true;
            }
            if i.consume_key_exact(Modifiers::NONE, Key::ArrowUp) {
                self.selected = if has_selection { self.selected.saturating_sub(1) } else { 0 };
                self.interacted = true;
                self.kb_mode = true;
            }
            if i.consume_key_exact(Modifiers::NONE, Key::Enter) {
                self.activate = true;
            }
            for (idx, &k) in NUM_KEYS.iter().enumerate() {
                if i.consume_key_exact(Modifiers::COMMAND, k) {
                    self.selected = idx;
                    self.activate = true;
                    self.interacted = true;
                }
            }
        });

        if ctx.input(|i| i.pointer.delta().length_sq() > 0.0) {
            self.kb_mode = false;
        }
    }

    fn show_empty_state(&self, ui: &mut Ui) {
        let theme = ui.ctx().get_lb_theme();
        let muted = theme.neutral_fg_secondary();
        let variant = theme.fg();

        let (icon, title, subtitle, icon_color): (Icon, &str, &str, _) =
            if self.submitted_query.is_empty() {
                (
                    Icon::SEARCH,
                    "Find a file",
                    "Start typing to search by name",
                    variant.get_color(Palette::Blue),
                )
            } else {
                (Icon::DOC_UNKNOWN, "No files found", "Try a different name", muted)
            };

        // Fill the available region so the pane doesn't collapse to 0 width.
        let rect = ui.available_rect_before_wrap();
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    icon.size(42.0).color(icon_color).show(ui);
                    ui.add_space(14.0);
                    ui.add(GlyphonLabel::new(title, theme.neutral_fg()).font_size(18.0));
                    ui.add_space(6.0);
                    ui.add(GlyphonLabel::new(subtitle, muted).font_size(13.0));
                });
            });
        });
    }

    fn show_result_cell(
        &self, ui: &mut Ui, row: &Row, index: usize, selected: bool,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let name_color = theme.neutral_fg();
        let parent_color = theme.neutral_fg_secondary();

        // Path indices are relative to full path; compute offset for filename
        let parent_char_len =
            row.parent_path.chars().count() as u32 + if row.parent_path.is_empty() { 0 } else { 1 }; // +1 for the '/'

        let mut frame = Frame::new()
            .inner_margin(Margin { left: 8, right: 8, top: 3, bottom: 3 })
            .outer_margin(Margin { left: 0, right: 20, top: 0, bottom: 0 })
            .corner_radius(CornerRadius::same(4));
        if selected {
            frame = frame.fill(theme.neutral_bg_tertiary());
        }

        let inner = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.set_min_height(16.0 * 1.3 + 13.0 * 1.3);

                let icon_size = 19.;
                let is_folder = row.filename.is_empty() || !row.filename.contains('.');

                let (icon, icon_color) = if !is_folder {
                    (
                        DocType::from_name(&row.filename).to_icon().size(icon_size),
                        theme.neutral_fg_secondary(),
                    )
                } else {
                    (Icon::FOLDER.size(icon_size), theme.fg().get_color(theme.prefs().primary))
                };
                icon.color(icon_color).show(ui);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    if index < 9 {
                        let modifier = if cfg!(any(target_os = "macos", target_os = "ios")) {
                            "⌘"
                        } else {
                            "Ctrl"
                        };
                        let number = (index + 1).to_string();
                        for glyph in [number.as_str(), modifier] {
                            ui.add(GlyphonLabel::new(glyph, parent_color).font_size(12.0));
                        }
                    }

                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        Self::highlighted_line(
                            ui,
                            &row.filename,
                            &row.path_indices,
                            parent_char_len,
                            name_color,
                            16.0,
                        );
                        Self::highlighted_line(
                            ui,
                            &row.parent_path,
                            &row.path_indices,
                            0,
                            parent_color,
                            13.0,
                        );
                    });
                });
            });
        });

        ui.interact(inner.response.rect, ui.id().with(("search_row", row.id)), egui::Sense::click())
    }

    fn highlighted_line(
        ui: &mut Ui, text: &str, highlights: &[u32], char_offset: u32, color: egui::Color32,
        size: f32,
    ) {
        let mut spans: Vec<(String, bool)> = Vec::new();
        for (i, c) in text.chars().enumerate() {
            let bold = highlights.contains(&(char_offset + i as u32));
            match spans.last_mut() {
                Some((s, b)) if *b == bold => s.push(c),
                _ => spans.push((c.to_string(), bold)),
            }
        }
        let span_refs: Vec<(&str, bool)> = spans.iter().map(|(s, b)| (s.as_str(), *b)).collect();
        ui.add(
            GlyphonLabel::new_rich(span_refs, color)
                .font_size(size)
                .max_width(ui.available_width()),
        );
    }
}
