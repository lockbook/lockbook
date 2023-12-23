use crate::tab::image_viewer::ImageViewer;
use crate::tab::markdown::Markdown;
use crate::tab::pdf_viewer::PdfViewer;
use crate::tab::plain_text::PlainText;
use crate::tab::svg_editor::SVGEditor;
use std::time::Instant;

pub mod image_viewer;
pub mod markdown;
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
