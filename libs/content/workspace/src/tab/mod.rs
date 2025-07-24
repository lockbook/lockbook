use crate::file_cache::FilesExt as _;
use crate::mind_map::show::MindMap;
use crate::space_inspector::show::SpaceInspector;
use crate::tab::image_viewer::ImageViewer;
use crate::tab::markdown_editor::Editor as Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::SVGEditor;
use crate::task_manager::TaskManager;
use crate::theme::icons::Icon;
use crate::workspace::Workspace;

use chrono::DateTime;
use egui::Id;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::errors::{LbErr, LbErrKind};
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::{DocumentHmac, FileType};
use lb_rs::model::svg;
use std::ops::IndexMut;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub mod image_viewer;
pub mod markdown_editor;
pub mod pdf_viewer;
pub mod svg_editor;

pub struct Tab {
    pub content: ContentState,
    pub back: Vec<Uuid>,
    pub forward: Vec<Uuid>,

    pub last_changed: Instant,
    pub last_saved: Instant,

    pub rename: Option<String>,
    pub is_closing: bool,
}

impl Tab {
    pub fn id(&self) -> Option<Uuid> {
        match &self.content {
            ContentState::Loading(id) => Some(*id),
            ContentState::Open(content) => content.id(),
            _ => None,
        }
    }

    pub fn hmac(&self) -> Option<DocumentHmac> {
        match &self.content {
            ContentState::Open(TabContent::Markdown(md)) => md.hmac,
            ContentState::Open(TabContent::Svg(svg)) => svg.open_file_hmac,
            _ => None,
        }
    }

    pub fn seq(&self) -> usize {
        match &self.content {
            ContentState::Open(TabContent::Markdown(md)) => md.buffer.current.seq,
            _ => 0,
        }
    }

    pub fn markdown(&self) -> Option<&Markdown> {
        match &self.content {
            ContentState::Open(TabContent::Markdown(md)) => Some(md),
            _ => None,
        }
    }

    pub fn markdown_mut(&mut self) -> Option<&mut Markdown> {
        match &mut self.content {
            ContentState::Open(TabContent::Markdown(md)) => Some(md),
            _ => None,
        }
    }

    pub fn svg(&self) -> Option<&SVGEditor> {
        match &self.content {
            ContentState::Open(TabContent::Svg(svg)) => Some(svg),
            _ => None,
        }
    }

    pub fn svg_mut(&mut self) -> Option<&mut SVGEditor> {
        match &mut self.content {
            ContentState::Open(TabContent::Svg(svg)) => Some(svg),
            _ => None,
        }
    }

    pub fn mind_map(&self) -> Option<&MindMap> {
        match &self.content {
            ContentState::Open(TabContent::MindMap(mm)) => Some(mm),
            _ => None,
        }
    }

    pub fn mind_map_mut(&mut self) -> Option<&mut MindMap> {
        match &mut self.content {
            ContentState::Open(TabContent::MindMap(mm)) => Some(mm),
            _ => None,
        }
    }

    pub fn space_inspector(&self) -> Option<&SpaceInspector> {
        match &self.content {
            ContentState::Open(TabContent::SpaceInspector(sv)) => Some(sv),
            _ => None,
        }
    }

    pub fn space_inspector_mut(&mut self) -> Option<&mut SpaceInspector> {
        match &mut self.content {
            ContentState::Open(TabContent::SpaceInspector(sv)) => Some(sv),
            _ => None,
        }
    }

    /// Clones the content required to save the tab. This is intended for use on the UI thread. Returns `None` if the
    /// tab does not have an editable file type open.
    pub fn clone_content(&self) -> Option<TabSaveContent> {
        match &self.content {
            ContentState::Open(content) => content.clone_content(),
            _ => None,
        }
    }

