use dioxus::prelude::*;

use crate::app::gui::fmt::format_ts_ms;
use crate::app::gui::replay::{
    data::{
        visible_allocation_points_until_cursor, visible_cpu_points_until_cursor,
        visible_memory_points_until_cursor, visible_samples_until_cursor,
    },
    state::ReplayState,
    types::{ReplaySample, ReplaySource},
};

use super::{
    ReplayControlsCallbacks, render_replay_chart, render_replay_controls, render_replay_timeline,
};

pub struct ReplayViewProps<'a> {
    pub replay: &'a ReplayState,
    pub tz_offset_hours: i8,
    pub on_name_lookup_change: EventHandler<String>,
    pub on_name_search: EventHandler<String>,
    pub on_name_pick: EventHandler<String>,
    pub on_start_change: EventHandler<String>,
    pub on_end_change: EventHandler<String>,
    pub on_preset: EventHandler<i64>,
    pub on_load: EventHandler<()>,
    pub on_toggle_play_pause: EventHandler<()>,
    pub on_stop: EventHandler<()>,
    pub on_seek_back: EventHandler<()>,
    pub on_seek_forward: EventHandler<()>,
    pub on_speed_change: EventHandler<String>,
    pub on_seek_ratio_change: EventHandler<String>,
    pub on_export: EventHandler<()>,
    pub on_load_sessions: EventHandler<()>,
    pub on_session_pick: EventHandler<(i64, i64)>,
    pub on_scan_marker_click: EventHandler<i64>,
}

pub fn render_replay_view(props: ReplayViewProps) -> Element {
    let ReplayViewProps {
        replay,
        tz_offset_hours,
        on_name_lookup_change,
        on_name_search,
        on_name_pick,
        on_start_change,
        on_end_change,
        on_preset,
        on_load,
        on_toggle_play_pause,
        on_stop,
        on_seek_back,
        on_seek_forward,
        on_speed_change,
        on_seek_ratio_change,
        on_export,
        on_load_sessions,
        on_session_pick,
        on_scan_marker_click,
    } = props;

    let cursor_label = replay
        .cursor_ms
        .map_or_else(|| "n/a".to_string(), |ms| format_ts_ms(ms, tz_offset_hours));
    let source_label = match replay.source {
        ReplaySource::None => "none",
        ReplaySource::SqliteByName => "sqlite (by name)",
    };
    let visible_samples = visible_samples_until_cursor(&replay.samples, replay.cursor_ms);
    let memory_points = visible_memory_points_until_cursor(&replay.samples, replay.cursor_ms);
    let cpu_points = visible_cpu_points_until_cursor(&replay.samples, replay.cursor_ms);
    let allocation_points =
        visible_allocation_points_until_cursor(&replay.allocation_series, replay.cursor_ms);
    let sample_count = replay.samples.len();
    let scan_count = replay.scan_markers.len();
    let allocation_count = replay.allocation_series.len();
    let cpu_values: Vec<f64> = cpu_points.iter().map(|(_, v, _)| *v).collect();
    let stats = replay_stats(visible_samples, &cpu_values);

    rsx! {
        div {
            class: "replay-page",

            section {
                class: "list-panel",
                h2 { "Replay" }
                p {
                    class: "detected-leaks-subtitle",
                    "Enter a process name and time range to replay its memory usage"
                }
                p {
                    class: "detected-leaks-subtitle",
                    "Data source: {source_label} | Samples: {sample_count} | Scans: {scan_count} | allocation snapshots: {allocation_count}"
                }
                p {
                    class: "detected-leaks-subtitle",
                    "{replay.message}"
                }

                {render_replay_controls(
                    replay,
                    tz_offset_hours,
                    ReplayControlsCallbacks {
                        on_name_lookup_change,
                        on_name_search,
                        on_name_pick,
                        on_start_change,
                        on_end_change,
                        on_preset,
                        on_load,
                        on_toggle_play_pause,
                        on_stop,
                        on_seek_back,
                        on_seek_forward,
                        on_speed_change,
                        on_load_sessions,
                        on_session_pick,
                    },
                )}

                {render_replay_timeline(replay, &cursor_label, on_seek_ratio_change, on_scan_marker_click)}
            }

            {render_replay_chart(&memory_points, &cpu_points, &allocation_points, tz_offset_hours)}

            if let Some(s) = stats {
                section {
                    class: "list-panel replay-stats-card",
                    div {
                        class: "replay-stats-row",
                        p {
                            class: "detected-leaks-subtitle",
                            "Mem min/max/mean: {s.0:.2} / {s.1:.2} / {s.2:.2} MB  |  CPU mean: {s.6:.1}%  |  Growth: {s.3:+.3} MB/min  |  Anomalous: {s.4}/{s.5}"
                        }
                        button {
                            class: "replay-export-btn",
                            disabled: replay.samples.is_empty(),
                            onclick: move |_| on_export.call(()),
                            "Export CSV"
                        }
                    }
                }
            }
        }
    }
}

fn replay_stats(
    samples: &[ReplaySample],
    cpu_values: &[f64],
) -> Option<(f64, f64, f64, f64, usize, usize, f64)> {
    if samples.len() < 2 {
        return None;
    }
    let values: Vec<f64> = samples.iter().map(|s| s.physical_mem_mb).collect();
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let span_ms =
        samples.last().unwrap().collected_at_ms - samples.first().unwrap().collected_at_ms;
    let growth = if span_ms > 0 {
        (values.last().unwrap() - values.first().unwrap()) / (span_ms as f64 / 60_000.0)
    } else {
        0.0
    };
    let anomalous = samples.iter().filter(|s| s.is_anomalous).count();
    let cpu_mean = if cpu_values.is_empty() {
        0.0
    } else {
        cpu_values.iter().sum::<f64>() / cpu_values.len() as f64
    };
    Some((min, max, mean, growth, anomalous, samples.len(), cpu_mean))
}
