use web_time::{SystemTime, UNIX_EPOCH};

use crate::model::crypto::Timestamped;

pub type TimeGetter = fn() -> Timestamp;

#[derive(Clone, Debug)]
pub struct Timestamp(pub i64);

pub fn get_time() -> Timestamp {
    let time = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(t) => t.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64),
    };

    Timestamp(time)
}

pub fn timestamp<T>(t: T, time_getter: TimeGetter) -> Timestamped<T> {
    Timestamped { value: t, timestamp: time_getter().0 }
}
