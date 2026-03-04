use crate::process::Process;
use crate::state::SystemState;
use crate::util::parser_utils::*;

pub use super::process_parser::*;

pub struct Parser {
    pub process_parser: ProcessParser
}

impl Parser {
    pub fn new() -> Self {
        Parser { 
            process_parser: ProcessParser::new()
        }
    }

    pub fn parse_process(&self, system_state: &mut SystemState, file_path: &String) -> Result<Process, ParseError> {
        self.process_parser.parse_process(system_state, file_path)
    }
}