    pub fn is_dirty(&self, tasks: &TaskManager) -> bool {
        if let Some(queued_at) = self.id().and_then(|id| tasks.save_queued_at(id)) {
            self.last_changed > queued_at
        } else {
            self.last_changed > self.last_saved
        }
    }
}

pub trait TabsExt: IndexMut<usize, Output = Tab> {
    fn position_by_id(&self, id: Uuid) -> Option<usize>;
    fn get_by_id(&self, id: Uuid) -> Option<&Tab> {
        self.position_by_id(id).map(|pos| &self[pos])
    }
    fn get_mut_by_id(&mut self, id: Uuid) -> Option<&mut Tab> {
        self.position_by_id(id).map(move |pos| &mut self[pos])
    }
}

impl TabsExt for [Tab] {
    fn position_by_id(&self, id: Uuid) -> Option<usize> {
        self.iter().position(|tab| tab.id() == Some(id))
    }
}

impl TabsExt for Vec<Tab> {
    fn position_by_id(&self, id: Uuid) -> Option<usize> {
        self.iter().position(|tab| tab.id() == Some(id))
    }
}

#[allow(clippy::large_enum_variant)]
pub enum ContentState {
    Loading(Uuid),
    Open(TabContent),
    Failed(TabFailure),
}

#[allow(clippy::large_enum_variant)]
pub enum TabContent {
    Image(ImageViewer),
    Markdown(Markdown),
    Pdf(PdfViewer),
    Svg(SVGEditor),
    MindMap(MindMap),
    SpaceInspector(SpaceInspector),
}

impl std::fmt::Debug for TabContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TabContent::Image(_) => write!(f, "TabContent::Image"),
            TabContent::Markdown(_) => write!(f, "TabContent::Markdown"),
            TabContent::Pdf(_) => write!(f, "TabContent::Pdf"),
            TabContent::Svg(_) => write!(f, "TabContent::Svg"),
            TabContent::MindMap(_) => write!(f, "TabContent::Graph"),
            TabContent::SpaceInspector(_) => write!(f, "TabContent::SpaceInspector"),
        }
    }
}

impl TabContent {
    pub fn id(&self) -> Option<Uuid> {
        match self {
            TabContent::Markdown(md) => Some(md.file_id),
            TabContent::Svg(svg) => Some(svg.open_file),
            TabContent::Image(image_viewer) => Some(image_viewer.id),
            TabContent::Pdf(pdf_viewer) => Some(pdf_viewer.id),
            TabContent::MindMap(_) => None,
            TabContent::SpaceInspector(_) => None,
        }
    }

    pub fn hmac(&self) -> Option<DocumentHmac> {
        match self {
            TabContent::Markdown(md) => md.hmac,
            TabContent::Svg(svg) => svg.open_file_hmac,
            _ => None,
        }
    }

    pub fn seq(&self) -> usize {
        match self {
            TabContent::Markdown(md) => md.buffer.current.seq,
            _ => 0,
        }
    }

    /// Clones the content required to save the tab. This is intended for use on the UI thread. Returns `None` if the
    /// content type is not editable.
    pub fn clone_content(&self) -> Option<TabSaveContent> {
        match self {
            TabContent::Markdown(md) => {
                Some(TabSaveContent::String(md.buffer.current.text.clone()))
            }
            TabContent::Svg(svg) => Some(TabSaveContent::Svg(Box::new(svg.buffer.clone()))),
            _ => None,
        }
    }
}

/// The content of a tab when a save is launched. Designed to include all info needed for a save while being as fast as
/// possible to assemble from an open tab on the UI thread.
#[derive(Clone)]
pub enum TabSaveContent {
    Bytes(Vec<u8>),
    String(String),
    Svg(Box<svg::buffer::Buffer>),
}

impl TabSaveContent {
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            TabSaveContent::Bytes(bytes) => bytes,
            TabSaveContent::String(string) => string.into_bytes(),
            TabSaveContent::Svg(buffer) => buffer.serialize().into_bytes(),
        }
    }
}

