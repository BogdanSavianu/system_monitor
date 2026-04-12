use dioxus::prelude::*;

use crate::app::gui::components::{SettingsSection, SettingsToggle};

#[component]
pub fn SettingsView(
    storage_enabled: bool,
    anomaly_enabled: bool,
    on_storage_toggle: EventHandler<bool>,
    on_anomaly_toggle: EventHandler<bool>,
    on_reset: EventHandler<()>,
) -> Element {
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
        }
    }
}
