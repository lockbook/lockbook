use std::sync::Mutex;

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

/// A wall clock that never reports a time earlier than one it already reported.
pub struct Clock {
    source: TimeGetter,
    /// The server stamps every version under its index_db lock, so this lock is uncontended.
    last: Mutex<i64>,
}

impl Clock {
    pub fn new(source: TimeGetter) -> Self {
        Self { source, last: Mutex::new(i64::MIN) }
    }

    pub fn now(&self) -> Timestamp {
        let mut last = self.last.lock().unwrap();
        // read under the lock so a lower reading means the source went back, not a race
        let reading = (self.source)().0;
        if reading < *last {
            tracing::warn!(back_ms = *last - reading, "clock went backwards");
        } else {
            *last = reading;
        }
        Timestamp(*last)
    }
}

impl Clone for Clock {
    /// A clock with this one's source that never reports earlier than this one has.
    fn clone(&self) -> Self {
        Self { source: self.source, last: Mutex::new(*self.last.lock().unwrap()) }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new(get_time)
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, Timestamp};
    use std::sync::atomic::{AtomicUsize, Ordering};

    const READINGS: [i64; 8] = [100, 105, 103, 103, 90, 106, 106, 50];
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn scripted() -> Timestamp {
        Timestamp(READINGS[NEXT.fetch_add(1, Ordering::SeqCst)])
    }

    #[test]
    fn never_reports_earlier_time() {
        let clock = Clock::new(scripted);
        let reported: Vec<i64> = READINGS.iter().map(|_| clock.now().0).collect();
        assert_eq!(reported, [100, 105, 105, 105, 105, 106, 106, 106]);
    }
}
