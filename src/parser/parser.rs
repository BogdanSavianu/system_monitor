use crate::process::Process;
use crate::state::SystemState;
use crate::util::parser_utils::*;

pub use super::process_parser::*;

pub struct Parser<ProcParser: TraitProcessParser> {
    pub process_parser: ProcParser
}

impl<ProcParser: TraitProcessParser> Parser<ProcParser> {
    pub fn new(process_parser: ProcParser) -> Self {
        Parser { 
            process_parser,
        }
    }

    pub fn parse_process(&self, system_state: &mut SystemState, file_path: &String) -> Result<Process, ParseError> {
        self.process_parser.parse_process(system_state, file_path)
    }
}