#[derive(Debug)]
pub enum TabFailure {
    SimpleMisc(String),
    Unexpected(String),
}

impl From<LbErr> for TabFailure {
    fn from(err: LbErr) -> Self {
        match err.kind {
            LbErrKind::Unexpected(msg) => Self::Unexpected(msg),
            _ => Self::SimpleMisc(format!("{err:?}")),
        }
    }
}

impl TabFailure {
    pub fn msg(&self) -> String {
        match self {
            TabFailure::SimpleMisc(msg) => msg.clone(),
            TabFailure::Unexpected(msg) => format!("Unexpected error: {msg}"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Markdown(markdown_editor::Event),
    Drop { content: Vec<ClipContent>, position: egui::Pos2 },
    Paste { content: Vec<ClipContent>, position: egui::Pos2 },
    PredictedTouch { id: egui::TouchId, force: Option<f32>, pos: egui::Pos2 },
}

#[derive(Debug, Clone)]
pub enum ClipContent {
    Files(Vec<PathBuf>),
    Image(Vec<u8>), // image format guessed by egui
}

#[derive(PartialEq)]
pub enum TabStatus {
    Dirty,
    LoadQueued,
    LoadInProgress,
    SaveQueued,
    SaveInProgress,
    Clean,
}

impl TabStatus {
    pub fn icon(&self) -> Icon {
        match self {
            TabStatus::Dirty => Icon::CIRCLE,
            TabStatus::LoadQueued => Icon::SCHEDULE,
            TabStatus::LoadInProgress => Icon::SAVE,
            TabStatus::SaveQueued => Icon::SCHEDULE,
            TabStatus::SaveInProgress => Icon::SAVE,
            TabStatus::Clean => Icon::CHECK_CIRCLE,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            TabStatus::Dirty => "Unsaved changes".to_string(),
            TabStatus::LoadQueued => "Queued for loading".to_string(),
            TabStatus::LoadInProgress => "Loading".to_string(),
            TabStatus::SaveQueued => "Queued for saving".to_string(),
            TabStatus::SaveInProgress => "Saving".to_string(),
            TabStatus::Clean => "Saved".to_string(),
        }
    }
}

impl Workspace {
    pub fn tab_status(&self, i: usize) -> TabStatus {
        if let Some(tab) = self.tabs.get(i) {
            if let Some(id) = tab.id() {
                if self.tasks.load_in_progress(id) {
                    return TabStatus::LoadInProgress;
                } else if self.tasks.save_in_progress(id) {
                    return TabStatus::SaveInProgress;
                } else if self.tasks.load_queued(id) {
                    return TabStatus::LoadQueued;
                } else if self.tasks.save_queued(id) {
                    return TabStatus::SaveQueued;
                }
            }
            if tab.is_dirty(&self.tasks) {
                return TabStatus::Dirty;
            }
        }
        TabStatus::Clean // can't get any cleaner than nonexistent!
    }

    pub fn tab_title(&self, tab: &Tab) -> String {
        match (tab.id(), &self.files) {
            (Some(id), Some(files)) => {
                if let Some(file) = files.files.get_by_id(id) {
                    file.name.clone()
                } else if let Ok(file) = self.core.get_file_by_id(id) {
                    // read-through (can remove when we master cache refreshes)
                    file.name.clone()
                } else {
                    "Unknown".into()
                }
            }
            (Some(_), None) => "Loading".into(),
            (None, _) => match tab.content {
                ContentState::Open(TabContent::MindMap(_)) => "Mind Map".into(),
                ContentState::Open(TabContent::SpaceInspector(_)) => "Space Inspector".into(),
                _ => "Unknown".into(),
            },
        }
    }
}

// todo: find a better place for the code that attaches additional things to egui::Context
pub trait ExtendedOutput {
    fn set_virtual_keyboard_shown(&self, enabled: bool);
    fn pop_virtual_keyboard_shown(&self) -> Option<bool>;
    fn set_context_menu(&self, pos: egui::Pos2);
    fn pop_context_menu(&self) -> Option<egui::Pos2>;
}

impl ExtendedOutput for egui::Context {
    fn set_virtual_keyboard_shown(&self, enabled: bool) {
        self.memory_mut(|m| {
            m.data
                .insert_temp(Id::new("virtual_keyboard_shown"), enabled);
        })
    }

    fn pop_virtual_keyboard_shown(&self) -> Option<bool> {
        self.memory_mut(|m| m.data.remove_temp(Id::new("virtual_keyboard_shown")))
    }

    fn set_context_menu(&self, pos: egui::Pos2) {
        self.memory_mut(|m| {
            m.data.insert_temp(Id::new("context_menu"), pos);
        })
    }

    fn pop_context_menu(&self) -> Option<egui::Pos2> {
        self.memory_mut(|m| m.data.remove_temp(Id::new("context_menu")))
    }
}

pub trait ExtendedInput {
    fn push_event(&self, event: Event);
    fn push_markdown_event(&self, event: markdown_editor::Event);
    fn pop_events(&self) -> Vec<Event>;
}

impl ExtendedInput for egui::Context {
    fn push_event(&self, event: Event) {
        self.memory_mut(|m| {
            let mut events: Vec<Event> = m
                .data
                .get_temp(Id::new("custom_events"))
                .unwrap_or_default();
            events.push(event);
            m.data.insert_temp(Id::new("custom_events"), events);
        })
    }

    fn push_markdown_event(&self, event: markdown_editor::Event) {
        self.push_event(Event::Markdown(event))
    }

    fn pop_events(&self) -> Vec<Event> {
        self.memory_mut(|m| {
            let events: Vec<Event> = m
                .data
                .get_temp(Id::new("custom_events"))
                .unwrap_or_default();
            m.data
                .insert_temp(Id::new("custom_events"), Vec::<Event>::new());
            events
        })
    }
}

// todo: use background thread
// todo: refresh file tree view
pub fn import_image(core: &Lb, file_id: Uuid, data: &[u8]) -> File {
    let file = core
        .get_file_by_id(file_id)
        .expect("get lockbook file for image");
    let siblings = core
        .get_children(&file.parent)
        .expect("get lockbook siblings for image");

    let imports_folder = {
        let mut imports_folder = None;
        for sibling in siblings {
            if sibling.name == "imports" {
                imports_folder = Some(sibling);
                break;
            }
        }
        imports_folder.unwrap_or_else(|| {
            core.create_file("imports", &file.parent, FileType::Folder)
                .expect("create lockbook folder for image")
        })
    };

    // get local time in a human readable datetime format
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let human_readable_time = DateTime::from_timestamp(time.as_secs() as _, 0)
        .expect("invalid system time")
        .format("%Y-%m-%d_%H-%M-%S")
        .to_string();
    let file_extension = image::guess_format(data)
        .unwrap_or(image::ImageFormat::Png /* shrug */)
        .extensions_str()
        .first()
        .unwrap_or(&"png");

    let file = core
        .create_file(
            &format!("pasted_image_{human_readable_time}.{file_extension}"),
            &imports_folder.id,
            FileType::Document,
        )
        .expect("create lockbook file for image");
    core.write_document(file.id, data)
        .expect("write lockbook file for image");

    file
}

pub fn core_get_relative_path(core: &Lb, from: Uuid, to: Uuid) -> String {
    let from_path = core
        .get_path_by_id(from)
        .expect("get source file path for relative link");
    let to_path = core
        .get_path_by_id(to)
        .expect("get target file path for relative link");
    get_relative_path(&from_path, &to_path)
}

pub fn get_relative_path(from: &str, to: &str) -> String {
    if from == to {
        if from.ends_with('/') {
            return "./".to_string();
        } else {
            return ".".to_string();
        }
    }

    let from_path = PathBuf::from(from);
    let to_path = PathBuf::from(to);

    let mut num_common_ancestors = 0;
    for (from_component, to_component) in from_path.components().zip(to_path.components()) {
        if from_component != to_component {
            break;
        }
        num_common_ancestors += 1;
    }

    let mut result = "../".repeat(from_path.components().count() - num_common_ancestors);
    for to_component in to_path.components().skip(num_common_ancestors) {
        result.push_str(to_component.as_os_str().to_str().unwrap());
        result.push('/');
    }
    if !to.ends_with('/') {
        result.pop();
    }
    result
}

pub fn canonicalize_path(path: &str) -> String {
    let path = PathBuf::from(path);
    let mut result = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(component) => {
                result.push(component);
            }
            Component::ParentDir => {
                result.pop();
            }
            _ => {}
        }
    }

    result.to_string_lossy().to_string()
}

pub fn core_get_by_relative_path<P: AsRef<Path>>(
    core: &Lb, from: Uuid, path: P,
) -> Result<File, String> {
    let path = path.as_ref();
    let target_path = if path.is_relative() {
        let mut open_file_path =
            PathBuf::from(core.get_path_by_id(from).map_err(|e| e.to_string())?);
        for component in path.components() {
            open_file_path.push(component);
        }
        let target_file_path = open_file_path.to_string_lossy();

        canonicalize_path(&target_file_path)
    } else {
        path.to_string_lossy().to_string()
    };
    core.get_by_path(&target_path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod test {
    #[test]
    fn get_relative_path() {
        use super::get_relative_path;

        // to documents
        assert_eq!(get_relative_path("/a/b/c", "/a/b/c"), ".");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/c/d"), "d");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/c/d/e"), "d/e");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/c/d/e/f"), "d/e/f");

        assert_eq!(get_relative_path("/a/b/c", "/a/b/d"), "../d");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/d/e"), "../d/e");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/d/e/f"), "../d/e/f");

