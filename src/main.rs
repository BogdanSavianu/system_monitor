use std::fs;

use system_monitor::{parser::{Parser, ProcessParser}, state::SystemState};

fn main() {
    let mut system_state = SystemState::new();
    let process_parser = ProcessParser::new();
    let parser = Parser::new(process_parser);

    for entry in fs::read_dir("/proc").unwrap() {
        let process_path = entry.unwrap().path();
        if process_path.is_dir() {
            let process_path_string = process_path.display().to_string();
            let _process = parser.parse_process(&mut system_state, &process_path_string);
            //println!("{:#?}", process);
        }
    }

    println!("{:#?}", system_state);
    println!("{:#?}", parser.get_status_info());
}
