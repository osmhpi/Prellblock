//! Utils for time conversion.

use std::{
    convert::TryInto,
    time::{Duration, SystemTime},
};

/// Convert a given `time` to a byte array.
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn system_time_to_bytes(time: SystemTime) -> impl AsRef<[u8]> {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as i64,
        Err(err) => -(err.duration().as_nanos() as i64),
    }
    .to_be_bytes()
}

/// Convert a given byte array to a timestamp.
///
/// # Panics
/// The function panics when the byte array cannot be interpreted as timestamp.
#[allow(clippy::cast_sign_loss)]
#[must_use]
pub fn system_time_from_bytes(bytes: &[u8]) -> SystemTime {
    let time = i64::from_be_bytes(bytes.try_into().unwrap());
    if time >= 0 {
        let duration = Duration::from_nanos(time as u64);
        SystemTime::UNIX_EPOCH + duration
    } else {
        let duration = Duration::from_nanos((-(time + 1)) as u64 + 1);
        SystemTime::UNIX_EPOCH - duration
    }
}
