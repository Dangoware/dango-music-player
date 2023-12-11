use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};

pub fn get_bytes<const S: usize>(iterator: &mut std::vec::IntoIter<u8>) -> [u8; S] {
    let mut bytes = [0; S];

    for i in 0..S {
        bytes[i] = iterator.next().unwrap();
    }

    return bytes;
}

pub fn get_bytes_vec(iterator: &mut std::vec::IntoIter<u8>, number: usize) -> Vec<u8> {
    let mut bytes = Vec::new();

    for _ in 0..number {
        bytes.push(iterator.next().unwrap());
    }

    return bytes;
}

/// Converts the windows DateTime into Chrono DateTime
pub fn get_datetime(iterator: &mut std::vec::IntoIter<u8>, topbyte: bool) -> DateTime<Utc> {
    let mut datetime_i64 = i64::from_le_bytes(get_bytes(iterator));

    if topbyte {
        // Zero the topmost byte
        datetime_i64 = datetime_i64 & 0x00FFFFFFFFFFFFFFF;
    }

    if datetime_i64 <= 0 {
        return Utc.timestamp_opt(0, 0).unwrap();
    }

    let unix_time_ticks = datetime_i64 - 621355968000000000;

    let unix_time_seconds = unix_time_ticks / 10000000;

    let unix_time_nanos = match (unix_time_ticks as f64 / 10000000.0) - unix_time_seconds as f64
        > 0.0
    {
        true => ((unix_time_ticks as f64 / 10000000.0) - unix_time_seconds as f64) * 1000000000.0,
        false => 0.0,
    };

    Utc.timestamp_opt(unix_time_seconds, unix_time_nanos as u32)
        .unwrap()
}
