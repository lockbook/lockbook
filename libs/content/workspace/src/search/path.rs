pub struct PathSearch {
    submitted_query: String,
    nucleo: Nucleo<PathResult>,
    selected: usize,
    /// Set by input handling when a row shortcut is pressed. Resolved to a
    /// file id in `show_result_picker` where the snapshot is available.
    activate: bool,
    /// True after arrow-key navigation; hover is ignored so the mouse doesn't
    /// fight the keyboard. Cleared as soon as the pointer moves.
    kb_mode: bool,
    selected_id: Option<Uuid>,
}

impl SearchExecutor for PathSearch {
    fn search_type(&self) -> super::SearchType {
        SearchType::Path
    }

    fn handle_query(&mut self, query: &str) {
        if self.submitted_query != query {
            self.nucleo.pattern.reparse(
                0,
                query,
                CaseMatching::Smart,
                Normalization::Smart,
                self.submitted_query.starts_with(query),
            );
            self.submitted_query = query.to_string();
            self.selected = 0;
            self.kb_mode = true;
        }
        self.nucleo.tick(1);
    }

    fn show_result_picker(&mut self, ui: &mut egui::Ui) -> super::PickerResponse {
        self.process_keys(ui.ctx());

        // Phase 1: clamp the (possibly over-incremented) selection, and if a
        // keyboard shortcut activated a row, return that id immediately — the
        // caller is about to dismiss the modal, so skip rendering.
        let snapshot = self.nucleo.snapshot();
        let n = snapshot.matched_item_count() as usize;
        if n > 0 && self.selected >= n {
            self.selected = n - 1;
        }
        if self.activate {
            self.activate = false;
            let activated = snapshot
                .get_matched_item(self.selected as u32)
                .map(|it| it.data.file.id);
            return super::PickerResponse { activated, selected: self.selected_id };
        }

        // Phase 2: render only the visible rows via `ScrollArea::show_rows`,
        // collecting mouse input into locals (self can't be mutated while the
        // snapshot borrow is alive).
        let mut hovered: Option<usize> = None;
        let mut clicked: Option<usize> = None;
        let mut clicked_id: Option<lb_rs::Uuid> = None;

        // Row height = two text line-heights plus the Frame's vertical margin.
        const ROW_HEIGHT: f32 = 16.0 * 1.3 + 13.0 * 1.3 + 6.0;

        // Scrollbar styling copied from `show_tree` in clients/egui: a solid
        // floating bar that's always visible, wider than the default.
        ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
        ui.style_mut().spacing.scroll.floating = true;
        ui.style_mut().spacing.scroll.bar_width *= 2.0;
        ui.spacing_mut().scroll.floating_width = 12.0;
        ui.spacing_mut().scroll.dormant_handle_opacity = 0.5;

        egui::ScrollArea::vertical()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show_rows(ui, ROW_HEIGHT, n, |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let mut matcher = Matcher::new(nucleo::Config::DEFAULT);

                for index in range {
                    let Some(item) = snapshot.get_matched_item(index as u32) else { continue };

                    let mut entry = item.data.clone();
                    let mut indices = Vec::new();
                    self.nucleo.pattern.column_pattern(0).indices(
                        item.matcher_columns[0].slice(..),
                        &mut matcher,
                        &mut indices,
                    );
                    entry.highlight = indices;

                    let resp = self.show_result_cell(ui, &entry, index, index == self.selected);
                    if resp.hovered() {
                        hovered = Some(index);
                    }
                    if resp.clicked() {
                        clicked = Some(index);
                        clicked_id = Some(item.data.file.id);
                    }
                }
            });

        // Phase 3: apply mouse-driven selection now that the snapshot is gone.
        // In keyboard mode hover is ignored, but explicit clicks always apply.
        if let Some(i) = clicked {
            self.selected = i;
        } else if !self.kb_mode {
            if let Some(i) = hovered {
                self.selected = i;
            }
        }

        {
            let snapshot = self.nucleo.snapshot();
            let new_id = snapshot
                .get_matched_item(self.selected as u32)
                .map(|item| item.data.file.id);
            if new_id != self.selected_id {
                self.selected_id = new_id;
            }
        }

        super::PickerResponse { activated: clicked_id, selected: self.selected_id }
    }

    fn show_preview(&mut self, ui: &mut egui::Ui, tab: Option<&mut crate::tab::Tab>) {
        ui.spinner();
    }
}

impl PathSearch {
    pub fn new(lb: &Lb, ctx: &Context) -> Self {
        let metas = lb.list_metadatas().unwrap();
        // todo there may be gains to be had to retrieve FilePaths instead of id paths
        let mut id_paths = lb.list_paths_with_ids(None).unwrap();
        id_paths.retain(|(_, path)| path != "/");

        let ctx_clone = ctx.clone();
        let notify = Arc::new(move || {
            ctx_clone.request_repaint();
        });

        let nucleo = Nucleo::new(nucleo::Config::DEFAULT, notify, None, 1);
        let injector = nucleo.injector();

        for (id, path) in id_paths {
            injector.push(
                PathResult {
                    file: metas.iter().find(|m| m.id == id).unwrap().clone(),
                    path: path.clone(),
                    highlight: vec![],
                },
                |e, cols| {
                    cols[0] = e.path.as_str().into();
                },
            );
        }

        Self {
            submitted_query: Default::default(),
            nucleo,
            selected: 0,
            activate: false,
            kb_mode: true,
            selected_id: None,
        }
    }

