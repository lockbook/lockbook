use crate::Lb;
use crate::io::server::Server;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

impl<S: Server> Lb<S> {
    /// hint to background processing pipelines whether or not a user is around
    pub fn app_foregrounded(&self) {
        let bg_lb = self.clone();
        tokio::spawn(async move {
            *bg_lb.user_last_seen.write().await = web_time::Instant::now();
            bg_lb.user_wake.notify_one();
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Ord, PartialEq, PartialOrd, Eq, Hash, Copy)]
pub enum DocEvent {
    Read(Uuid, i64),
    Write(Uuid, i64),
}
