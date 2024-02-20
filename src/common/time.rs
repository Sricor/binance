use chrono::prelude::Utc;
// use chrono::Duration;

pub fn timestamp_millis() -> i64 {
    let now = Utc::now();

    now.timestamp_millis()
}

// pub fn timestamp_millis_after_days(days: i64) -> i64 {
//     let timestamp = Utc::now() + Duration::days(days);

//     timestamp.timestamp_millis()
// }

// pub fn timestamp_millis_before_days(days: i64) -> i64 {
//     let timestamp = Utc::now() - Duration::days(days);

//     timestamp.timestamp_millis()
// }

// pub fn is_timestamp_millis_expired(timestamp: i64) -> bool {
//     timestamp < timestamp_millis()
// }
