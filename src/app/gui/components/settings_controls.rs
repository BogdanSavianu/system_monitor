use dioxus::prelude::*;

#[component]
pub fn SettingsSection(title: String, description: String, children: Element) -> Element {
    rsx! {
        section {
            class: "settings-section",
            h3 { class: "settings-title", "{title}" }
            p { class: "settings-description", "{description}" }
            div {
                class: "settings-body",
                {children}
            }
        }
    }
}

#[component]
pub fn SettingsToggle(
    label: String,
    hint: String,
    enabled: bool,
    on_toggle: EventHandler<bool>,
) -> Element {
    rsx! {
        label {
            class: "settings-toggle",
            div {
                class: "settings-toggle-text",
                div { class: "settings-toggle-label", "{label}" }
                div { class: "settings-toggle-hint", "{hint}" }
            }
            input {
                r#type: "checkbox",
                checked: enabled,
                onclick: move |_| on_toggle.call(!enabled),
            }
        }
    }
}
