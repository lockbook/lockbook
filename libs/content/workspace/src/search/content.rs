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
    /// Right on a collapsed file header (tree-style disclose).
    expand_sel: bool,
    /// Left: child → header, or collapse an expanded header.
    collapse_sel: bool,
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
            expand_sel: false,
            collapse_sel: false,
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
}

impl FlatEntry {
    fn match_idx(self) -> usize {
        match self {
            Self::Header { match_idx }
            | Self::Child { match_idx, .. }
            | Self::ShowMore { match_idx, .. } => match_idx,
        }
    }
    fn highlight_idx(self) -> Option<usize> {
        match self {
            Self::Child { highlight_idx, .. } => Some(highlight_idx),
            Self::Header { .. } => Some(0),
            Self::ShowMore { .. } => None,
        }
    }
    fn height(self) -> f32 {
        match self {
            Self::Header { .. } => FILE_ROW_H,
            Self::Child { .. } | Self::ShowMore { .. } => CONT_ROW_H,
        }
    }
}

fn gap_after_row(flat: &[FlatEntry], i: usize) -> bool {
    matches!(flat[i], FlatEntry::Child { .. } | FlatEntry::ShowMore { .. })
        && matches!(flat.get(i + 1), None | Some(FlatEntry::Header { .. }))
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
        }
    }
    entries
}

/// Right steps into the first match (or expands “show more”). Left on a child
/// selects the file; Left on an expanded header collapses back to [`PREVIEW`].
fn apply_tree_keys(
    expand: bool, collapse: bool, expanded: &mut HashSet<Uuid>, selected: &mut Option<usize>,
    results: &[SearchResult], flat: &mut Vec<FlatEntry>,
) {
    if !expand && !collapse {
        return;
    }
    let Some(i) = *selected else { return };
    let Some(entry) = flat.get(i).copied() else { return };
    let Some(r) = results.get(entry.match_idx()) else { return };
    let mut rebuild = false;
    if expand {
        match entry {
            FlatEntry::Header { .. } => {
                let next = i + 1;
                if matches!(
                    flat.get(next),
                    Some(FlatEntry::Child { match_idx, .. } | FlatEntry::ShowMore { match_idx, .. })
                        if *match_idx == entry.match_idx()
                ) {
                    *selected = Some(next);
                }
            }
            FlatEntry::ShowMore { .. } => {
                expanded.insert(r.id);
                rebuild = true;
            }
            _ => {}
        }
    }
    if collapse {
        match entry {
            FlatEntry::Child { match_idx, .. } | FlatEntry::ShowMore { match_idx, .. } => {
                if let Some(hi) = flat.iter().position(
                    |e| matches!(e, FlatEntry::Header { match_idx: mi } if *mi == match_idx),
                ) {
                    *selected = Some(hi);
                }
            }
            FlatEntry::Header { .. } if expanded.contains(&r.id) => {
                expanded.remove(&r.id);
                rebuild = true;
            }
            _ => {}
        }
    }
    if rebuild {
        *flat = build_flat_index(results, expanded);
    }
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

        let mut flat = build_flat_index(results, &self.expanded);
        if !self.submitted_query.is_empty() && !flat.is_empty() && self.selected.is_none() {
            self.selected = Some(0);
            self.kb_mode = true;
        }

        let expand = std::mem::take(&mut self.expand_sel);
        let collapse = std::mem::take(&mut self.collapse_sel);
        apply_tree_keys(
            expand,
            collapse,
            &mut self.expanded,
            &mut self.selected,
            results,
            &mut flat,
        );
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
            if let Some(FlatEntry::ShowMore { match_idx, .. }) =
                self.selected.and_then(|i| flat.get(i))
            {
                if let Some(r) = results.get(*match_idx) {
                    self.expanded.insert(r.id);
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
        let mut more_id: Option<Uuid> = None;

        let heights: Vec<f32> = flat
            .iter()
            .enumerate()
            .map(|(i, e)| e.height() + if gap_after_row(&flat, i) { GROUP_GAP } else { 0.0 })
            .collect();
        let geom = RowGeom::from_heights(&heights);
        let highlight = self.selected;
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
                                        self.show_more_row(ui, r, *count, selected)
                                    }
                                }
                            },
                        )
                        .0;
                        if self.kb_mode && self.selected == Some(fi) {
                            resp.scroll_to_me(None);
                        }
                        if resp.hovered() {
                            hovered_flat = Some(fi);
                        }
                        let disclose = matches!(entry, FlatEntry::ShowMore { .. });
                        if resp.clicked() && disclose {
                            more_id = Some(r.id);
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

        if let Some(id) = more_id {
            self.expanded.insert(id);
            self.kb_mode = true;
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
                if i.consume_key_exact(Modifiers::NONE, Key::ArrowRight) {
                    self.expand_sel = true;
                    self.kb_mode = true;
                }
                if i.consume_key_exact(Modifiers::NONE, Key::ArrowLeft) {
                    self.collapse_sel = true;
                    self.kb_mode = true;
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

    fn show_more_row(
        &self, ui: &mut Ui, result: &SearchResult, count: usize, selected: bool,
    ) -> egui::Response {
        let t = ui.ctx().get_lb_theme();
        let label = format!("Show {count} more match{}", if count == 1 { "" } else { "es" });
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
