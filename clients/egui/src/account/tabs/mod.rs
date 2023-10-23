mod drawing;
mod image_viewer;
mod markdown;
mod pdf_viewer;
mod plain_text;
mod svg_editor;

pub use drawing::Drawing;
pub use image_viewer::ImageViewer;
pub use markdown::Markdown;
pub use pdf_viewer::PdfViewer;
pub use plain_text::PlainText;
pub use svg_editor::SVGEditor;

use std::time::Instant;

pub struct Tab {
    pub id: lb::Uuid,
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
    pub id: lb::Uuid,
    pub content: SaveRequestContent,
}

pub enum SaveRequestContent {
    Text(String),
    Draw(lb::Drawing),
}

impl Tab {
    pub fn make_save_request(&self) -> Option<SaveRequest> {
        if let Some(tab_content) = &self.content {
            let maybe_save_content = match tab_content {
                TabContent::Drawing(d) => Some(SaveRequestContent::Draw(d.drawing.clone())),
                TabContent::Markdown(md) => {
                    Some(SaveRequestContent::Text(md.editor.buffer.current.text.clone()))
                }
                TabContent::PlainText(txt) => Some(SaveRequestContent::Text(txt.content.clone())),
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
    Drawing(Box<Drawing>),
    Image(Box<ImageViewer>),
    Markdown(Box<Markdown>),
    PlainText(Box<PlainText>),
    Pdf(Box<PdfViewer>),
    Svg(Box<SVGEditor>),
}

pub enum TabFailure {
    DeletedFromSync,
    SimpleMisc(String),
    Unexpected(String),
}

impl From<lb::LbError> for TabFailure {
    fn from(err: lb::LbError) -> Self {
        match err.kind {
            lb::CoreError::Unexpected(msg) => Self::Unexpected(msg),
            _ => Self::SimpleMisc(format!("{:?}", err)),
        }
    }
}
