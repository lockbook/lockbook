use std::iter;
use std::sync::{Arc, RwLock};

use egui::{Color32, Context, FontFamily, FontId, Galley, Id, Key, Modifiers, Ui, pos2};
use lb_rs::blocking::Lb;
use lb_rs::search::{SearchFilter, SearchResult};

use crate::{
    file_cache::{FileCache, FilesExt, canonicalize, link_id},
    search::{SearchExecutor, SearchType},
    show::InputStateExt,
    style::{
        FileRow, Space, ThemeExt, TypeRole, file_row_icon, path_crumbs, phosphor,
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
    files: Arc<RwLock<FileCache>>,
    /// The file the query names when it's a link.
    link: Option<Link>,
}

/// A query that is a link, with the file it names.
#[derive(Debug)]
enum Link {
    /// An id link or a bare id: that file alone, wherever search is scoped.
    /// None when no file here has the id.
    Id(Option<Row>),
    /// An absolute path to a file: it leads the name matches.
    Path(Row),
}

/// What a query names when it's a link to a file.
#[derive(Debug, PartialEq)]
enum Linked {
    /// `lb://<id>`, `https://<host>/open/<id>`, or a bare id; None when no
    /// file here has it.
    Id(Option<lb_rs::Uuid>),
    /// An absolute path, URL-encoded or not, that reaches a file.
    Path(lb_rs::Uuid),
}

/// The file `query` links to, read from the file cache. Surrounding
/// whitespace, a trailing `/`, and a `#fragment` are ignored. Relative paths
/// and wikilinks have no note to resolve from, so they aren't links here.
fn linked<F: FilesExt + ?Sized>(files: &F, query: &str) -> Option<Linked> {
    let query = query.trim();
    let bare = || {
        let id = query.split(['#', '?']).next()?.trim_end_matches('/');
        lb_rs::Uuid::try_parse(id).ok()
    };
    if let Some(id) = link_id(query).or_else(bare) {
        return Some(Linked::Id(files.get_by_id(id).map(|f| f.id)));
    }
    if !query.starts_with('/') {
        return None;
    }
    let path = canonicalize(query.split('#').next()?);
    if path.is_empty() {
        return None;
    }
    let decoded = urlencoding::decode(&path).map_or(path.clone(), |p| p.into_owned());
    files.by_path(&decoded).map(|f| Linked::Path(f.id))
}

/// `id`'s result row.
fn file_row<F: FilesExt + ?Sized>(files: &F, id: lb_rs::Uuid) -> Option<Row> {
    let file = files.get_by_id(id)?;
    Some(Row {
        id,
        filename: file.name.clone(),
        parent_path: files.path(file.parent),
        is_folder: file.is_folder(),
    })
}

/// The rows a query shows: a link's file, then for a path the name matches
/// without it.
fn compose(link: Option<&Link>, matches: impl Iterator<Item = Row>) -> Vec<Row> {
    match link {
        Some(Link::Id(row)) => row.iter().cloned().collect(),
        Some(Link::Path(row)) => iter::once(row.clone())
            .chain(matches.filter(|r| r.id != row.id))
            .collect(),
        None => matches.collect(),
    }
}

#[derive(Clone, Debug)]
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
        self.link = {
            let files = self.files.read().unwrap();
            linked(&*files, query).and_then(|linked| match linked {
                Linked::Id(id) => Some(Link::Id(id.and_then(|id| file_row(&*files, id)))),
                Linked::Path(id) => file_row(&*files, id).map(Link::Path),
            })
        };
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
        match &self.link {
            Some(Link::Id(row)) => row.is_some(),
            Some(Link::Path(_)) => true,
            None => self.visible().next().is_some(),
        }
    }

    fn show_result_picker(
        &mut self, ui: &mut egui::Ui, allow_kb_nav: bool, scope_name: &str, empty_centered: bool,
        pin_state: &dyn Fn(lb_rs::Uuid) -> Option<bool>,
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
                file_cmd: None,
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
                file_cmd: None,
            };
        }

        let mut hovered: Option<usize> = None;
        let mut clicked: Option<usize> = None;
        let mut clicked_id: Option<lb_rs::Uuid> = None;
        let mut clicked_new_tab = false;
        let mut file_cmd = None;

        const ROW_HEIGHT: f32 = FileRow::height_for(true);

        let highlight = self.selected;
        let t = ui.ctx().get_lb_theme();
        with_overlay_scroll(ui, Id::unique("search_path_scroll"), |ui| {
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
                        match super::row_menu(&resp, &t, row.is_folder, || pin_state(row.id)) {
                            Some(super::RowMenu::Open { new_tab }) => {
                                clicked_id = Some(row.id);
                                clicked_new_tab = new_tab;
                            }
                            Some(super::RowMenu::File(cmd)) => file_cmd = Some((row.id, cmd)),
                            None => {}
                        }
                    }
                });
            ((), out.state.clamped_offset().y, out.id)
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
            file_cmd,
        }
    }
}

