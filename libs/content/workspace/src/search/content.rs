#[derive(Clone)]
pub struct ContentSearch {
    doc_store: Arc<RwLock<DocStore>>,
    query_state: Arc<RwLock<QueryState>>,
    /// Flat index across all visible rows (headers + child highlights).
    selected: usize,
    kb_mode: bool,
    selected_id: Option<Uuid>,
    activate: bool,
    /// When Some, the picker drills into a single file's highlights.
    focused_file: Option<Uuid>,
    /// Render time of the previous frame, in microseconds.
    last_render_us: u128,
}

impl ContentSearch {
    pub fn new(lb: &Lb, ctx: &Context) -> Self {
        let content_search = ContentSearch {
            doc_store: Default::default(),
            query_state: Default::default(),
            selected: 0,
            kb_mode: true,
            selected_id: None,
            activate: false,
            focused_file: None,
            last_render_us: 0,
        };

        let lb = lb.clone();
        let ctx = ctx.clone();
        let bg_cs = content_search.clone();
        thread::spawn(move || {
            bg_cs.build_doc_store(lb, ctx);
        });

        content_search
    }
}

#[derive(Default)]
pub struct DocStore {
    documents: Vec<(File, String, String)>,

    uningested_files: Vec<File>,
    ignored_ids: usize,
    ingest_failures: usize,

    start_time: Option<Instant>,
    end_time: Option<Instant>,
}

impl ContentSearch {
    fn build_doc_store(&self, lb: Lb, ctx: Context) {
        let start = Instant::now();

        let metas = lb.list_metadatas().unwrap();
        let paths = Arc::new(lb.list_paths_with_ids(None).unwrap());

        self.doc_store.write().unwrap().start_time = Some(start);

        for meta in metas {
            let mut ignore = false;
            if !meta.is_document() {
                continue;
            }

            if !meta.name.ends_with(".md") {
                ignore = true;
            }

            if ignore {
                self.doc_store.write().unwrap().ignored_ids += 1;
            } else {
                self.doc_store.write().unwrap().uningested_files.push(meta);
            }
        }

        for _ in 0..available_parallelism()
            .map(|number| number.get())
            .unwrap_or(4)
        {
            let bg_ds = self.doc_store.clone();
            let bg_lb = lb.clone();
            let ctx = ctx.clone();
            let bg_paths = paths.clone();
            thread::spawn(move || {
                loop {
                    let Some(meta) = bg_ds.write().unwrap().uningested_files.pop() else {
                        return;
                    };

                    let id = meta.id;
                    let doc = bg_lb
                        .read_document(meta.id, false)
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok());

                    let mut doc_store = bg_ds.write().unwrap();
                    if let Some(doc) = doc {
                        // todo: see lowercasing notes
                        let doc = doc.to_lowercase();
                        doc_store.documents.push((
                            meta,
                            bg_paths.iter().find(|(i, _)| *i == id).unwrap().1.clone(),
                            doc,
                        ));
                    } else {
                        doc_store.ingest_failures += 1;
                    }

                    if doc_store.uningested_files.is_empty() {
                        doc_store.end_time = Some(Instant::now());
                    }
                    ctx.request_repaint();
                }
            });
        }
    }
}

#[derive(Default)]
pub struct QueryState {
    submitted_query: String,
    ellapsed_ms: u128,
    matches: Vec<Matches>,
}

#[derive(Default)]
pub struct Matches {
    id: Uuid,
    highlights: Vec<Highlight>,

    exact_matches: u32,
    substring_matches: u32,
}

