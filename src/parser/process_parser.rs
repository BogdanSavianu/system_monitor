use std::num::ParseIntError;

use crate::process::Process;
use crate::util::parser_utils::*;

pub struct ProcessParser;

impl From<ParseIntError> for ParseError {
    fn from(err: ParseIntError) -> ParseError {
        ParseError::ParsingError(err.to_string())
    }
}

impl ProcessParser {
    pub fn new() -> Self {
        ProcessParser {}
    }

    pub fn parse_process(&self, file_path: &String) -> Result<Process, ParseError> {
        let pid = extract_pid_from_path(file_path)?;
        let mut process = Process::new(pid);
        let _ = process.collect_threads();

        Ok (process)
    }
}
