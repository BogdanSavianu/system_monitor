use dioxus::prelude::*;

use crate::app::gui::state::GuiPage;

#[component]
pub fn AppNav(active_page: GuiPage, on_change: EventHandler<GuiPage>) -> Element {
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