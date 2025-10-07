use std::{
    collections::HashMap,
    net::IpAddr,
    ops::Deref,
    time::{SystemTime, UNIX_EPOCH},
};

use google_androidpublisher3::chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::{
    ServerState,
    billing::{
        app_store_client::AppStoreClient, google_play_client::GooglePlayClient,
        stripe_client::StripeClient,
    },
    document_service::DocumentService,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BandwidthReport {
    monthly_agg: HashMap<YearMonth, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct YearMonth {
    pub year: i32,
    pub month: u32,
}

impl YearMonth {
    fn current() -> Self {
        let now = Local::now();
        Self { year: now.year(), month: now.month() }
    }
}

pub static SERVER_BANDWIDTH_CAP: usize = 1_000_000_000_000; // 1tb = $120

impl BandwidthReport {
    pub fn current_bandwidth(&self) -> usize {
        self.monthly_agg
            .get(&YearMonth::current())
            .copied()
            .unwrap_or_default()
    }

    pub fn all_bandwidth(&self) -> usize {
        self.monthly_agg.values().sum()
    }

    pub fn increase_by(&mut self, inc: usize) {
        let now = YearMonth::current();
        match self.monthly_agg.get_mut(&YearMonth::current()) {
            Some(new) => *new += inc,
            None => {
                self.monthly_agg.insert(now, inc);
            }
        }
    }
}

/// This struct helps us ensure that a given IP isn't making too many accounts
/// this could be expanded upon as a broader rate limit, for now we're just going
/// to apply the pattern where it's needed (new-account).
#[derive(Copy, Debug, Clone, Serialize, Deserialize)]
pub struct IpData {
    ip: IpAddr,
    time: u64,
}
static MAX_IPS: u16 = 1000;

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    /// Checks whether the server is configured to rate limit, and if so it will make sure that
    /// this IP has not created an account within the last 1 minute
    pub async fn can_create_account(&self, ip: IpAddr) -> bool {
        if !self.config.features.new_account_rate_limit {
            return true;
        }

        let ips = self.recent_new_account_ips.lock().await;
        for visitor in ips.deref() {
            if visitor.ip == ip {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                if now - visitor.time > Duration::minutes(1).whole_milliseconds() as u64 {
                    return true;
                } else {
                    tracing::error!("account creation not permitted due to rate limit");
                    return false;
                }
            }
        }
        true
    }

    pub async fn did_create_account(&self, ip: IpAddr) {
        let mut ips = self.recent_new_account_ips.lock().await;
        ips.retain(|visitor| visitor.ip != ip);
        if ips.len() > MAX_IPS as usize {
            ips.pop_front();
        }

        ips.push_back(IpData {
            ip,
            time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        });
    }
}
