use lb::{
    model::usage::bytes_to_human,
    model::{errors::LbErr, file::File},
    service::usage::UsageMetrics,
};

pub struct AccountScreenInitData {
    pub sync_status: Result<String, LbErr>,
    pub files: Vec<File>,
    pub usage: Result<Usage, String>,
}

pub struct AccountPhraseData {
    pub phrase: String,
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
