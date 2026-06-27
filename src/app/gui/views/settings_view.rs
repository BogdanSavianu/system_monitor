use dioxus::prelude::*;

use crate::app::gui::components::{SettingsSection, SettingsToggle};

const TZ_OPTIONS: &[(&str, i8)] = &[
    ("UTC", 0),
    ("UTC+1 (CET)", 1),
    ("UTC+2 (EET / Romania winter)", 2),
    ("UTC+3 (EEST / Romania summer)", 3),
    ("UTC+4", 4),
    ("UTC-1", -1),
    ("UTC-2", -2),
    ("UTC-3", -3),
    ("UTC-4 (EDT)", -4),
    ("UTC-5 (EST / CDT)", -5),
    ("UTC-6 (CST / MDT)", -6),
    ("UTC-7 (MST / PDT)", -7),
    ("UTC-8 (PST)", -8),
];

#[component]
pub fn SettingsView(
    storage_enabled: bool,
    anomaly_enabled: bool,
    db_path: String,
    db_size_bytes: Option<u64>,
    tz_offset_hours: i8,
    on_storage_toggle: EventHandler<bool>,
    on_anomaly_toggle: EventHandler<bool>,
    on_tz_change: EventHandler<i8>,
    on_reset: EventHandler<()>,
    on_reset_history: EventHandler<()>,
) -> Element {
    let db_size_label = match db_size_bytes {
        None => "file not found".to_string(),
        Some(b) if b < 1024 => format!("{} B", b),
        Some(b) if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        Some(b) => format!("{:.2} MB", b as f64 / (1024.0 * 1024.0)),
    };
    let effective_storage_enabled = storage_enabled || anomaly_enabled;

    rsx! {
        div {
            class: "settings-page",
            SettingsSection {
                title: "Detection pipeline".to_string(),
                description: "Configure local history collection and anomaly analysis.".to_string(),
                SettingsToggle {
                    label: "Enable storage".to_string(),
                    hint: "Persist process and network history locally for trends and debugging.".to_string(),
                    enabled: storage_enabled,
                    on_toggle: on_storage_toggle,
                }
                SettingsToggle {
                    label: "Enable anomaly detection".to_string(),
                    hint: "Turns on anomaly detection and automatically requires storage history.".to_string(),
                    enabled: anomaly_enabled,
                    on_toggle: on_anomaly_toggle,
                }
            }

            SettingsSection {
                title: "Effective configuration".to_string(),
                description: "What the runtime currently resolves from your toggle choices.".to_string(),
                div {
                    class: "settings-kv",
                    span { class: "settings-k", "Storage requested" }
                    span { class: "settings-v", if storage_enabled { "on" } else { "off" } }
                }
                div {
                    class: "settings-kv",
                    span { class: "settings-k", "Anomaly requested" }
                    span { class: "settings-v", if anomaly_enabled { "on" } else { "off" } }
                }
                div {
                    class: "settings-kv",
                    span { class: "settings-k", "Effective storage" }
                    span {
                        class: "settings-v",
                        if effective_storage_enabled { "on" } else { "off" }
                    }
                }
                if anomaly_enabled && !storage_enabled {
                    p {
                        class: "settings-note",
                        "Anomaly detection requires stored history. Effective storage is auto-enabled."
                    }
                }
                button {
                    class: "settings-reset-btn",
                    onclick: move |_| on_reset.call(()),
                    "Reset to defaults"
                }
            }

            SettingsSection {
                title: "Display".to_string(),
                description: "How timestamps are shown across the app.".to_string(),
                div {
                    class: "settings-kv",
                    span { class: "settings-k", "Timezone" }
                    select {
                        class: "settings-select",
                        onchange: move |e| {
                            if let Ok(v) = e.value().parse::<i8>() {
                                on_tz_change.call(v);
                            }
                        },
                        for (label, offset) in TZ_OPTIONS {
                            option {
                                value: "{offset}",
                                selected: *offset == tz_offset_hours,
                                "{label}"
                            }
                        }
                    }
                }
            }

            SettingsSection {
                title: "Storage".to_string(),
                description: "SQLite database used for history and replay.".to_string(),
                div {
                    class: "settings-kv",
                    span { class: "settings-k", "Database path" }
                    span { class: "settings-v settings-mono", "{db_path}" }
                }
                div {
                    class: "settings-kv",
                    span { class: "settings-k", "Database size" }
                    span { class: "settings-v", "{db_size_label}" }
                }
                p {
                    class: "settings-note",
                    "Clears every history table - samples, sessions, deep scans, reachability scans, and leaked blocks. The schema is recreated. Settings are preserved."
                }
                button {
                    class: "settings-reset-btn",
                    onclick: move |_| on_reset_history.call(()),
                    "Reset history"
                }
            }
        }
    }
}
