use std::time::{SystemTime, UNIX_EPOCH};
pub trait Clock {
    fn get_time() -> u64;
}

pub struct ClockImpl;

impl Clock for ClockImpl {
    fn get_time() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}
