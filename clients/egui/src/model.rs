pub struct AccountScreenInitData {
    pub files: Vec<lb::File>,
    pub sync_status: Result<String, String>,
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
            "draw" => Self::Drawing,
            "md" => Self::Markdown,
            "txt" => Self::PlainText,
            "png" | "jpeg" | "jpg" | "gif" | "webp" | "bmp" | "ico" => Self::Image(ext.to_string()),
            "cr2" => Self::ImageUnsupported(ext.to_string()),
            "go" => Self::Code(ext.to_string()),
            _ => Self::Unknown,
        }
    }
}

pub enum SyncError {
    Major(String),
    Minor(String),
}

impl From<lb::Error<lb::SyncAllError>> for SyncError {
    fn from(err: lb::Error<lb::SyncAllError>) -> Self {
        match err {
            lb::Error::UiError(err) => Self::Minor(
                match err {
                    lb::SyncAllError::Retry => "Please retry syncing in a few moments.",
                    lb::SyncAllError::CouldNotReachServer => "Offline.",
                    lb::SyncAllError::ClientUpdateRequired => "Client upgrade required.",
                }
                .to_string(),
            ),
            lb::Error::Unexpected(msg) => Self::Major(msg),
        }
    }
}
