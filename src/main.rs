use std::fs;

use system_monitor::parser::Parser;

fn main() {
    let parser = Parser::new();

    for entry in fs::read_dir("/proc").unwrap() {
        let process_path = entry.unwrap().path();
        if process_path.is_dir() {
            let process_path_string = process_path.display().to_string();
            let process = parser.parse_process(&process_path_string);
            println!("{:#?}", process);
        }
    }
}
