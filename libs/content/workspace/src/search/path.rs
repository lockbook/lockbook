use egui::{Context, CornerRadius, Frame, Key, Margin, Modifiers, Ui};
use lb_rs::blocking::Lb;
use lb_rs::search::SearchFilter;

use crate::{
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
}

struct Row {
    id: lb_rs::Uuid,
    filename: String,
    parent_path: String,
    is_folder: bool,
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
    }

    fn update_filter(&mut self, filter: Option<SearchFilter>) {
        self.searcher.update_filter(filter);
        self.selected = 0;
    }

    fn set_kb_mode(&mut self, kb_mode: bool) {
        self.kb_mode = kb_mode;
    }

    fn show_result_picker(
        &mut self, ui: &mut egui::Ui, allow_kb_nav: bool,
    ) -> super::PickerResponse {
        self.process_keys(ui.ctx(), allow_kb_nav);

        let rows = self.rows();
        let n = rows.len();

        if n > 0 && self.selected >= n {
            self.selected = n - 1;
        }

        if self.activate {
            self.activate = false;
            let activated = rows.get(self.selected).map(|r| r.id);
            return super::PickerResponse {
                activated,
                activated_in_new_tab: false,
                selected: self.selected_id,
                selected_range: None,
            };
        }

        if n == 0 {
            self.show_empty_state(ui);
            return super::PickerResponse {
                activated: None,
                activated_in_new_tab: false,
                selected: self.selected_id,
                selected_range: None,
            };
        }

        let mut hovered: Option<usize> = None;
        let mut clicked: Option<usize> = None;
        let mut clicked_id: Option<lb_rs::Uuid> = None;
        let mut clicked_new_tab = false;

        const ROW_HEIGHT: f32 = 16.0 * 1.3 + 13.0 * 1.3 + 6.0;

        ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
        ui.style_mut().spacing.scroll.floating = true;
        ui.style_mut().spacing.scroll.bar_width *= 2.0;
        ui.spacing_mut().scroll.floating_width = 12.0;
        ui.spacing_mut().scroll.dormant_handle_opacity = 0.5;

        let highlight = Some(self.selected);

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
                        clicked_new_tab = ui.input(|i| i.modifiers.command);
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Open in new tab").clicked() {
                            clicked_id = Some(row.id);
                            clicked_new_tab = true;
                            ui.close();
                        }
                    });
                }
            });

        if let Some(i) = clicked {
            self.selected = i;
        } else if !self.kb_mode {
            if let Some(i) = hovered {
                self.selected = i;
            }
        }

        let new_id = rows.get(self.selected).map(|r| r.id);
        if new_id != self.selected_id {
            self.selected_id = new_id;
        }

        super::PickerResponse {
            activated: clicked_id,
            activated_in_new_tab: clicked_new_tab,
            selected: self.selected_id,
            selected_range: None,
        }
    }
}

impl PathSearch {
    pub fn new(lb: &Lb) -> Self {
        Self {
            searcher: lb.path_searcher(),
            submitted_query: String::new(),
            selected: 0,
            activate: false,
            kb_mode: true,
            selected_id: None,
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
                path_indices: r.path_indices.clone(),
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
                    self.selected = self.selected.saturating_add(1);
                    self.kb_mode = true;
                }
                if i.consume_key_exact(Modifiers::NONE, Key::ArrowUp) {
                    self.selected = self.selected.saturating_sub(1);
                    self.kb_mode = true;
                }
                if i.consume_key_exact(Modifiers::NONE, Key::Enter) {
                    self.activate = true;
                }
                for (idx, &k) in NUM_KEYS.iter().enumerate() {
                    if i.consume_key_exact(Modifiers::COMMAND, k) {
                        self.selected = idx;
                        self.activate = true;
                    }
                }
            });
        }

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

                let (icon, icon_color) = if !row.is_folder {
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

                    if row.is_folder {
                        ui.add_space(6.0);
                        Icon::FILTER.size(14.0).color(parent_color).show(ui);
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
