use crate::tab::image_viewer::ImageViewer;
use crate::tab::markdown_editor::Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::plain_text::PlainText;
use crate::tab::svg_editor::SVGEditor;
use chrono::DateTime;
use egui::Id;
use lb_rs::{File, FileType, Uuid};
use markdown_editor::input::canonical::Modification;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub mod image_viewer;
pub mod markdown_editor;
pub mod pdf_viewer;
pub mod plain_text;
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
    pub content: String,
}

impl Tab {
    pub fn make_save_request(&self) -> Option<SaveRequest> {
        if let Some(tab_content) = &self.content {
            let maybe_save_content = match tab_content {
                TabContent::Markdown(md) => Some(md.editor.buffer.current.text.clone()),
                TabContent::PlainText(txt) => Some(txt.content.clone()),
                TabContent::Svg(svg) => Some(svg.get_minimal_content()),
                _ => None,
            };
            maybe_save_content.map(|content| SaveRequest { id: self.id, content })
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
    Markdown(Markdown),
    PlainText(PlainText),
    Pdf(PdfViewer),
    Svg(SVGEditor),
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
    Markdown(Modification),
    Drop { content: Vec<ClipContent>, position: egui::Pos2 },
    Paste { content: Vec<ClipContent>, position: egui::Pos2 },
}

#[derive(Debug, Clone)]
pub enum ClipContent {
    Files(Vec<PathBuf>),
    Png(Vec<u8>),
}

pub trait EventManager {
    fn push_event(&self, event: Event);
    fn push_markdown_event(&self, event: Modification);
    fn pop_events(&self) -> Vec<Event>;
}

impl EventManager for egui::Context {
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

    fn push_markdown_event(&self, event: Modification) {
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

// todo: use relative path (caller responsibilty?)
// todo: use background thread
// todo: refresh file tree view
pub fn import_image(core: &lb_rs::Core, open_file: Uuid, data: &[u8]) -> File {
    println!("importing image");

    let file = core
        .get_file_by_id(open_file)
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

    let file = core
        .create_file(
            &format!("pasted_image_{}.png", human_readable_time),
            imports_folder.id,
            FileType::Document,
        )
        .expect("create lockbook file for image");
    core.write_document(file.id, data)
        .expect("write lockbook file for image");

    file
}
