mod drawing;
mod image_viewer;
mod markdown;
mod plain_text;

pub use drawing::Drawing;
pub use image_viewer::ImageViewer;
pub use markdown::Markdown;
pub use plain_text::PlainText;

pub struct Tab {
    pub id: lb::Uuid,
    pub name: String,
    pub path: String,
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
    DeletedFromSync,
    SimpleMisc(String),
    Unexpected(String),
}

impl From<lb::Error<lb::ReadDocumentError>> for TabFailure {
    fn from(err: lb::Error<lb::ReadDocumentError>) -> Self {
        match err {
            lb::Error::UiError(err) => Self::SimpleMisc(format!("{:?}", err)),
            lb::Error::Unexpected(msg) => Self::Unexpected(msg),
        }
    }
}

impl From<lb::Error<lb::WriteToDocumentError>> for TabFailure {
    fn from(err: lb::Error<lb::WriteToDocumentError>) -> Self {
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

impl From<lb::Error<lb::SaveDrawingError>> for TabFailure {
    fn from(err: lb::Error<lb::SaveDrawingError>) -> Self {
        match err {
            lb::Error::UiError(err) => Self::SimpleMisc(format!("{:?}", err)),
            lb::Error::Unexpected(msg) => Self::Unexpected(msg),
        }
    }
}
