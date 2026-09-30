use std::collections::HashSet;
use std::ops::Range;

use egui::{Context, Id, Key, Modifiers, Rect, Sense, Ui, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::search::{ContentSearcher, SearchFilter, SearchResult};

use crate::{
    search::{SearchExecutor, SearchType, path},
    show::InputStateExt,
    style::{
        FileRow, RowGeom, Space, ThemeExt, file_row_icon, parent_crumbs, phosphor,
        with_overlay_scroll,
    },
};

pub struct ContentSearch {
    searcher: ContentSearcher,
    submitted_query: String,
    /// Flat index across visible rows (file headers + expanded snippets).
    selected: Option<usize>,
    kb_mode: bool,
    selected_id: Option<Uuid>,
    activate: bool,
    activate_new_tab: bool,
    scoped: bool,
    /// Files whose extra snippets are shown in-place.
    expanded: HashSet<Uuid>,
    activate_nth: Option<usize>,
    /// Scroll the selection into view next frame. Rows off screen aren't
    /// painted, so this can't wait for the row to ask.
    reveal_selected: bool,
}

impl ContentSearch {
    pub fn new(lb: &Lb) -> Self {
        ContentSearch {
            searcher: lb.content_searcher(),
            submitted_query: String::new(),
            selected: None,
            kb_mode: false,
            selected_id: None,
            activate: false,
            activate_new_tab: false,
            scoped: false,
            expanded: HashSet::new(),
            activate_nth: None,
            reveal_selected: false,
        }
    }
}

const FILE_ROW_H: f32 = FileRow::height_for(true);
const CONT_ROW_H: f32 = FileRow::continuation_height();
/// Air after a file's matches so the next file reads as a new group.
const GROUP_GAP: f32 = Space::Sm.pts();
/// Matches shown under a file before “Show N more matches”.
const PREVIEW: usize = 4;

#[derive(Clone, Copy, Debug)]
enum FlatEntry {
    /// File: name + location (same language as filename search). Click opens.
    Header { match_idx: usize },
    /// A content hit hanging under the file (`highlight_idx` into `content_matches`).
    Child { match_idx: usize, highlight_idx: usize },
    /// Remaining hits after [`PREVIEW`]; one click reveals the rest.
    ShowMore { match_idx: usize, count: usize },
    /// Last row of an expanded file; collapses it back to [`PREVIEW`].
    ShowLess { match_idx: usize },
}

impl FlatEntry {
    fn match_idx(self) -> usize {
        match self {
            Self::Header { match_idx }
            | Self::Child { match_idx, .. }
            | Self::ShowMore { match_idx, .. }
            | Self::ShowLess { match_idx } => match_idx,
        }
    }
    fn highlight_idx(self) -> Option<usize> {
        match self {
            Self::Child { highlight_idx, .. } => Some(highlight_idx),
            Self::Header { .. } => Some(0),
            Self::ShowMore { .. } | Self::ShowLess { .. } => None,
        }
    }
    fn height(self) -> f32 {
        match self {
            Self::Header { .. } => FILE_ROW_H,
            Self::Child { .. } | Self::ShowMore { .. } | Self::ShowLess { .. } => CONT_ROW_H,
        }
    }
}

fn gap_after_row(flat: &[FlatEntry], i: usize) -> bool {
    matches!(
        flat[i],
        FlatEntry::Child { .. } | FlatEntry::ShowMore { .. } | FlatEntry::ShowLess { .. }
    ) && matches!(flat.get(i + 1), None | Some(FlatEntry::Header { .. }))
}

/// Expand or collapse a file's matches. Collapsing puts the selection on the
/// file's "Show N more" row, so Enter toggles it back.
fn set_expanded(
    expanded: &mut HashSet<Uuid>, selected: &mut Option<usize>, flat: &[FlatEntry],
    match_idx: usize, id: Uuid, expand: bool,
) {
    if expand {
        expanded.insert(id);
        return;
    }
    expanded.remove(&id);
    let header = flat
        .iter()
        .position(|e| matches!(e, FlatEntry::Header { match_idx: mi } if *mi == match_idx));
    if let Some(h) = header {
        *selected = Some(h + 1 + PREVIEW);
    }
}

fn build_flat_index(results: &[SearchResult], expanded: &HashSet<Uuid>) -> Vec<FlatEntry> {
    let mut entries = Vec::new();
    for (mi, r) in results.iter().enumerate() {
        entries.push(FlatEntry::Header { match_idx: mi });
        let n = r.content_matches.len();
        if n == 0 {
            continue;
        }
        let show = if expanded.contains(&r.id) { n } else { n.min(PREVIEW) };
        for k in 0..show {
            entries.push(FlatEntry::Child { match_idx: mi, highlight_idx: k });
        }
        if show < n {
            entries.push(FlatEntry::ShowMore { match_idx: mi, count: n - show });
        } else if n > PREVIEW {
            entries.push(FlatEntry::ShowLess { match_idx: mi });
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
        self.selected = None;
        self.kb_mode = false;
        self.selected_id = None;
        self.expanded.clear();
    }

    fn update_filter(&mut self, filter: Option<SearchFilter>) {
        self.scoped = filter.is_some();
        self.searcher.update_filter(filter);
        self.selected = None;
        self.selected_id = None;
        self.expanded.clear();
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
        self.process_keys(ui.ctx(), allow_kb_nav);

        let results = self.searcher.results();
        self.expanded
            .retain(|id| results.iter().any(|r| r.id == *id));

        let flat = build_flat_index(results, &self.expanded);
        if !self.submitted_query.is_empty() && !flat.is_empty() && self.selected.is_none() {
            self.selected = Some(0);
            self.kb_mode = true;
            self.reveal_selected = true;
        }

        let total = flat.len();
        if let Some(i) = self.selected {
            if total == 0 {
                self.selected = None;
            } else if i >= total {
                self.selected = Some(total - 1);
            }
        }

        if let Some(n) = self.activate_nth.take() {
            let header = flat
                .iter()
                .enumerate()
                .filter(|(_, e)| matches!(e, FlatEntry::Header { .. }))
                .nth(n)
                .map(|(fi, _)| fi);
            if let Some(fi) = header {
                self.selected = Some(fi);
                self.activate = true;
            }
        }

        if self.activate {
            self.activate = false;
            if let Some(&entry @ (FlatEntry::ShowMore { .. } | FlatEntry::ShowLess { .. })) =
                self.selected.and_then(|i| flat.get(i))
            {
                if let Some(r) = results.get(entry.match_idx()) {
                    let expand = matches!(entry, FlatEntry::ShowMore { .. });
                    set_expanded(
                        &mut self.expanded,
                        &mut self.selected,
                        &flat,
                        entry.match_idx(),
                        r.id,
                        expand,
                    );
                    self.reveal_selected = true;
                }
                self.activate_new_tab = false;
                return super::PickerResponse {
                    activated: None,
                    activated_in_new_tab: false,
                    selected: self.selected_id,
                    selected_range: None,
                    clear_scope: false,
                };
            }
            let activated = self
                .selected
                .and_then(|i| flat.get(i))
                .and_then(|e| results.get(e.match_idx()))
                .map(|r| r.id);
            let selected_range = self.selected.and_then(|i| flat.get(i)).and_then(|e| {
                let r = results.get(e.match_idx())?;
                let hi = e.highlight_idx()?;
                r.content_matches.get(hi).map(|m| m.range.clone())
            });
            let in_new_tab = std::mem::take(&mut self.activate_new_tab);
            return super::PickerResponse {
                activated,
                activated_in_new_tab: in_new_tab,
                selected: self.selected_id,
                selected_range,
                clear_scope: false,
            };
        }

        if flat.is_empty() {
            let clear_scope = self.show_empty_state(ui, scope_name, empty_centered);
            return super::PickerResponse {
                activated: None,
                activated_in_new_tab: false,
                selected: self.selected_id,
                selected_range: None,
                clear_scope,
            };
        }

        let mut hovered_flat: Option<usize> = None;
        let mut clicked_flat: Option<usize> = None;
        let mut clicked_new_tab = false;
        let mut ctx_id: Option<Uuid> = None;
        let mut ctx_new_tab = false;
        let mut toggled: Option<(usize, Uuid, bool)> = None;

        let heights: Vec<f32> = flat
            .iter()
            .enumerate()
            .map(|(i, e)| e.height() + if gap_after_row(&flat, i) { GROUP_GAP } else { 0.0 })
            .collect();
        let geom = RowGeom::from_heights(&heights);
        let highlight = self.selected;
        let reveal = std::mem::take(&mut self.reveal_selected)
            .then_some(self.selected)
            .flatten()
            .filter(|&i| i < flat.len());
        let t = ui.ctx().get_lb_theme();

        with_overlay_scroll(ui, Id::new("search_content_scroll"), |ui| {
            let out = egui::ScrollArea::vertical()
                .id_salt("search_content_rows")
                .auto_shrink([false, false])
                .show_viewport(ui, |ui, viewport| {
                    let content_min = ui.max_rect().min;
                    let w = ui.max_rect().width().max(1.0);
                    let content_h = geom.total.max(viewport.height());
                    ui.allocate_exact_size(vec2(w, content_h), Sense::hover());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if let Some(i) = reveal {
                        let top = pos2(content_min.x, content_min.y + geom.top(i));
                        ui.scroll_to_rect(
                            Rect::from_min_size(top, vec2(w, flat[i].height())),
                            None,
                        );
                    }

                    let view_bot = viewport.max.y;
                    for (fi, entry) in flat.iter().enumerate() {
                        let y = geom.top(fi);
                        let h = entry.height();
                        if y + h < viewport.min.y || y > view_bot {
                            continue;
                        }
                        let Some(r) = results.get(entry.match_idx()) else { continue };
                        let rect =
                            Rect::from_min_size(pos2(content_min.x, content_min.y + y), vec2(w, h));
                        let selected = highlight == Some(fi);
                        let resp = crate::style::place_at(
                            ui,
                            rect,
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(w);
                                match entry {
                                    FlatEntry::Header { .. } => {
                                        self.show_file_row(ui, r, entry.match_idx(), selected)
                                    }
                                    FlatEntry::Child { highlight_idx, .. } => {
                                        self.show_snippet_row(ui, r, *highlight_idx, selected)
                                    }
                                    FlatEntry::ShowMore { count, .. } => {
                                        let s = if *count == 1 { "" } else { "es" };
                                        let label = format!("Show {count} more match{s}");
                                        self.show_toggle_row(ui, r, &label, selected)
                                    }
                                    FlatEntry::ShowLess { .. } => {
                                        self.show_toggle_row(ui, r, "Show less", selected)
                                    }
                                }
                            },
                        )
                        .0;
                        if resp.hovered() {
                            hovered_flat = Some(fi);
                        }
                        let disclose = matches!(
                            entry,
                            FlatEntry::ShowMore { .. } | FlatEntry::ShowLess { .. }
                        );
                        if resp.clicked() && disclose {
                            let expand = matches!(entry, FlatEntry::ShowMore { .. });
                            toggled = Some((entry.match_idx(), r.id, expand));
                        } else if resp.clicked() {
                            clicked_flat = Some(fi);
                            clicked_new_tab = ui.input(|i| i.modifiers.command);
                        }
                        if !disclose {
                            if let Some(new_tab) =
                                crate::style::context_menu::show(&resp, &t, |e| {
                                    e.item(phosphor::ARROW_SQUARE_OUT, "Open", false);
                                    e.item(phosphor::APP_WINDOW, "Open in new tab", true);
                                })
                            {
                                ctx_id = Some(r.id);
                                ctx_new_tab = new_tab;
                            }
                        }
                    }
                });
            ((), out.state.offset.y, out.id)
        });

        let mut activated = None;
        let mut activated_in_new_tab = false;

        if let Some((match_idx, id, expand)) = toggled {
            set_expanded(&mut self.expanded, &mut self.selected, &flat, match_idx, id, expand);
            self.kb_mode = true;
            self.reveal_selected = true;
        } else if let Some(id) = ctx_id {
            activated = Some(id);
            activated_in_new_tab = ctx_new_tab;
        } else if let Some(i) = clicked_flat {
            self.selected = Some(i);
            self.kb_mode = false;
            activated = flat
                .get(i)
                .and_then(|e| results.get(e.match_idx()))
                .map(|r| r.id);
            activated_in_new_tab = clicked_new_tab;
        } else if !self.kb_mode {
            if let Some(i) = hovered_flat {
                self.selected = Some(i);
            }
        }

        let sel_entry = self.selected.and_then(|i| flat.get(i));
        self.selected_id = sel_entry.and_then(|e| results.get(e.match_idx()).map(|r| r.id));
        let selected_range = sel_entry.and_then(|e| {
            let r = results.get(e.match_idx())?;
            let hi = e.highlight_idx()?;
            r.content_matches.get(hi).map(|m| m.range.clone())
        });

        super::PickerResponse {
            activated,
            activated_in_new_tab,
            selected: self.selected_id,
            selected_range,
            clear_scope: false,
        }
    }
}

impl ContentSearch {
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
                    self.reveal_selected = true;
                }
                if i.consume_key_exact(Modifiers::NONE, Key::ArrowUp) {
                    self.selected = Some(self.selected.map_or(0, |i| i.saturating_sub(1)));
                    self.kb_mode = true;
                    self.reveal_selected = true;
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
                        self.activate_nth = Some(idx);
                        self.kb_mode = true;
                    }
                }
            });
        }

        if ctx.input(|i| i.pointer.delta().length_sq() > 16.0) {
            self.kb_mode = false;
        }
    }

    fn show_file_row(
        &self, ui: &mut Ui, result: &SearchResult, ordinal: usize, selected: bool,
    ) -> egui::Response {
        let t = ui.ctx().get_lb_theme();
        let sc_w = if ordinal < 9 { path::shortcut_trail_w(ui, &t) } else { 0.0 };
        let resp = FileRow::new(&t, &result.filename)
            .icon(file_row_icon(&result.filename, false))
            .subtitle(parent_crumbs(&result.parent_path))
            .highlighted(selected)
            .trail_reserve(sc_w)
            .show(ui, Id::new("content_file").with(result.id));
        if ordinal < 9 {
            path::paint_row_shortcut(ui, &t, resp.rect, ordinal + 1);
        }
        resp
    }

    fn show_snippet_row(
        &self, ui: &mut Ui, result: &SearchResult, hi: usize, selected: bool,
    ) -> egui::Response {
        let t = ui.ctx().get_lb_theme();
        let spans = self.extract_snippet(result.id, &result.content_matches[hi].range);
        FileRow::new(&t, "")
            .continuation(true)
            .caption_spans(spans)
            .highlighted(selected)
            .show(ui, Id::new("content_snip").with(result.id).with(hi))
    }

    fn show_toggle_row(
        &self, ui: &mut Ui, result: &SearchResult, label: &str, selected: bool,
    ) -> egui::Response {
        let t = ui.ctx().get_lb_theme();
        FileRow::new(&t, label)
            .icon(phosphor::ARROWS_VERTICAL)
            .continuation(true)
            .muted(true)
            .centered(true)
            .highlighted(selected)
            .show(ui, Id::new("content_show_more").with(result.id))
    }

    fn show_empty_state(&self, ui: &mut Ui, scope_name: &str, center: bool) -> bool {
        let (title, subtitle, in_folder) = if self.submitted_query.is_empty() {
            ("Search in files", "Type to search file contents", None)
        } else {
            ("No matches", "", Some(scope_name))
        };
        super::paint_search_empty(ui, title, subtitle, self.scoped, in_folder, center)
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
