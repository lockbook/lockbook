use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

pub trait Clock {
    fn get_time() -> Result<u128, SystemTimeError>;
}

pub struct ClockImpl;

impl Clock for ClockImpl {
    fn get_time() -> Result<u128, SystemTimeError> {
        Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
    }
}