pub struct Highlight {
    range: Range<usize>,
    exact: bool,
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
fn build_flat_index(matches: &[Matches], focused: Option<Uuid>) -> Vec<FlatEntry> {
    let mut entries = Vec::new();
    for (mi, m) in matches.iter().enumerate() {
        if let Some(fid) = focused {
            if m.id != fid {
                continue;
            }
            // In focused mode, no header, no cap, no expand.
            for hi in 0..m.highlights.len() {
                entries.push(FlatEntry::Child { match_idx: mi, highlight_idx: hi });
            }
        } else {
            entries.push(FlatEntry::Header { match_idx: mi });
            let shown = m.highlights.len().min(MAX_CHILDREN);
            for hi in 0..shown {
                entries.push(FlatEntry::Child { match_idx: mi, highlight_idx: hi });
            }
            if m.highlights.len() > MAX_CHILDREN {
                entries.push(FlatEntry::Expand {
                    match_idx: mi,
                    remaining: m.highlights.len() - MAX_CHILDREN,
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
        // todo: expose lowercase controls. Right now lowercasing is done through a path of least
        // resistance. We lowercase the documents during ingestion (in place) which has a minimal
        // impact on performance. Doing it within this function makes the query take 2x as long.
        // the optimal solution would use a FSM and have controls for managing cases.
        //
        // also there seem to be some fancy algorithms in the space and it could be worth exploring
        // them as well
        let query = query.to_ascii_lowercase();
        let start = Instant::now();

        let docs = self.doc_store.read().unwrap();
        let mut results = self.query_state.write().unwrap();
        if results.submitted_query == query {
            return;
        }

        results.submitted_query = query.to_string();

        // todo: incrementalism
        results.matches.clear();

        if query.is_empty() {
            return;
        }

        for (meta, _, content) in &docs.documents {
            let mut m = Matches { id: meta.id, ..Default::default() };

            for (idx, _) in content.match_indices(&query) {
                m.highlights
                    .push(Highlight { range: idx..idx + query.len(), exact: true });
                m.exact_matches += 1;
            }

            let mut all_words_matched = true;
            for sub_query in query.split_whitespace() {
                let mut sub_query_matched = false;
                for (idx, _) in content.match_indices(sub_query) {
                    sub_query_matched = true;
                    if m.highlights.iter().any(|h| h.range.contains(&idx)) {
                        continue;
                    }
                    m.highlights
                        .push(Highlight { range: idx..idx + sub_query.len(), exact: false });
                    m.substring_matches += 1;
                }
                if !sub_query_matched {
                    all_words_matched = false;
                }
            }

            if all_words_matched {
                results.matches.push(m);
            }
        }

        results.matches.sort_unstable_by(|a, b| {
            if a.exact_matches > 0 || b.exact_matches > 0 {
                b.exact_matches.cmp(&a.exact_matches)
            } else {
                b.substring_matches.cmp(&a.substring_matches)
            }
        });

        results.ellapsed_ms = start.elapsed().as_millis();

        // Reset selection and focus when query changes.
        drop(results);
        drop(docs);
        self.selected = 0;
        self.kb_mode = true;
        self.focused_file = None;
    }

    fn show_result_picker(&mut self, ui: &mut egui::Ui) -> super::PickerResponse {
        let render_start = Instant::now();
        self.process_keys(ui.ctx());

        let doc_store = self.doc_store.read().unwrap();
        let query_state = self.query_state.read().unwrap();

        // If focused_file no longer exists in matches, clear focus.
        if let Some(fid) = self.focused_file {
            if !query_state.matches.iter().any(|m| m.id == fid) {
                self.focused_file = None;
            }
        }

        let flat = build_flat_index(&query_state.matches, self.focused_file);
        let total = flat.len();
        if total > 0 && self.selected >= total {
            self.selected = total - 1;
        }

        if self.activate {
            self.activate = false;
            // If the selected row is an Expand row, enter focus mode rather than opening.
            if let Some(FlatEntry::Expand { match_idx, .. }) = flat.get(self.selected) {
                if let Some(m) = query_state.matches.get(*match_idx) {
                    self.focused_file = Some(m.id);
                    self.selected = 0;
                    self.kb_mode = true;
                    self.last_render_us = render_start.elapsed().as_micros();
                    return super::PickerResponse { activated: None, selected: self.selected_id };
                }
            }
            let activated = flat
                .get(self.selected)
                .and_then(|e| query_state.matches.get(e.match_idx()))
                .map(|m| m.id);
            self.last_render_us = render_start.elapsed().as_micros();
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
            if let Some(m) = query_state.matches.iter().find(|m| m.id == fid) {
                if let Some((file, path, _)) =
                    doc_store.documents.iter().find(|(f, _, _)| f.id == fid)
                {
                    let resp = self.show_focused_header(ui, file, path, m);
                    if resp.clicked() {
                        focused_header_clicked = true;
                    }
                }
            }
        }

        // Subtle metrics bar above results (skip when focused to avoid clutter).
        if self.focused_file.is_none() && !query_state.submitted_query.is_empty() {
            self.show_metrics_bar(ui, &query_state, &doc_store);
        }

        // Empty states: no query typed, or query with no matches.
        if flat.is_empty() {
            self.show_empty_state(
                ui,
                query_state.submitted_query.is_empty(),
                doc_store.end_time.is_none() && !doc_store.uningested_files.is_empty(),
                doc_store.documents.len(),
                doc_store.uningested_files.len(),
            );

            self.last_render_us = render_start.elapsed().as_micros();
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
                        let m = &query_state.matches[entry.match_idx()];
                        let Some((file, path, content)) =
                            doc_store.documents.iter().find(|(f, _, _)| f.id == m.id)
                        else {
                            continue;
                        };

                        let is_selected_result = sel_match_idx == Some(entry.match_idx());

                        let resp = match entry {
                            FlatEntry::Header { .. } => {
                                self.show_header_row(ui, file, path, m, is_selected_result)
                            }
                            FlatEntry::Child { highlight_idx, .. } => {
                                let is_active =
                                    is_selected_result && sel_highlight_idx == Some(*highlight_idx);
                                self.show_child_row(ui, file, content, m, *highlight_idx, is_active)
                            }
                            FlatEntry::Expand { remaining, .. } => {
                                let is_active = self.selected == fi;
                                let r = self.show_expand_row(ui, file, *remaining, is_active);
                                if r.clicked() {
                                    expand_clicked = Some(file.id);
                                }
                                r
                            }
                        };

                        if is_selected_result {
                            selected_group_rect = Some(match selected_group_rect {
                                Some(r) => r.union(resp.rect),
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
            .and_then(|e| query_state.matches.get(e.match_idx()))
            .map(|m| m.id);
        self.selected_id = new_id;

        self.last_render_us = render_start.elapsed().as_micros();
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
        &self, ui: &mut Ui, file: &File, path: &str, matches: &Matches, _selected: bool,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let name_color = theme.neutral_fg();
        let parent_color = theme.neutral_fg_secondary();
        let variant = theme.fg();

        let parent_path = path.strip_suffix(&file.name).unwrap_or(path);

        let frame = Frame::new()
            .inner_margin(Margin { left: 8, right: 8, top: 3, bottom: 3 })
            .outer_margin(Margin { left: 0, right: 20, top: 0, bottom: 0 })
            .corner_radius(CornerRadius::same(4));

        let inner = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.set_min_height(16.0 * 1.3 + 13.0 * 1.3);

                let icon_size = 19.;
                DocType::from_name(&file.name)
                    .to_icon()
                    .size(icon_size)
                    .color(theme.neutral_fg_secondary())
                    .show(ui);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;

                    let badge_size = egui::vec2(22.0, 16.0);

                    if matches.substring_matches > 0 {
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
                                            &format!("{}", matches.substring_matches),
                                            fg,
                                        )
                                        .font_size(11.0),
                                    );
                                });
                        });
                    }

                    if matches.exact_matches > 0 {
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
                                            &format!("{}", matches.exact_matches),
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
                            GlyphonLabel::new(&file.name, name_color)
                                .font_size(16.0)
                                .max_width(ui.available_width()),
                        );
                        ui.add(
                            GlyphonLabel::new(parent_path, parent_color)
                                .font_size(13.0)
                                .max_width(ui.available_width()),
                        );
                    });
                });
            });
        });

        ui.interact(
            inner.response.rect,
            ui.id().with(("content_header", file.id)),
            egui::Sense::click(),
        )
    }

    fn show_child_row(
        &self, ui: &mut Ui, file: &File, content: &str, matches: &Matches, hi: usize,
        is_active: bool,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let parent_color = theme.neutral_fg_secondary();
        let variant = theme.fg();

        let highlight = &matches.highlights[hi];
        let snippet = extract_context(content, &highlight.range);

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
                            .spans
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
            ui.id().with(("content_child", file.id, hi)),
            egui::Sense::click(),
        )
    }

    fn show_expand_row(
        &self, ui: &mut Ui, file: &File, remaining: usize, is_active: bool,
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
            ui.id().with(("content_expand", file.id)),
            egui::Sense::click(),
        )
    }

    fn show_metrics_bar(&self, ui: &mut Ui, qs: &QueryState, ds: &DocStore) {
        let theme = ui.ctx().get_lb_theme();
        let muted = theme.neutral_fg_secondary();
        let variant = theme.fg();

        let file_count = qs.matches.len();
        let total_highlights: usize = qs.matches.iter().map(|m| m.highlights.len()).sum();
        let total_exact: u32 = qs.matches.iter().map(|m| m.exact_matches).sum();
        let total_partial: u32 = qs.matches.iter().map(|m| m.substring_matches).sum();

        let indexing = ds.end_time.is_none() && !ds.uningested_files.is_empty();

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

                    ui.add(GlyphonLabel::new("·", muted).font_size(11.0));

                    // Query time.
                    ui.add(
                        GlyphonLabel::new(&format!("query {} ms", qs.ellapsed_ms), muted)
                            .font_size(11.0),
                    );

                    // Render time of previous frame.
                    if self.last_render_us > 0 {
                        ui.add(GlyphonLabel::new("·", muted).font_size(11.0));
                        let render_ms = self.last_render_us as f32 / 1000.0;
                        ui.add(
                            GlyphonLabel::new(&format!("render {:.1} ms", render_ms), muted)
                                .font_size(11.0),
                        );
                    }

                    // Indexing indicator.
                    if indexing {
                        ui.add(GlyphonLabel::new("·", muted).font_size(11.0));
                        ui.add(
                            GlyphonLabel::new(
                                &format!("indexing {}…", ds.uningested_files.len()),
                                variant.get_color(Palette::Yellow),
                            )
                            .font_size(11.0),
                        );
                    } else {
                        ui.add(GlyphonLabel::new("·", muted).font_size(11.0));
                        ui.add(
                            GlyphonLabel::new(
                                &format!("{} docs indexed", ds.documents.len()),
                                muted,
                            )
                            .font_size(11.0),
                        );
                    }
                });
            });
    }

    fn show_empty_state(
        &self, ui: &mut Ui, no_query: bool, indexing: bool, doc_count: usize, pending: usize,
    ) {
        let theme = ui.ctx().get_lb_theme();
        let muted = theme.neutral_fg_secondary();
        let variant = theme.fg();

        let (title, subtitle, icon_color): (String, String, _) = if no_query {
            (
                "Search your notes".to_string(),
                if indexing {
                    format!("Indexing {} file{}…", pending, if pending == 1 { "" } else { "s" })
                } else {
                    format!(
                        "{} document{} ready — start typing to find matches",
                        doc_count,
                        if doc_count == 1 { "" } else { "s" }
                    )
                },
                variant.get_color(Palette::Blue),
            )
        } else {
            ("No matches".to_string(), "Try a different query or shorter words".to_string(), muted)
        };

        // Fill the available region so the pane doesn't collapse to 0 width.
        let rect = ui.available_rect_before_wrap();
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    Icon::SEARCH.size(42.0).color(icon_color).show(ui);
                    ui.add_space(14.0);
                    ui.add(GlyphonLabel::new(&title, theme.neutral_fg()).font_size(18.0));
                    ui.add_space(6.0);
                    ui.add(GlyphonLabel::new(&subtitle, muted).font_size(13.0));
                });
            });
        });
    }

    fn show_focused_header(
        &self, ui: &mut Ui, file: &File, path: &str, matches: &Matches,
    ) -> egui::Response {
        let theme = ui.ctx().get_lb_theme();
        let name_color = theme.neutral_fg();
        let parent_color = theme.neutral_fg_secondary();

        let parent_path = path.strip_suffix(&file.name).unwrap_or(path);
        let total = matches.highlights.len();

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
                                file.name,
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
                            &format!("Back to all results · {}", parent_path),
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
            ui.id().with(("focused_header", file.id)),
            egui::Sense::click(),
        )
    }
}

