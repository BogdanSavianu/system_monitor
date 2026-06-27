use dioxus::prelude::*;

use crate::app::gui::state::GuiPage;

#[component]
pub fn AppNav(
    active_page: GuiPage,
    leak_alert_count: usize,
    on_change: EventHandler<GuiPage>,
) -> Element {
    rsx! {
        div {
            class: "app-nav",
            button {
                class: if active_page == GuiPage::Monitor {
                    "app-nav-btn active"
                } else {
                    "app-nav-btn"
                },
                onclick: move |_| on_change.call(GuiPage::Monitor),
                "Monitor"
            }
            button {
                class: if active_page == GuiPage::System {
                    "app-nav-btn active"
                } else {
                    "app-nav-btn"
                },
                onclick: move |_| on_change.call(GuiPage::System),
                "System"
            }
            button {
                class: if active_page == GuiPage::Leaks {
                    "app-nav-btn active"
                } else {
                    "app-nav-btn"
                },
                onclick: move |_| on_change.call(GuiPage::Leaks),
                "Leaks"
                if leak_alert_count > 0 {
                    span { class: "nav-badge", "{leak_alert_count}" }
                }
            }
            button {
                class: if active_page == GuiPage::Replay {
                    "app-nav-btn active"
                } else {
                    "app-nav-btn"
                },
                onclick: move |_| on_change.call(GuiPage::Replay),
                "Replay"
            }
            button {
                class: if active_page == GuiPage::Tree {
                    "app-nav-btn active"
                } else {
                    "app-nav-btn"
                },
                onclick: move |_| on_change.call(GuiPage::Tree),
                "Tree"
            }
            button {
                class: if active_page == GuiPage::Settings {
                    "app-nav-btn active"
                } else {
                    "app-nav-btn"
                },
                onclick: move |_| on_change.call(GuiPage::Settings),
                "Settings"
            }
        }
    }
}
