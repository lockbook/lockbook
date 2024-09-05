use crate::tab::image_viewer::ImageViewer;
use crate::tab::markdown_editor::Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::svg_editor::SVGEditor;
use chrono::DateTime;
use egui::Id;
use lb_rs::{DecryptedDocument, DocumentHmac, File, FileType, Uuid};
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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

    pub is_new_file: bool,
    pub last_changed: Instant,
    pub last_saved: Instant,
}

pub struct SaveRequest {
    pub id: lb_rs::Uuid,
    pub hmac: Option<DocumentHmac>,
    pub content: String,
}

impl Tab {
    pub fn make_save_request(&self) -> Option<SaveRequest> {
        let mut hmac = None;
        if let Some(tab_content) = &self.content {
            let maybe_save_content = match tab_content {
                TabContent::Markdown(md) => {
                    hmac = md.editor.hmac;
                    Some(md.editor.buffer.current.text.clone())
                }
                TabContent::Svg(svg) => Some(svg.get_minimal_content()),
                _ => None,
            };
            maybe_save_content.map(|content| SaveRequest { id: self.id, content, hmac })
        } else {
            None
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.last_changed > self.last_saved
    }
}

pub enum TabContent {
    Image(ImageViewer),
    MergeMarkdown { hmac: Option<DocumentHmac>, content: DecryptedDocument },
    Markdown(Markdown),
    Pdf(PdfViewer),
    Svg(SVGEditor),
}

impl std::fmt::Debug for TabContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TabContent::Image(_) => write!(f, "TabContent::Image"),
            TabContent::MergeMarkdown { hmac, content } => write!(
                f,
                "TabContent::MergeMarkdown {{ hmac: {:?}, content: {:?} }}",
                hmac, content
            ),
            TabContent::Markdown(_) => write!(f, "TabContent::Markdown"),
            TabContent::Pdf(_) => write!(f, "TabContent::Pdf"),
            TabContent::Svg(_) => write!(f, "TabContent::Svg"),
        }
    }
}

#[derive(Debug)]
pub enum TabFailure {
    DeletedFromSync,
    SimpleMisc(String),
    Unexpected(String),
}

impl From<lb_rs::LbError> for TabFailure {
    fn from(err: lb_rs::LbError) -> Self {
        match err.kind {
            lb_rs::CoreError::Unexpected(msg) => Self::Unexpected(msg),
            _ => Self::SimpleMisc(format!("{:?}", err)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Markdown(markdown_editor::Event),
    Drop { content: Vec<ClipContent>, position: egui::Pos2 },
    Paste { content: Vec<ClipContent>, position: egui::Pos2 },
}

#[derive(Debug, Clone)]
pub enum ClipContent {
    Files(Vec<PathBuf>),
    Image(Vec<u8>), // image format guessed by egui
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
pub fn import_image(core: &lb_rs::Core, file_id: Uuid, data: &[u8]) -> File {
    let file = core
        .get_file_by_id(file_id)
        .expect("get lockbook file for image");
    let siblings = core
        .get_children(file.parent)
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
            core.create_file("imports", file.parent, FileType::Folder)
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
            imports_folder.id,
            FileType::Document,
        )
        .expect("create lockbook file for image");
    core.write_document(file.id, data)
        .expect("write lockbook file for image");

    file
}

pub fn core_get_relative_path(core: &lb_rs::Core, from: Uuid, to: Uuid) -> String {
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

pub fn core_get_by_relative_path(
    core: &lb_rs::Core, from: Uuid, path: &Path,
) -> Result<File, String> {
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
