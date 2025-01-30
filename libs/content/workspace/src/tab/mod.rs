use crate::mind_map::show::MindMap;
use crate::tab::image_viewer::ImageViewer;
use crate::tab::markdown_editor::Editor as Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::SVGEditor;
use crate::task_manager::TaskManager;
use crate::theme::icons::Icon;
use crate::workspace::Workspace;
use chrono::DateTime;
use egui::Id;
use lb_rs::blocking::Lb;
use lb_rs::model::errors::{LbErr, LbErrKind};
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::{DocumentHmac, FileType};
use lb_rs::{svg, Uuid};
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::instrument;

pub mod image_viewer;
pub mod markdown_editor;
pub mod pdf_viewer;
pub mod svg_editor;

pub struct Tab {
    pub id: lb_rs::Uuid,
    pub name: String,
    pub rename: Option<String>,
    pub path: String,
    pub failure: Option<TabFailure>,
    pub content: Option<TabContent>,
    pub is_closing: bool,

    pub is_new_file: bool,
    pub last_changed: Instant,
    pub last_saved: Instant,
}

impl Tab {
    pub fn is_dirty(&self, tasks: &TaskManager) -> bool {
        if let Some(queued_at) = tasks.save_queued_at(self.id) {
            self.last_changed > queued_at
        } else {
            self.last_changed > self.last_saved
        }
    }
}

pub enum TabContent {
    Image(ImageViewer),
    Markdown(Markdown),
    Pdf(PdfViewer),
    Svg(SVGEditor),
    Graph(MindMap),
}

impl std::fmt::Debug for TabContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TabContent::Image(_) => write!(f, "TabContent::Image"),
            TabContent::Markdown(_) => write!(f, "TabContent::Markdown"),
            TabContent::Pdf(_) => write!(f, "TabContent::Pdf"),
            TabContent::Svg(_) => write!(f, "TabContent::Svg"),
            TabContent::Graph(_) => write!(f, "TabContent::Graph"),
        }
    }
}

impl TabContent {
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

    /// Clones the content required to save the tab. This is intended for use on the UI thread.
    #[instrument(level = "error", skip_all)]
    pub fn clone_content(&self) -> Option<TabSaveContent> {
        match self {
            TabContent::Markdown(md) => {
                Some(TabSaveContent::String(md.buffer.current.text.clone()))
            }
            TabContent::Svg(svg) => Some(TabSaveContent::Svg(svg.buffer.clone())),
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
    Svg(svg::buffer::Buffer),
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
    DeletedFromSync,
    SimpleMisc(String),
    Unexpected(String),
}

impl From<LbErr> for TabFailure {
    fn from(err: LbErr) -> Self {
        match err.kind {
            LbErrKind::Unexpected(msg) => Self::Unexpected(msg),
            _ => Self::SimpleMisc(format!("{:?}", err)),
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
    pub fn tab_status(&self, id: Uuid) -> TabStatus {
        if let Some(tab) = self.tabs.iter().find(|t| t.id == id) {
            if self.tasks.load_in_progress(tab.id) {
                TabStatus::LoadInProgress
            } else if self.tasks.save_in_progress(tab.id) {
                TabStatus::SaveInProgress
            } else if self.tasks.load_queued(tab.id) {
                TabStatus::LoadQueued
            } else if self.tasks.save_queued(tab.id) {
                TabStatus::SaveQueued
            } else if tab.is_dirty(&self.tasks) {
                TabStatus::Dirty
            } else {
                TabStatus::Clean
            }
        } else {
            TabStatus::Clean
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
            &format!("pasted_image_{}.{}", human_readable_time, file_extension),
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

pub fn core_get_by_relative_path(core: &Lb, from: Uuid, path: &Path) -> Result<File, String> {
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
