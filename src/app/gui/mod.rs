mod app;
#[cfg(feature = "dioxus-gui")]
mod backend;
#[cfg(feature = "dioxus-gui")]
mod replay;
#[cfg(feature = "dioxus-gui")]
mod components;
#[cfg(feature = "dioxus-gui")]
mod runtime;
#[cfg(feature = "dioxus-gui")]
mod fmt;
#[cfg(feature = "dioxus-gui")]
mod settings_store;
#[cfg(feature = "dioxus-gui")]
mod state;
#[cfg(feature = "dioxus-gui")]
mod view_models;
#[cfg(feature = "dioxus-gui")]
mod views;

use system_monitor::util::ParseError;

pub fn run_gui_mode() -> Result<(), ParseError> {
    app::run_gui_app()
}
