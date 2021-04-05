use lockbook_models::crypto::Timestamped;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock {
    fn get_time() -> i64;
    fn timestamp<T>(t: T) -> Timestamped<T> {
        Timestamped {
            value: t,
            timestamp: Self::get_time(),
        }
    }
}

pub struct ClockImpl;

impl Clock for ClockImpl {
    fn get_time() -> i64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(t) => t.as_millis() as i64,
            Err(e) => -(e.duration().as_millis() as i64),
        }
    }
}
