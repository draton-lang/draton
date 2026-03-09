use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{DurationValue, Timestamp};

fn now_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i64,
        Err(_) => 0,
    }
}

/// Returns the current timestamp.
pub fn now() -> Timestamp {
    Timestamp::from_unix_ms(now_ms())
}

/// Sleeps for the given number of milliseconds.
pub fn sleep(ms: i64) {
    if ms <= 0 {
        return;
    }
    thread::sleep(Duration::from_millis(ms as u64));
}

/// Returns the duration elapsed since a timestamp.
pub fn since(timestamp: Timestamp) -> DurationValue {
    DurationValue::from_ms(now_ms().saturating_sub(timestamp.unix()))
}
