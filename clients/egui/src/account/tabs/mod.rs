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

impl From<lb::LbError> for TabFailure {
    fn from(err: lb::LbError) -> Self {
        match err.kind {
            lb::LbErrorKind::Unexpected(msg) => Self::Unexpected(msg),
            _ => Self::SimpleMisc(format!("{:?}", err)),
        }
    }
}
