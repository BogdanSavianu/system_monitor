use dioxus::prelude::*;

#[component]
pub fn FilterBar(filter_text: String, on_change: EventHandler<String>) -> Element {
    rsx! {
        div {
            class: "demo-filter",
            label { "Filter pid/name: " }
            input {
                value: "{filter_text}",
                oninput: move |event| on_change.call(event.value()),
            }
        }
    }
}
