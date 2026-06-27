use std::time::{Instant, SystemTime, UNIX_EPOCH};

use system_monitor::storage::StoredSessionInfo;
use time::{OffsetDateTime, PrimitiveDateTime, format_description::well_known::Rfc3339};

use super::types::{
    ReplayAllocationPoint, ReplayLeakedBlock, ReplaySample, ReplayScanMarker, ReplaySource,
};

#[derive(Debug, Clone)]
pub struct ReplayState {
    pub start_at_input: String,
    pub end_at_input: String,
    pub name_lookup_input: String,
    pub name_suggestions: Vec<String>,
    pub speed: f64,
    pub is_playing: bool,
    pub is_loading: bool,
    pub cursor_ms: Option<i64>,
    pub last_tick_at: Option<Instant>,
    pub samples: Vec<ReplaySample>,
    pub source: ReplaySource,
    pub message: String,
    pub sessions_list: Vec<StoredSessionInfo>,
    pub sessions_loading: bool,
    pub scan_markers: Vec<ReplayScanMarker>,
    pub scans_loading: bool,
    pub selected_scan_id: Option<i64>,
    pub selected_scan_blocks: Vec<ReplayLeakedBlock>,
    pub blocks_loading: bool,
    pub allocation_series: Vec<ReplayAllocationPoint>,
    pub allocation_loading: bool,
}

impl ReplayState {
    pub fn new() -> Self {
        let now_ms = now_unix_ms();
        let start_ms = now_ms.saturating_sub(24 * 60 * 60 * 1000);

        Self {
            start_at_input: format_datetime_input(start_ms),
            end_at_input: format_datetime_input(now_ms),
            name_lookup_input: String::new(),
            name_suggestions: Vec::new(),
            speed: 1.0,
            is_playing: false,
            is_loading: false,
            cursor_ms: None,
            last_tick_at: None,
            samples: Vec::new(),
            source: ReplaySource::None,
            message: "Enter a process name and time range, then click Load".to_string(),
            sessions_list: Vec::new(),
            sessions_loading: false,
            scan_markers: Vec::new(),
            scans_loading: false,
            selected_scan_id: None,
            selected_scan_blocks: Vec::new(),
            blocks_loading: false,
            allocation_series: Vec::new(),
            allocation_loading: false,
        }
    }

    pub fn set_name_lookup_input(&mut self, value: String) {
        self.name_lookup_input = value;
        self.name_suggestions.clear();
    }

    pub fn set_name_suggestions(&mut self, names: Vec<String>) {
        self.name_suggestions = names;
    }

    pub fn pick_name_suggestion(&mut self, name: String) {
        self.name_lookup_input = name;
        self.name_suggestions.clear();
    }

    pub fn set_start_at_input(&mut self, value: String) {
        self.start_at_input = value;
    }

