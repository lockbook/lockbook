use crate::Lb;

pub struct Status {
    pub offline: bool,
    pub pending_shares: bool,
    
    pub local_status: Option<LocalStatus>,
    pub sync_status: Option<SyncStatus>,

    pub space_used: Option<UsageInfo>,
}

pub struct LocalStatus {}

pub struct SyncStatus {}

pub struct UsageInfo {}

impl Lb {
    fn status(&self) -> Status {
    }
}

// this is going to be cheap to ask for
// we will eagerly compute this and have it ready
// we will broadcast changes to these fields
// we will consume other status updates and keep these fields up to date
// some of these fields can invalidate one another
// offline for example can invalidate the other statuses, and it's nice to
// centrally manage that data dependency here