/// A snippet with bold spans for the matched region.
struct Snippet {
    /// (text, is_bold) pairs.
    spans: Vec<(String, bool)>,
}

fn extract_context(content: &str, range: &Range<usize>) -> Snippet {
    // Work in char indices to avoid slicing inside multi-byte characters.
    let char_indices: Vec<(usize, char)> = content.char_indices().collect();

    let match_start_ci = char_indices
        .iter()
        .position(|(byte, _)| *byte >= range.start)
        .unwrap_or(char_indices.len());
    let match_end_ci = char_indices
        .iter()
        .position(|(byte, _)| *byte >= range.end)
        .unwrap_or(char_indices.len());

    let context = 30;
    let start_ci = match_start_ci.saturating_sub(context);
    let end_ci = (match_end_ci + context).min(char_indices.len());

    // Snap start forward to whitespace boundary.
    let mut start = start_ci;
    if start > 0 {
        for i in start..match_start_ci {
            if char_indices[i].1.is_whitespace() {
                start = i + 1;
                break;
            }
        }
    }

    // Snap end back to whitespace boundary.
    let mut end = end_ci;
    if end < char_indices.len() {
        for i in (match_end_ci..end).rev() {
            if char_indices[i].1.is_whitespace() {
                end = i;
                break;
            }
        }
    }

    let get_byte = |ci: usize| {
        char_indices
            .get(ci)
            .map(|(b, _)| *b)
            .unwrap_or(content.len())
    };
    let start_byte = get_byte(start);
    let match_start_byte = get_byte(match_start_ci);
    let match_end_byte = get_byte(match_end_ci);
    let end_byte = get_byte(end);

    let prefix_ellipsis = if start > 0 { "..." } else { "" };
    let suffix_ellipsis = if end < char_indices.len() { "..." } else { "" };

    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect()
    };

    let before = clean(&content[start_byte..match_start_byte]);
    let matched = clean(&content[match_start_byte..match_end_byte]);
    let after = clean(&content[match_end_byte..end_byte]);

    let mut spans = Vec::new();

    let pre = format!("{}{}", prefix_ellipsis, before.trim_start());
    if !pre.is_empty() {
        spans.push((pre, false));
    }
    if !matched.is_empty() {
        spans.push((matched, true));
    }
    let suf = format!("{}{}", after.trim_end(), suffix_ellipsis);
    if !suf.is_empty() {
        spans.push((suf, false));
    }

    Snippet { spans }
}

use std::{
    cmp::min,
    ops::Range,
    sync::{Arc, RwLock},
    thread::{self, available_parallelism},
    time::Instant,
};

use egui::{Context, CornerRadius, Frame, Key, Margin, Modifiers, Ui};
use lb_rs::{Uuid, blocking::Lb, model::file::File};

use crate::{
    search::{SearchExecutor, SearchType},
    show::{DocType, InputStateExt},
    theme::{
        icons::Icon,
        palette_v2::{Palette, ThemeExt},
    },
    widgets::GlyphonLabel,
};
