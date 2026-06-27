use std::time::SystemTime;
use time::{OffsetDateTime, UtcOffset};

pub fn format_ts(value: Option<SystemTime>, tz_offset_hours: i8) -> String {
    let Some(value) = value else {
        return "n/a".to_string();
    };
    let offset = UtcOffset::from_hms(tz_offset_hours, 0, 0).unwrap_or(UtcOffset::UTC);
    let dt = OffsetDateTime::from(value).to_offset(offset);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

pub fn format_ts_ms(ms: i64, tz_offset_hours: i8) -> String {
    let Ok(millis) = u64::try_from(ms) else {
        return "n/a".to_string();
    };
    let ts = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(millis);
    format_ts(Some(ts), tz_offset_hours)
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}
