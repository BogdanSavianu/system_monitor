use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::process::Process;
use crate::state::SystemState;
use crate::util::parser_utils::*;
use crate::model::SystemStatusFileModel;

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

    pub fn get_status_info(&self) -> Result<SystemStatusFileModel, ParseError> {
        let file_path = format!("/proc/stat");
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        self.parse_status_info(buf_reader)
    }

    fn parse_status_info<R>(&self, reader: R) -> Result<SystemStatusFileModel, ParseError>
        where R: BufRead,
    {
        let mut total_cpu: Option<u64> = None;
        let mut cpus: Vec<u64> = Vec::new();
        let mut cpu_section_started = false;

        for line in reader.lines() {
            let line = line.map_err(|err| ParseError::ParsingError(err.to_string()))?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let key = parts[0];
            if key == "cpu" {
                total_cpu = Some(self.sum_cpu_jiffies(&parts[1..])?);
                cpu_section_started = true;
                continue;
            }

            if let Some(index) = key.strip_prefix("cpu") {
                if !index.is_empty() && index.chars().all(|ch| ch.is_ascii_digit()) {
                    cpus.push(self.sum_cpu_jiffies(&parts[1..])?);
                    cpu_section_started = true;
                    continue;
                }
            }

            if cpu_section_started {
                break;
            }
        }

        let total_cpu = total_cpu.ok_or_else(|| {
            ParseError::ParsingError("missing aggregate cpu line in /proc/stat".to_string())
        })?;

        let num_cores = cpus.len() as u8;

        Ok(SystemStatusFileModel::build(total_cpu, cpus, num_cores))
    }


    fn sum_cpu_jiffies(&self, fields: &[&str]) -> Result<u64, ParseError> {
        if fields.len() < 8 {
            return Err(ParseError::ParsingError(
                "invalid /proc/stat cpu line: expected at least 8 fields".to_string(),
            ));
        }

        fields
            .iter()
            .take(8)
            .map(|x| x.parse::<u64>())
            .sum::<Result<u64, _>>()
            .map_err(ParseError::from)
    }

}
