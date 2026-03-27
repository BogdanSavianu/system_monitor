mod app;

use system_monitor::util::ParseError;

fn main() -> Result<(), ParseError> {
    app::run()
}
