use crate::Lb;

pub enum Status {
    Syncing,
    Offline,
}

impl Lb {
    fn status(&self) -> Vec<Status> {
    }
}
