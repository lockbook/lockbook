use lb::{logic::usage::bytes_to_human, model::file::File, service::usage::UsageMetrics};
use workspace_rs::theme::icons::Icon;

pub struct AccountScreenInitData {
    pub sync_status: Result<String, String>,
    pub files: Vec<File>,
    pub usage: Result<Usage, String>,
}

pub struct Usage {
    pub used: String,
    pub available: String,
    pub percent: f32,
}

impl From<UsageMetrics> for Usage {
    fn from(metrics: UsageMetrics) -> Self {
        let used = metrics.server_usage.exact;
        let available = metrics.data_cap.exact;

        Self {
            used: bytes_to_human(used),
            available: bytes_to_human(available),
            percent: used as f32 / available as f32,
        }
    }
}

pub enum DocType {
    PlainText,
    Markdown,
    Drawing,
    Image,
    ImageUnsupported,
    Code,
    Unknown,
}

impl DocType {
    pub fn from_name(name: &str) -> Self {
        let ext = name.split('.').last().unwrap_or_default();
        match ext {
            "draw" | "svg" => Self::Drawing,
            "md" => Self::Markdown,
            "txt" => Self::PlainText,
            "cr2" => Self::ImageUnsupported,
            "go" => Self::Code,
            _ if workspace_rs::tab::image_viewer::is_supported_image_fmt(ext) => Self::Image,
            _ => Self::Unknown,
        }
    }
    pub fn to_icon(&self) -> Icon {
        match self {
            DocType::Markdown | DocType::PlainText => Icon::DOC_TEXT,
            DocType::Drawing => Icon::DRAW,
            DocType::Image => Icon::IMAGE,
            DocType::Code => Icon::CODE,
            _ => Icon::DOC_UNKNOWN,
        }
    }
}
