use crate::tab::image_viewer::ImageViewer;
use crate::tab::markdown_editor::Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::plain_text::PlainText;
use crate::tab::svg_editor::SVGEditor;
use egui::Id;
use markdown_editor::input::canonical::Modification;
use std::collections::HashMap;
use std::time::Instant;

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
    Drop { content: ClipContent, position: egui::Pos2 },
    Paste { content: ClipContent, position: egui::Pos2 },
}

pub type ClipContent = HashMap<String, Vec<u8>>;

pub trait CustomEventer {
    fn push_custom_event(&self, event: Event);
    fn push_markdown_event(&self, event: Modification);
    fn pop_custom_events(&self) -> Vec<Event>;
}

impl CustomEventer for egui::Context {
    fn push_custom_event(&self, event: Event) {
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
        self.memory_mut(|m| {
            let mut events: Vec<Event> = m
                .data
                .get_temp(Id::new("custom_events"))
                .unwrap_or_default();
            events.push(Event::Markdown(event));
            m.data.insert_temp(Id::new("custom_events"), events);
        })
    }

    fn pop_custom_events(&self) -> Vec<Event> {
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