    pub fn set_end_at_input(&mut self, value: String) {
        self.end_at_input = value;
    }

    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed;
    }

    pub fn build_name_request(&self) -> Result<(String, i64, i64), String> {
        let name = self.name_lookup_input.trim().to_string();
        if name.is_empty() {
            return Err("enter a process name to look up".to_string());
        }
        let start_ms = parse_datetime_input_to_unix_ms(&self.start_at_input)?;
        let end_ms = parse_datetime_input_to_unix_ms(&self.end_at_input)?;
        if end_ms <= start_ms {
            return Err("end timestamp must be greater than start timestamp".to_string());
        }
        Ok((name, start_ms, end_ms))
    }

    pub fn mark_loading(&mut self) {
        self.is_loading = true;
        self.is_playing = false;
        self.last_tick_at = None;
        self.samples.clear();
        self.cursor_ms = None;
        self.source = ReplaySource::None;
        self.message = "Loading replay samples...".to_string();
        self.scan_markers.clear();
        self.scans_loading = true;
        self.selected_scan_id = None;
        self.selected_scan_blocks.clear();
        self.blocks_loading = false;
        self.allocation_series.clear();
        self.allocation_loading = true;
    }

    pub fn apply_samples(
        &mut self,
        samples: Vec<ReplaySample>,
        source: ReplaySource,
        message: String,
    ) {
        self.is_loading = false;
        self.source = source;
        self.samples = samples;
        self.cursor_ms = self.samples.first().map(|sample| sample.collected_at_ms);
        self.is_playing = false;
        self.last_tick_at = None;
        self.message = message;
    }

    pub fn set_scan_markers(&mut self, scans: Vec<ReplayScanMarker>) {
        self.scan_markers = scans;
        self.scans_loading = false;
    }

    pub fn set_allocation_series(&mut self, series: Vec<ReplayAllocationPoint>) {
        self.allocation_series = series;
        self.allocation_loading = false;
    }

    /// moves the cursor to the scan's timestamp and marks blocks as loading
    /// the caller then fetches them, landing via set_selected_scan_blocks
    pub fn select_scan(&mut self, scan_id: i64) {
        let scan_ts = self
            .scan_markers
            .iter()
            .find(|m| m.id == scan_id)
            .map(|m| m.started_at_ms);
        self.selected_scan_id = Some(scan_id);
        self.blocks_loading = true;
        self.selected_scan_blocks.clear();
        if let Some(ts) = scan_ts {
            self.cursor_ms = Some(ts);
            self.is_playing = false;
            self.last_tick_at = None;
        }
    }

    pub fn set_selected_scan_blocks(&mut self, scan_id: i64, blocks: Vec<ReplayLeakedBlock>) {
        if self.selected_scan_id != Some(scan_id) {
            return;
        }
        self.selected_scan_blocks = blocks;
        self.blocks_loading = false;
    }

    pub fn close_scan_modal(&mut self) {
        self.selected_scan_id = None;
        self.selected_scan_blocks.clear();
        self.blocks_loading = false;
    }

    pub fn set_error(&mut self, message: String) {
        self.is_loading = false;
        self.is_playing = false;
        self.last_tick_at = None;
        self.message = message;
    }

    pub fn toggle_play_pause(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        self.is_playing = !self.is_playing;
        self.last_tick_at = if self.is_playing {
            Some(Instant::now())
        } else {
            None
        };
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.last_tick_at = None;
        self.cursor_ms = self.samples.first().map(|sample| sample.collected_at_ms);
    }

    pub fn seek_to_ratio(&mut self, ratio: f64) {
        let Some(first) = self.samples.first() else {
            return;
        };
        let Some(last) = self.samples.last() else {
            return;
        };

        let ratio = ratio.clamp(0.0, 1.0);
        let span = last.collected_at_ms.saturating_sub(first.collected_at_ms);
        let offset = (span as f64 * ratio).round() as i64;
        self.cursor_ms = Some(first.collected_at_ms.saturating_add(offset));

        if self.is_playing {
            self.last_tick_at = Some(Instant::now());
        }
    }

    pub fn seek_by_seconds(&mut self, seconds: i64) {
        let Some(first) = self.samples.first() else {
            return;
        };
        let Some(last) = self.samples.last() else {
            return;
        };

        let base = self.cursor_ms.unwrap_or(first.collected_at_ms);
        let shifted = base.saturating_add(seconds.saturating_mul(1000));
        self.cursor_ms = Some(shifted.clamp(first.collected_at_ms, last.collected_at_ms));

        if self.is_playing {
            self.last_tick_at = Some(Instant::now());
        }
    }

    pub fn current_ratio(&self) -> f64 {
        let Some(first) = self.samples.first() else {
            return 0.0;
        };
        let Some(last) = self.samples.last() else {
            return 0.0;
        };
        let Some(cursor) = self.cursor_ms else {
            return 0.0;
        };

        let span = last.collected_at_ms.saturating_sub(first.collected_at_ms);
        if span == 0 {
            return 1.0;
        }

        let offset = cursor.saturating_sub(first.collected_at_ms);
        offset as f64 / span as f64
    }

    pub fn apply_preset_range(&mut self, lookback_ms: i64) {
        let end_ms = now_unix_ms();
        let start_ms = end_ms.saturating_sub(lookback_ms);
        self.start_at_input = format_datetime_input(start_ms);
        self.end_at_input = format_datetime_input(end_ms);
    }

    pub fn apply_session_range(&mut self, first_ms: i64, last_ms: i64) {
        self.start_at_input = format_datetime_input(first_ms);
        self.end_at_input = format_datetime_input(last_ms);
    }

    pub fn set_sessions_list(&mut self, sessions: Vec<StoredSessionInfo>) {
        self.sessions_list = sessions;
        self.sessions_loading = false;
    }

    pub fn mark_sessions_loading(&mut self) {
        self.sessions_loading = true;
    }
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

fn parse_datetime_input_to_unix_ms(value: &str) -> Result<i64, String> {
    let s = value.trim();

    let formats = [
        "[year]-[month]-[day]T[hour]:[minute]:[second]",
        "[year]-[month]-[day]T[hour]:[minute]",
        "[year]-[month]-[day] [hour]:[minute]:[second]",
    ];

    for fmt_str in &formats {
        if let Ok(fmt) = time::format_description::parse(fmt_str)
            && let Ok(parsed) = PrimitiveDateTime::parse(s, &fmt)
        {
            let unix_ms = parsed.assume_utc().unix_timestamp_nanos() / 1_000_000;
            return i64::try_from(unix_ms)
                .map_err(|_| "datetime is out of supported timestamp range".to_string());
        }
    }

    Err("invalid datetime".to_string())
}

fn format_datetime_input(unix_ms: i64) -> String {
    let seconds = unix_ms.div_euclid(1000);
    match OffsetDateTime::from_unix_timestamp(seconds) {
        Ok(datetime) => datetime
            .format(&Rfc3339)
            .ok()
            .map(|text| {
                text.split('.')
                    .next()
                    .unwrap_or(&text)
                    .trim_end_matches('Z')
                    .replace('T', " ")
            })
            .unwrap_or_else(|| "1970-01-01 00:00:00".to_string()),
        Err(_) => "1970-01-01 00:00:00".to_string(),
    }
}
