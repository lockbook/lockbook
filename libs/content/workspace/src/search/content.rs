use std::ops::Range;

use egui::{Context, CornerRadius, Frame, Key, Margin, Modifiers, Ui};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::search::{ContentSearcher, SearchResult};

use crate::{
    search::{SearchExecutor, SearchType},
    show::{DocType, InputStateExt},
    theme::{
        icons::Icon,
        palette_v2::{Palette, ThemeExt},
    },
    widgets::GlyphonLabel,
};

pub struct ContentSearch {
    searcher: ContentSearcher,
    submitted_query: String,
    /// Flat index across all visible rows (headers + child highlights).
    selected: usize,
    kb_mode: bool,
    selected_id: Option<Uuid>,
    activate: bool,
    /// When Some, the picker drills into a single file's highlights.
    focused_file: Option<Uuid>,
}

impl ContentSearch {
    pub fn new(lb: &Lb, _ctx: &Context) -> Self {
        ContentSearch {
            searcher: lb.content_searcher(),
            submitted_query: String::new(),
            selected: 0,
            kb_mode: true,
            selected_id: None,
            activate: false,
            focused_file: None,
        }
    }
}

const CHILD_ROW_HEIGHT: f32 = 20.0;
/// Unit slot size for show_rows virtualization. Headers take 2 slots, children/expand take 1.
const ROW_SLOT_HEIGHT: f32 = CHILD_ROW_HEIGHT;
const MAX_CHILDREN: usize = 4;

/// What a flat index points to.
enum FlatEntry {
    Header {
        match_idx: usize,
    },
    Child {
        match_idx: usize,
        highlight_idx: usize,
    },
    /// "Show N more" row for a file with more than MAX_CHILDREN highlights.
    Expand {
        match_idx: usize,
        remaining: usize,
    },
}

impl FlatEntry {
    fn match_idx(&self) -> usize {
        match self {
            FlatEntry::Header { match_idx }
            | FlatEntry::Child { match_idx, .. }
            | FlatEntry::Expand { match_idx, .. } => *match_idx,
        }
    }
    fn highlight_idx(&self) -> Option<usize> {
        match self {
            FlatEntry::Child { highlight_idx, .. } => Some(*highlight_idx),
            _ => None,
        }
    }
}

/// Build the flat index. If `focused` is Some, only include entries for that file (no cap, no expand).
fn build_flat_index(results: &[SearchResult], focused: Option<Uuid>) -> Vec<FlatEntry> {
    let mut entries = Vec::new();
    for (mi, r) in results.iter().enumerate() {
        if let Some(fid) = focused {
            if r.id != fid {
                continue;
            }
            // In focused mode, no header, no cap, no expand.
            for hi in 0..r.content_matches.len() {
                entries.push(FlatEntry::Child { match_idx: mi, highlight_idx: hi });
            }
        } else {
            entries.push(FlatEntry::Header { match_idx: mi });
            let shown = r.content_matches.len().min(MAX_CHILDREN);
            for hi in 0..shown {
                entries.push(FlatEntry::Child { match_idx: mi, highlight_idx: hi });
            }
            if r.content_matches.len() > MAX_CHILDREN {
                entries.push(FlatEntry::Expand {
                    match_idx: mi,
                    remaining: r.content_matches.len() - MAX_CHILDREN,
                });
            }
        }
    }
    entries
}

impl SearchExecutor for ContentSearch {
    fn search_type(&self) -> super::SearchType {
        SearchType::Content
    }

    fn handle_query(&mut self, query: &str) {
        if self.submitted_query == query {
            return;
        }
        self.submitted_query = query.to_string();
        self.searcher.query(query);
        self.selected = 0;
        self.kb_mode = true;
        self.focused_file = None;
    }

