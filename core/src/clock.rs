use std::time::{SystemTimeError, UNIX_EPOCH, SystemTime};
pub trait Clock {
    fn get_time() -> u128;
}

pub struct ClockImpl;

impl ClockImpl {
    pub fn get_time() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }
}