    /// Keyboard input for the path picker. Only touches `self.selected` and
    /// `self.activate`; snapshot-based clamping happens in `show_result_picker`.
    fn process_keys(&mut self, ctx: &Context) {
        // ⌘1..⌘9 (Ctrl on Linux/Windows) jumps to and activates that row.
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

        // Any pointer movement hands control back to the mouse.
        if ctx.input(|i| i.pointer.delta().length_sq() > 0.0) {
            self.kb_mode = false;
        }
    }

    fn show_result_cell(
        &self, ui: &mut Ui, entry: &PathResult, index: usize, selected: bool,
    ) -> egui::Response {
        // functionality:
        // todo: support folders, and generally a richer icon experience
        let theme = ui.ctx().get_lb_theme();
        let name_color = theme.neutral_fg();
        let parent_color = theme.neutral_fg_secondary();

        // nucleo returns char indices into the full path; pass a char offset
        // so each sub-line filters the shared slice without allocating.
        let parent_path = entry.parent_path();
        let parent_char_len = parent_path.chars().count() as u32;

        // Extra right padding so the ⌘N hint stays clear of the scrollbar.
        let mut frame = Frame::new()
            .inner_margin(Margin { left: 8, right: 8, top: 3, bottom: 3 })
            .outer_margin(Margin { left: 0, right: 20, top: 0, bottom: 0 })
            .corner_radius(CornerRadius::same(4));
        if selected {
            // Use a subtle neutral fill rather than the accent selection color
            // so the text colors still contrast cleanly.
            frame = frame.fill(theme.neutral_bg_tertiary());
        }
        let inner = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                // establish the row height up front so Align::Center
                // actually centers the icon against the two lines of text.
                // line heights are ~size * 1.3 + inter-line spacing.
                ui.set_min_height(16.0 * 1.3 + 13.0 * 1.3);

                let icon_size = 19.;
                let (icon, icon_color) = if entry.file.is_document() {
                    (
                        DocType::from_name(&entry.file.name)
                            .to_icon()
                            .size(icon_size),
                        theme.neutral_fg_secondary(),
                    )
                } else {
                    let is_shared = !entry.file.shares.is_empty();
                    let icon =
                        if is_shared { Icon::SHARED_FOLDER } else { Icon::FOLDER }.size(icon_size);
                    let color = if is_shared {
                        theme.fg().get_color(theme.prefs().secondary)
                    } else {
                        theme.fg().get_color(theme.prefs().primary)
                    };
                    (icon, color)
                };
                icon.color(icon_color).show(ui);

                // Reserve the keycaps on the right first (right_to_left)
                // so the text block in the middle gets the remaining width.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    // Only the first 9 rows get a ⌘N / CtrlN shortcut.
                    if index < 9 {
                        let modifier = if cfg!(any(target_os = "macos", target_os = "ios")) {
                            "⌘"
                        } else {
                            "Ctrl"
                        };
                        // laid out right-to-left, so the number (rightmost) draws first.
                        let number = (index + 1).to_string();
                        for glyph in [number.as_str(), modifier] {
                            ui.add(GlyphonLabel::new(glyph, parent_color).font_size(12.0));
                        }
                    }

                    // Remaining width goes to the text block.
                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        Self::highlighted_line(
                            ui,
                            &entry.file.name,
                            &entry.highlight,
                            parent_char_len,
                            name_color,
                            16.0,
                        );
                        Self::highlighted_line(
                            ui,
                            parent_path,
                            &entry.highlight,
                            0,
                            parent_color,
                            13.0,
                        );
                    });
                });
            });
        });
        // Promote the frame's allocated rect to a click+hover surface.
        ui.interact(
            inner.response.rect,
            ui.id().with(("search_row", entry.file.id)),
            egui::Sense::click(),
        )
    }

    /// Render `text` as a single laid-out line, bolding any character whose
    /// char-index (plus `char_offset`) appears in `highlights`. Background
    /// color is reserved for the current selection.
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

#[derive(Clone)]
pub struct PathResult {
    file: File,
    path: String,
    highlight: Vec<u32>,
}

impl PathResult {
    fn parent_path(&self) -> &str {
        if self.path.ends_with('/') {
            self.path
                .strip_suffix(&format!("{}/", self.file.name))
                .unwrap()
        } else {
            self.path.strip_suffix(&self.file.name).unwrap()
        }
    }
}

use std::sync::Arc;

use egui::{Context, CornerRadius, Frame, Key, Margin, Modifiers, Ui};
use lb_rs::{Uuid, blocking::Lb, model::file::File};
use nucleo::{
    Matcher, Nucleo,
    pattern::{CaseMatching, Normalization},
};

use crate::{
    search::{SearchExecutor, SearchType},
    show::{DocType, InputStateExt},
    theme::{icons::Icon, palette_v2::ThemeExt},
    widgets::GlyphonLabel,
};
