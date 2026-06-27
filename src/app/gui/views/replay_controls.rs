use dioxus::prelude::*;

use crate::app::gui::fmt::format_ts_ms;
use crate::app::gui::replay::state::ReplayState;

pub struct ReplayControlsCallbacks {
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
    pub on_load_sessions: EventHandler<()>,
    pub on_session_pick: EventHandler<(i64, i64)>,
}

pub fn render_replay_controls(
    replay: &ReplayState,
    tz_offset_hours: i8,
    cb: ReplayControlsCallbacks,
) -> Element {
    let ReplayControlsCallbacks {
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
    } = cb;

    rsx! {
        div {
            class: "replay-controls",

            label {
                "Process name"
                input {
                    r#type: "text",
                    placeholder: "e.g. firefox",
                    value: replay.name_lookup_input.clone(),
                    oninput: move |event| {
                        let value = event.value();
                        on_name_lookup_change.call(value.clone());
                        if value.trim().len() >= 2 {
                            on_name_search.call(value);
                        }
                    },
                }
            }

            if !replay.name_suggestions.is_empty() {
                ul {
                    class: "replay-filter-dropdown",
                    for name in replay.name_suggestions.clone() {
                        li {
                            button {
                                class: if replay.name_lookup_input == name {
                                    "replay-filter-option selected"
                                } else {
                                    "replay-filter-option"
                                },
                                onclick: {
                                    let n = name.clone();
                                    move |_| on_name_pick.call(n.clone())
                                },
                                "{name}"
                            }
                        }
                    }
                }
            }

            label {
                "Start (UTC) - YYYY-MM-DD HH:MM:SS"
                input {
                    r#type: "text",
                    value: replay.start_at_input.clone(),
                    oninput: move |event| on_start_change.call(event.value()),
                }
            }

            label {
                "End (UTC) - YYYY-MM-DD HH:MM:SS"
                input {
                    r#type: "text",
                    value: replay.end_at_input.clone(),
                    oninput: move |event| on_end_change.call(event.value()),
                }
            }

            div {
                class: "replay-presets",
                for (label, ms) in [
                    ("Last 15m", 15 * 60 * 1000_i64),
                    ("Last 1h",  60 * 60 * 1000_i64),
                    ("Last 6h",  6 * 3600 * 1000_i64),
                    ("Last 24h", 24 * 3600 * 1000_i64),
                ] {
                    button {
                        onclick: move |_| on_preset.call(ms),
                        "{label}"
                    }
                }
            }

            div {
                class: "replay-sessions",
                div {
                    class: "replay-sessions-header",
                    span { "Past sessions" }
                    button {
                        class: "replay-sessions-refresh",
                        disabled: replay.sessions_loading,
                        onclick: move |_| on_load_sessions.call(()),
                        if replay.sessions_loading { "Loading..." } else { "Refresh" }
                    }
                }
                if replay.sessions_list.is_empty() {
                    p {
                        class: "detected-leaks-subtitle",
                        if replay.name_lookup_input.trim().is_empty() {
                            "No sessions loaded - click Refresh to fetch from database"
                        } else {
                            "No sessions loaded - click Refresh (results will be filtered to sessions where this process ran)"
                        }
                    }
                } else {
                    ul {
                        class: "replay-sessions-list",
                        for session in &replay.sessions_list {
                            li {
                                button {
                                    class: "replay-session-option",
                                    onclick: {
                                        let first = session.first_seen_ms;
                                        let last = session.last_seen_ms;
                                        move |_| on_session_pick.call((first, last))
                                    },
                                    "{format_ts_ms(session.first_seen_ms, tz_offset_hours)} → {format_ts_ms(session.last_seen_ms, tz_offset_hours)}"
                                }
                            }
                        }
                    }
                }
            }

            button {
                class: "replay-primary-btn",
                disabled: replay.is_loading || replay.name_lookup_input.trim().is_empty(),
                onclick: move |_| on_load.call(()),
                if replay.is_loading { "Loading..." } else { "Load" }
            }

            div {
                class: "replay-transport",
                button {
                    disabled: replay.samples.is_empty(),
                    onclick: move |_| on_seek_back.call(()),
                    "<< 10s"
                }
                button {
                    disabled: replay.samples.is_empty(),
                    onclick: move |_| on_toggle_play_pause.call(()),
                    if replay.is_playing { "Pause" } else { "Play" }
                }
                button {
                    disabled: replay.samples.is_empty(),
                    onclick: move |_| on_stop.call(()),
                    "Stop"
                }
                button {
                    disabled: replay.samples.is_empty(),
                    onclick: move |_| on_seek_forward.call(()),
                    "10s >>"
                }
                select {
                    value: replay.speed.to_string(),
                    onchange: move |event| on_speed_change.call(event.value()),
                    for speed in [0.25_f64, 0.5, 1.0, 2.0, 4.0, 8.0] {
                        option {
                            value: "{speed}",
                            "{speed}x"
                        }
                    }
                }
            }
        }
    }
}
