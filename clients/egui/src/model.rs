use workspace_rs::theme::icons::Icon;

pub struct AccountScreenInitData {
    pub sync_status: Result<String, String>,
    pub files: Vec<lb::File>,
    pub usage: Result<Usage, String>,
}

pub struct Usage {
    pub used: String,
    pub available: String,
    pub percent: f32,
}

impl From<lb::UsageMetrics> for Usage {
    fn from(metrics: lb::UsageMetrics) -> Self {
        let used = metrics.server_usage.exact;
        let available = metrics.data_cap.exact;

        Self {
            used: lb::bytes_to_human(used),
            available: lb::bytes_to_human(available),
            percent: used as f32 / available as f32,
        }
    }
}

pub enum DocType {
    PlainText,
    Markdown,
    Drawing,
    Image(String),
    ImageUnsupported(String),
    Code(String),
    Unknown,
}

impl DocType {
    pub fn from_name(name: &str) -> Self {
        let ext = name.split('.').last().unwrap_or_default();
        match ext {
            "draw" | "svg" => Self::Drawing,
            "md" => Self::Markdown,
            "txt" => Self::PlainText,
            "png" | "jpeg" | "jpg" | "gif" | "webp" | "bmp" | "ico" => Self::Image(ext.to_string()),
            "cr2" => Self::ImageUnsupported(ext.to_string()),
            "go" => Self::Code(ext.to_string()),
            _ => Self::Unknown,
        }
    }
    pub fn to_icon(&self) -> Icon {
        match self {
            DocType::Markdown | DocType::PlainText => Icon::DOC_TEXT,
            DocType::Drawing => Icon::DRAW,
            DocType::Image(_) => Icon::IMAGE,
            DocType::Code(_) => Icon::CODE,
            _ => Icon::DOC_UNKNOWN,
        }
    }
}
