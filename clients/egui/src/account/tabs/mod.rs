mod drawing;
mod image_viewer;
mod markdown;
mod plain_text;

pub use drawing::Drawing;
pub use image_viewer::ImageViewer;
pub use markdown::Markdown;
pub use plain_text::PlainText;

use eframe::egui;

pub struct Tab {
    pub id: lb::Uuid,
    pub name: String,
    pub failure: Option<TabFailure>,
    pub content: Option<TabContent>,
}

pub enum TabContent {
    Drawing(Box<Drawing>),
    Image(Box<ImageViewer>),
    Markdown(Box<Markdown>),
    PlainText(Box<PlainText>),
}

pub enum TabFailure {
    SimpleMisc(String),
    Unexpected(String),
}

impl TabFailure {
    pub fn show(&self, ui: &mut egui::Ui) {
        match self {
            Self::SimpleMisc(msg) => ui.label(msg),
            Self::Unexpected(msg) => ui.label(msg),
        };
    }
}

impl From<lb::Error<lb::ReadDocumentError>> for TabFailure {
    fn from(err: lb::Error<lb::ReadDocumentError>) -> Self {
        match err {
            lb::Error::UiError(err) => Self::SimpleMisc(format!("{:?}", err)),
            lb::Error::Unexpected(msg) => Self::Unexpected(msg),
        }
    }
}

impl From<lb::Error<lb::GetDrawingError>> for TabFailure {
    fn from(err: lb::Error<lb::GetDrawingError>) -> Self {
        match err {
            lb::Error::UiError(err) => Self::SimpleMisc(format!("{:?}", err)),
            lb::Error::Unexpected(msg) => Self::Unexpected(msg),
        }
    }
}