    fn show_result_picker(&mut self, ui: &mut egui::Ui) -> super::PickerResponse {
        self.process_keys(ui.ctx());

        let results = self.searcher.results();

        // If focused_file no longer exists in matches, clear focus.
        if let Some(fid) = self.focused_file {
            if !results.iter().any(|r| r.id == fid) {
                self.focused_file = None;
            }
        }

        let flat = build_flat_index(results, self.focused_file);
        let total = flat.len();
        if total > 0 && self.selected >= total {
            self.selected = total - 1;
        }

        if self.activate {
            self.activate = false;
            // If the selected row is an Expand row, enter focus mode rather than opening.
            if let Some(FlatEntry::Expand { match_idx, .. }) = flat.get(self.selected) {
                if let Some(r) = results.get(*match_idx) {
                    self.focused_file = Some(r.id);
                    self.selected = 0;
                    self.kb_mode = true;
                    return super::PickerResponse { activated: None, selected: self.selected_id };
                }
            }
            let activated = flat
                .get(self.selected)
                .and_then(|e| results.get(e.match_idx()))
                .map(|r| r.id);
            return super::PickerResponse { activated, selected: self.selected_id };
        }

        // Scrollbar styling matching path search.
        ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
        ui.style_mut().spacing.scroll.floating = true;
        ui.style_mut().spacing.scroll.bar_width *= 2.0;
        ui.spacing_mut().scroll.floating_width = 12.0;
        ui.spacing_mut().scroll.dormant_handle_opacity = 0.5;

        let sel_entry = flat.get(self.selected);
        let sel_match_idx = sel_entry.map(|e| e.match_idx());
        let sel_highlight_idx = sel_entry.and_then(|e| e.highlight_idx());

        let mut hovered_flat: Option<usize> = None;
        let mut clicked_flat: Option<usize> = None;
        let mut expand_clicked: Option<Uuid> = None;
        let mut focused_header_clicked = false;
        let mut selected_group_rect: Option<egui::Rect> = None;

        // Render focused header outside the scroll area.
        if let Some(fid) = self.focused_file {
            if let Some(r) = results.iter().find(|r| r.id == fid) {
                let resp = self.show_focused_header(ui, r);
                if resp.clicked() {
                    focused_header_clicked = true;
                }
            }
        }

        // Subtle metrics bar above results (skip when focused to avoid clutter).
        if self.focused_file.is_none() && !results.is_empty() {
            self.show_metrics_bar(ui, results);
        }

        // Empty states: no query typed, or query with no matches.
        if flat.is_empty() {
            self.show_empty_state(ui, results.is_empty());
            return super::PickerResponse { activated: None, selected: self.selected_id };
        }

        egui::ScrollArea::vertical()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show_rows(
                ui,
                ROW_SLOT_HEIGHT,
                {
                    // Header = 2 slots, child/expand = 1 slot.
                    flat.iter()
                        .map(|e| if matches!(e, FlatEntry::Header { .. }) { 2 } else { 1 })
                        .sum()
                },
                |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;

                    // Map the slot range back to flat-entry indices.
                    let mut slot_cursor = 0usize;
                    let mut first_fi = flat.len();
                    for (fi, e) in flat.iter().enumerate() {
                        if slot_cursor >= range.start {
                            first_fi = fi;
                            break;
                        }
                        slot_cursor += if matches!(e, FlatEntry::Header { .. }) { 2 } else { 1 };
                    }

                    for fi in first_fi..flat.len() {
                        if slot_cursor >= range.end {
                            break;
                        }
                        let entry = &flat[fi];
                        slot_cursor +=
                            if matches!(entry, FlatEntry::Header { .. }) { 2 } else { 1 };
                        let Some(r) = results.get(entry.match_idx()) else {
                            continue;
                        };

                        let is_selected_result = sel_match_idx == Some(entry.match_idx());

                        let resp = match entry {
                            FlatEntry::Header { .. } => {
                                self.show_header_row(ui, r, is_selected_result)
                            }
                            FlatEntry::Child { highlight_idx, .. } => {
                                let is_active =
                                    is_selected_result && sel_highlight_idx == Some(*highlight_idx);
                                self.show_child_row(ui, r, *highlight_idx, is_active)
                            }
                            FlatEntry::Expand { remaining, .. } => {
                                let is_active = self.selected == fi;
                                let resp = self.show_expand_row(ui, r, *remaining, is_active);
                                if resp.clicked() {
                                    expand_clicked = Some(r.id);
                                }
                                resp
                            }
                        };

                        if is_selected_result {
                            selected_group_rect = Some(match selected_group_rect {
                                Some(rect) => rect.union(resp.rect),
                                None => resp.rect,
                            });
                        }

                        if resp.hovered() {
                            hovered_flat = Some(fi);
                        }
                        if resp.clicked() {
                            clicked_flat = Some(fi);
                        }
                    }

                    // Paint the group frame after all rows have been laid out so it
                    // spans the header + children + expand row.
                    if let Some(rect) = selected_group_rect {
                        let theme = ui.ctx().get_lb_theme();
                        let stroke = egui::Stroke::new(
                            1.0,
                            theme.neutral_fg_secondary().linear_multiply(0.3),
                        );
                        // Inset the right edge so it doesn't overlap the scrollbar.
                        let rect = egui::Rect::from_min_max(
                            rect.min,
                            egui::pos2(rect.max.x - 20.0, rect.max.y),
                        );
                        ui.painter().rect_stroke(
                            rect,
                            CornerRadius::same(4),
                            stroke,
                            egui::StrokeKind::Middle,
                        );
                    }
                },
            );

