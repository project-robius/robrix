use chrono::NaiveDateTime;
use matrix_sdk::ruma::MilliSecondsSinceUnixEpoch;



pub fn unix_time_millis_to_datetime(millis: &MilliSecondsSinceUnixEpoch) -> Option<NaiveDateTime> {
    let millis: i64 = millis.get().into();
    NaiveDateTime::from_timestamp_millis(millis)
}