impl PathSearch {
    pub fn new(lb: &Lb, files: Arc<RwLock<FileCache>>) -> Self {
        Self {
            searcher: lb.path_searcher(),
            submitted_query: String::new(),
            selected: None,
            activate: false,
            activate_new_tab: false,
            kb_mode: false,
            selected_id: None,
            scoped: false,
            files,
            link: None,
        }
    }

    /// Empty query lists recent documents; folders only surface as matches.
    fn visible(&self) -> impl Iterator<Item = &SearchResult> {
        let recents = self.submitted_query.is_empty();
        self.searcher
            .results()
            .iter()
            .filter(move |r| !(recents && r.is_folder))
    }

    fn rows(&self) -> Vec<Row> {
        let matches = self.visible().map(|r| Row {
            id: r.id,
            filename: r.filename.clone(),
            parent_path: r.parent_path.clone(),
            is_folder: r.is_folder,
        });
        compose(self.link.as_ref(), matches)
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
        if matches!(self.link, Some(Link::Id(None))) {
            let subtitle = "It may be deleted or not shared with you.";
            return super::paint_search_empty(
                ui,
                "No file with this id",
                subtitle,
                false,
                None,
                center,
            );
        }
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
            .subtitle(path_crumbs(&row.parent_path))
            .selected(selected)
            .trail_reserve(trail)
            .show(ui, Id::unique("search_row").with(row.id));
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

#[cfg(test)]
mod tests {
    use super::{Link, Linked, Row, compose, file_row, linked};
    use crate::test_utils::files::{at, cache, tree};

    #[test]
    fn id_links_name_their_file() {
        let files = cache(tree("alice", &["/notes/plan.md", "/notes/archive/"]));
        for path in ["/notes/plan.md", "/notes/archive/"] {
            let id = at(&files, path);
            for query in [
                format!("lb://{id}"),
                format!("https://app.lockbook.net/open/{id}"),
                format!("{id}"),
                format!("  LB://{}/#intro \n", id.to_string().to_uppercase()),
                format!("{id}#intro"),
            ] {
                assert_eq!(linked(&files, &query), Some(Linked::Id(Some(id))), "{query}");
            }
        }
    }

    #[test]
    fn an_unknown_id_names_nothing() {
        let files = cache(tree("alice", &["/plan.md"]));
        let query = "https://app.lockbook.net/open/00000000-0000-0000-0000-00000000002a";
        assert_eq!(linked(&files, query), Some(Linked::Id(None)));
    }

    #[test]
    fn absolute_paths_name_their_file() {
        let files = cache(tree("alice", &["/my notes/Q3 plan.md", "/plan.md"]));
        let id = at(&files, "/my notes/Q3 plan.md");
        for query in
            ["/my notes/Q3 plan.md", "/my%20notes/Q3%20plan.md", " /my notes/./Q3 plan.md#goals"]
        {
            assert_eq!(linked(&files, query), Some(Linked::Path(id)), "{query}");
        }
        for query in ["/my notes/Q3", "plan.md", "[[plan]]", "/", "plan"] {
            assert_eq!(linked(&files, query), None, "{query}");
        }
    }

    /// Name matches stand for what the searcher returns, scope applied.
    #[test]
    fn a_path_leads_its_matches_and_an_id_stands_alone() {
        let files = cache(tree("alice", &["/plan.md", "/notes/plan.md"]));
        let row = |path| file_row(&files, at(&files, path)).unwrap();
        let matches = || [row("/plan.md"), row("/notes/plan.md")].into_iter();
        let ids = |rows: Vec<Row>| rows.into_iter().map(|r| r.id).collect::<Vec<_>>();

        let path = Link::Path(row("/notes/plan.md"));
        let expected = vec![at(&files, "/notes/plan.md"), at(&files, "/plan.md")];
        assert_eq!(ids(compose(Some(&path), matches())), expected);

        let id = Link::Id(Some(row("/notes/plan.md")));
        assert_eq!(ids(compose(Some(&id), std::iter::empty())), vec![at(&files, "/notes/plan.md")]);
        assert!(compose(Some(&Link::Id(None)), matches()).is_empty());
    }
}