        // Handle focused header click (exit focus mode).
        if focused_header_clicked {
            self.focused_file = None;
            self.selected = 0;
            self.kb_mode = true;
        }

        // Handle expand click (enter focus mode).
        if let Some(fid) = expand_clicked {
            self.focused_file = Some(fid);
            self.selected = 0;
            self.kb_mode = true;
        } else if let Some(i) = clicked_flat {
            // Apply mouse-driven selection.
            self.selected = i;
            self.kb_mode = false;
        } else if !self.kb_mode {
            if let Some(i) = hovered_flat {
                self.selected = i;
            }
        }

        // Derive selected_id from the current flat selection.
        let new_id = flat
            .get(self.selected)
            .and_then(|e| results.get(e.match_idx()))
            .map(|r| r.id);
        self.selected_id = new_id;

        super::PickerResponse { activated: None, selected: self.selected_id }
    }

    fn show_preview(&mut self, ui: &mut egui::Ui, _tab: Option<&mut crate::tab::Tab>) {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
    }
}

impl ContentSearch {
    fn process_keys(&mut self, ctx: &Context) {
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
            if self.focused_file.is_some() && i.consume_key_exact(Modifiers::NONE, Key::Escape) {
                self.focused_file = None;
                self.selected = 0;
                self.kb_mode = true;
            }
        });

        if ctx.input(|i| i.pointer.delta().length_sq() > 0.0) {
            self.kb_mode = false;
        }
    }

    fn show_header_row(
        &self, ui: &mut Ui, result: &SearchResult, _selected: bool,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let name_color = theme.neutral_fg();
        let parent_color = theme.neutral_fg_secondary();
        let variant = theme.fg();

        let exact_matches = result.content_matches.iter().filter(|m| m.exact).count();
        let substring_matches = result.content_matches.len() - exact_matches;

        let frame = Frame::new()
            .inner_margin(Margin { left: 8, right: 8, top: 3, bottom: 3 })
            .outer_margin(Margin { left: 0, right: 20, top: 0, bottom: 0 })
            .corner_radius(CornerRadius::same(4));

        let inner = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.set_min_height(16.0 * 1.3 + 13.0 * 1.3);

                let icon_size = 19.;
                DocType::from_name(&result.filename)
                    .to_icon()
                    .size(icon_size)
                    .color(theme.neutral_fg_secondary())
                    .show(ui);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;

                    let badge_size = egui::vec2(22.0, 16.0);

                    if substring_matches > 0 {
                        let bg = variant.get_color(Palette::Magenta).linear_multiply(0.15);
                        let fg = variant.get_color(Palette::Magenta);
                        ui.allocate_ui(badge_size, |ui| {
                            Frame::new()
                                .inner_margin(Margin { left: 5, right: 5, top: 1, bottom: 1 })
                                .corner_radius(CornerRadius::same(3))
                                .fill(bg)
                                .show(ui, |ui| {
                                    ui.add(
                                        GlyphonLabel::new(
                                            &format!("{}", substring_matches),
                                            fg,
                                        )
                                        .font_size(11.0),
                                    );
                                });
                        });
                    }

                    if exact_matches > 0 {
                        let bg = variant.get_color(Palette::Blue).linear_multiply(0.15);
                        let fg = variant.get_color(Palette::Blue);
                        ui.allocate_ui(badge_size, |ui| {
                            Frame::new()
                                .inner_margin(Margin { left: 5, right: 5, top: 1, bottom: 1 })
                                .corner_radius(CornerRadius::same(3))
                                .fill(bg)
                                .show(ui, |ui| {
                                    ui.add(
                                        GlyphonLabel::new(
                                            &format!("{}", exact_matches),
                                            fg,
                                        )
                                        .font_size(11.0),
                                    );
                                });
                        });
                    }

                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.add(
                            GlyphonLabel::new(&result.filename, name_color)
                                .font_size(16.0)
                                .max_width(ui.available_width()),
                        );
                        ui.add(
                            GlyphonLabel::new(&result.parent_path, parent_color)
                                .font_size(13.0)
                                .max_width(ui.available_width()),
                        );
                    });
                });
            });
        });

        ui.interact(
            inner.response.rect,
            ui.id().with(("content_header", result.id)),
            egui::Sense::click(),
        )
    }

    fn show_child_row(
        &self, ui: &mut Ui, result: &SearchResult, hi: usize, is_active: bool,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let parent_color = theme.neutral_fg_secondary();
        let variant = theme.fg();

        let highlight = &result.content_matches[hi];
        let snippet = self.extract_snippet(result.id, &highlight.range);

        let (badge_bg, badge_fg) = if highlight.exact {
            (
                variant.get_color(Palette::Blue).linear_multiply(0.15),
                variant.get_color(Palette::Blue),
            )
        } else {
            (
                variant.get_color(Palette::Magenta).linear_multiply(0.15),
                variant.get_color(Palette::Magenta),
            )
        };
        let label = if highlight.exact { "exact" } else { "partial" };

        let mut child_frame = Frame::new()
            .outer_margin(Margin { left: 14, right: 20, top: 1, bottom: 1 })
            .inner_margin(Margin { left: 14, right: 10, top: 2, bottom: 2 })
            .corner_radius(CornerRadius::same(4));
        if is_active {
            child_frame = child_frame.fill(theme.neutral_bg_tertiary());
        }

        let cf = child_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.set_height(CHILD_ROW_HEIGHT - 4.0);

                Frame::new()
                    .inner_margin(Margin { left: 4, right: 4, top: 1, bottom: 1 })
                    .corner_radius(CornerRadius::same(3))
                    .fill(badge_bg)
                    .show(ui, |ui| {
                        ui.add(GlyphonLabel::new(label, badge_fg).font_size(10.0));
                    });

                let max_w = ui.available_width();
                ui.add(
                    GlyphonLabel::new_rich(
                        snippet
                            .iter()
                            .map(|(t, b)| (t.as_str(), *b))
                            .collect(),
                        parent_color,
                    )
                    .font_size(12.0)
                    .max_width(max_w),
                );
            });
        });

        ui.interact(
            cf.response.rect,
            ui.id().with(("content_child", result.id, hi)),
            egui::Sense::click(),
        )
    }

    fn show_expand_row(
        &self, ui: &mut Ui, result: &SearchResult, remaining: usize, is_active: bool,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let variant = theme.fg();
        let fg = variant.get_color(Palette::Cyan);

        let mut frame = Frame::new()
            .outer_margin(Margin { left: 14, right: 20, top: 1, bottom: 1 })
            .inner_margin(Margin { left: 14, right: 10, top: 2, bottom: 2 })
            .corner_radius(CornerRadius::same(4));
        if is_active {
            frame = frame.fill(theme.neutral_bg_tertiary());
        }

        let cf = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(CHILD_ROW_HEIGHT - 4.0);
                ui.add(
                    GlyphonLabel::new(
                        &format!(
                            "Show {} more match{}…",
                            remaining,
                            if remaining == 1 { "" } else { "es" }
                        ),
                        fg,
                    )
                    .font_size(12.0),
                );
            });
        });

        ui.interact(
            cf.response.rect,
            ui.id().with(("content_expand", result.id)),
            egui::Sense::click(),
        )
    }

    fn show_metrics_bar(&self, ui: &mut Ui, results: &[SearchResult]) {
        let theme = ui.ctx().get_lb_theme();
        let muted = theme.neutral_fg_secondary();
        let variant = theme.fg();

        let file_count = results.len();
        let total_highlights: usize = results.iter().map(|r| r.content_matches.len()).sum();
        let total_exact: usize = results
            .iter()
            .flat_map(|r| &r.content_matches)
            .filter(|m| m.exact)
            .count();
        let total_partial = total_highlights - total_exact;

        Frame::new()
            .inner_margin(Margin { left: 10, right: 24, top: 2, bottom: 4 })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;

                    // Results count.
                    ui.add(
                        GlyphonLabel::new(
                            &format!(
                                "{} file{}",
                                file_count,
                                if file_count == 1 { "" } else { "s" }
                            ),
                            muted,
                        )
                        .font_size(11.0),
                    );

                    ui.add(GlyphonLabel::new("·", muted).font_size(11.0));

                    // Total matches with colored mini-badges.
                    if total_exact > 0 {
                        ui.add(
                            GlyphonLabel::new(
                                &format!("{} exact", total_exact),
                                variant.get_color(Palette::Blue),
                            )
                            .font_size(11.0),
                        );
                    }
                    if total_partial > 0 {
                        ui.add(
                            GlyphonLabel::new(
                                &format!("{} partial", total_partial),
                                variant.get_color(Palette::Magenta),
                            )
                            .font_size(11.0),
                        );
                    }
                    if total_exact == 0 && total_partial == 0 {
                        ui.add(
                            GlyphonLabel::new(&format!("{} matches", total_highlights), muted)
                                .font_size(11.0),
                        );
                    }

                    // Index build time.
                    let build_ms = self.searcher.build_time().as_millis();
                    if build_ms > 0 {
                        ui.add(GlyphonLabel::new("·", muted).font_size(11.0));
                        ui.add(
                            GlyphonLabel::new(
                                &format!("indexed in {} ms", build_ms),
                                muted,
                            )
                            .font_size(11.0),
                        );
                    }
                });
            });
    }

    fn show_empty_state(&self, ui: &mut Ui, no_results: bool) {
        let theme = ui.ctx().get_lb_theme();
        let muted = theme.neutral_fg_secondary();
        let variant = theme.fg();

        let (title, subtitle, icon_color): (&str, &str, _) = if no_results {
            (
                "Search your notes",
                "Start typing to find matches",
                variant.get_color(Palette::Blue),
            )
        } else {
            ("No matches", "Try a different query or shorter words", muted)
        };

        // Fill the available region so the pane doesn't collapse to 0 width.
        let rect = ui.available_rect_before_wrap();
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    Icon::SEARCH.size(42.0).color(icon_color).show(ui);
                    ui.add_space(14.0);
                    ui.add(GlyphonLabel::new(title, theme.neutral_fg()).font_size(18.0));
                    ui.add_space(6.0);
                    ui.add(GlyphonLabel::new(subtitle, muted).font_size(13.0));
                });
            });
        });
    }

    fn show_focused_header(&self, ui: &mut Ui, result: &SearchResult) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let name_color = theme.neutral_fg();
        let parent_color = theme.neutral_fg_secondary();

        let total = result.content_matches.len();

        let frame = Frame::new()
            .inner_margin(Margin { left: 8, right: 8, top: 6, bottom: 6 })
            .outer_margin(Margin { left: 0, right: 20, top: 0, bottom: 4 })
            .corner_radius(CornerRadius::same(4))
            .fill(theme.neutral_bg_secondary());

        let inner = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                // Back arrow icon.
                Icon::ARROW_LEFT.size(14.0).color(parent_color).show(ui);

                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add(
                        GlyphonLabel::new(
                            &format!(
                                "{} — {} match{}",
                                result.filename,
                                total,
                                if total == 1 { "" } else { "es" }
                            ),
                            name_color,
                        )
                        .font_size(14.0)
                        .max_width(ui.available_width()),
                    );
                    ui.add(
                        GlyphonLabel::new(
                            &format!("Back to all results · {}", result.parent_path),
                            parent_color,
                        )
                        .font_size(11.0)
                        .max_width(ui.available_width()),
                    );
                });
            });
        });

        ui.interact(
            inner.response.rect,
            ui.id().with(("focused_header", result.id)),
            egui::Sense::click(),
        )
    }

    fn extract_snippet(&self, id: Uuid, range: &Range<usize>) -> Vec<(String, bool)> {
        let Some((prefix, matched, suffix)) = self.searcher.snippet(id, range, 30) else {
            return vec![("...".to_string(), false)];
        };

        let clean = |s: &str| -> String {
            s.chars()
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                .collect()
        };

        let mut spans = Vec::new();

        let pre = clean(prefix);
        if !pre.is_empty() {
            spans.push((pre, false));
        }
        let mat = clean(matched);
        if !mat.is_empty() {
            spans.push((mat, true));
        }
        let suf = clean(suffix);
        if !suf.is_empty() {
            spans.push((suf, false));
        }

        if spans.is_empty() {
            spans.push(("...".to_string(), false));
        }

        spans
    }
}
