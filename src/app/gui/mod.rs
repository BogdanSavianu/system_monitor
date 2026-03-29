mod app;
#[cfg(feature = "dioxus-gui")]
mod backend;
#[cfg(feature = "dioxus-gui")]
mod components;
#[cfg(feature = "dioxus-gui")]
mod runtime;
#[cfg(feature = "dioxus-gui")]
mod state;
mod view_models;
#[cfg(feature = "dioxus-gui")]
mod views;

use system_monitor::util::ParseError;

pub fn run_gui_mode() -> Result<(), ParseError> {
    app::run_gui_app()
}
