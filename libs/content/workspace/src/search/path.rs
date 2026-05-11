use egui::{Context, CornerRadius, Frame, Key, Margin, Modifiers, Ui};
use lb_rs::blocking::Lb;
use lb_rs::search::{PathSearcher, SearchResult};

use crate::{
    search::{SearchExecutor, SearchType},
    show::{DocType, InputStateExt},
    theme::{icons::Icon, palette_v2::ThemeExt},
    widgets::GlyphonLabel,
};

pub struct PathSearch {
    searcher: PathSearcher,
    selected: usize,
    activate: bool,
    kb_mode: bool,
    selected_id: Option<lb_rs::Uuid>,
}

impl SearchExecutor for PathSearch {
    fn search_type(&self) -> SearchType {
        SearchType::Path
    }

    fn handle_query(&mut self, query: &str) {
        self.searcher.query(query);
        // Reset selection when query changes
        if self.selected >= self.searcher.results().len() {
            self.selected = 0;
        }
    }

    fn show_result_picker(&mut self, ui: &mut egui::Ui) -> super::PickerResponse {
        self.process_keys(ui.ctx());

        let results = self.searcher.results();
        let n = results.len();

        if n > 0 && self.selected >= n {
            self.selected = n - 1;
        }

        if self.activate {
            self.activate = false;
            let activated = results.get(self.selected).map(|r| r.id);
            return super::PickerResponse { activated, selected: self.selected_id };
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

        egui::ScrollArea::vertical()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show_rows(ui, ROW_HEIGHT, n, |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;

                for index in range {
                    let Some(result) = results.get(index) else { continue };

                    let resp = self.show_result_cell(ui, result, index, index == self.selected);
                    if resp.hovered() {
                        hovered = Some(index);
                    }
                    if resp.clicked() {
                        clicked = Some(index);
                        clicked_id = Some(result.id);
                    }
                }
            });

        if let Some(i) = clicked {
            self.selected = i;
        } else if !self.kb_mode {
            if let Some(i) = hovered {
                self.selected = i;
            }
        }

        let new_id = results.get(self.selected).map(|r| r.id);
        if new_id != self.selected_id {
            self.selected_id = new_id;
        }

        super::PickerResponse { activated: clicked_id, selected: self.selected_id }
    }

    fn show_preview(&mut self, ui: &mut egui::Ui, _tab: Option<&mut crate::tab::Tab>) {
        ui.spinner();
    }
}

impl PathSearch {
    pub fn new(lb: &Lb, _ctx: &Context) -> Self {
        Self {
            searcher: lb.path_searcher(),
            selected: 0,
            activate: false,
            kb_mode: true,
            selected_id: None,
        }
    }

    fn process_keys(&mut self, ctx: &Context) {
        const NUM_KEYS: [Key; 9] = [
            Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5,
            Key::Num6, Key::Num7, Key::Num8, Key::Num9,
        ];

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

        if ctx.input(|i| i.pointer.delta().length_sq() > 0.0) {
            self.kb_mode = false;
        }
    }

    fn show_result_cell(
        &self, ui: &mut Ui, result: &SearchResult, index: usize, selected: bool,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let name_color = theme.neutral_fg();
        let parent_color = theme.neutral_fg_secondary();

        // Path indices are relative to full path; compute offset for filename
        let parent_char_len = result.parent_path.chars().count() as u32
            + if result.parent_path.is_empty() { 0 } else { 1 }; // +1 for the '/'

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
                let is_folder = result.filename.is_empty()
                    || !result.filename.contains('.');

                let (icon, icon_color) = if !is_folder {
                    (
                        DocType::from_name(&result.filename)
                            .to_icon()
                            .size(icon_size),
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
                            &result.filename,
                            &result.path_indices,
                            parent_char_len,
                            name_color,
                            16.0,
                        );
                        Self::highlighted_line(
                            ui,
                            &result.parent_path,
                            &result.path_indices,
                            0,
                            parent_color,
                            13.0,
                        );
                    });
                });
            });
        });

        ui.interact(
            inner.response.rect,
            ui.id().with(("search_row", result.id)),
            egui::Sense::click(),
        )
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