        assert_eq!(get_relative_path("/a/b/c", "/a/d"), "../../d");
        assert_eq!(get_relative_path("/a/b/c", "/a/d/e"), "../../d/e");
        assert_eq!(get_relative_path("/a/b/c", "/a/d/e/f"), "../../d/e/f");

        assert_eq!(get_relative_path("/a/b/c", "/d"), "../../../d");
        assert_eq!(get_relative_path("/a/b/c", "/d/e"), "../../../d/e");
        assert_eq!(get_relative_path("/a/b/c", "/d/e/f"), "../../../d/e/f");

        // to folders
        assert_eq!(get_relative_path("/a/b/c", "/a/b/c/d/"), "d/");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/c/d/e/"), "d/e/");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/c/d/e/f/"), "d/e/f/");

        assert_eq!(get_relative_path("/a/b/c", "/a/b/"), "../");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/d/"), "../d/");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/d/e/"), "../d/e/");
        assert_eq!(get_relative_path("/a/b/c", "/a/b/d/e/f/"), "../d/e/f/");

        assert_eq!(get_relative_path("/a/b/c", "/a/"), "../../");
        assert_eq!(get_relative_path("/a/b/c", "/a/d/"), "../../d/");
        assert_eq!(get_relative_path("/a/b/c", "/a/d/e/"), "../../d/e/");
        assert_eq!(get_relative_path("/a/b/c", "/a/d/e/f/"), "../../d/e/f/");

        assert_eq!(get_relative_path("/a/b/c", "/"), "../../../");
        assert_eq!(get_relative_path("/a/b/c", "/d/"), "../../../d/");
        assert_eq!(get_relative_path("/a/b/c", "/d/e/"), "../../../d/e/");
        assert_eq!(get_relative_path("/a/b/c", "/d/e/f/"), "../../../d/e/f/");
    }
}
