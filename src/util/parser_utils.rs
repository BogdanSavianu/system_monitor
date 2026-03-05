use std::num::ParseIntError;

#[derive(Debug)]
pub enum ParseError {
    NonExistingResource(u32),
    ParsingError(String)
}

pub fn extract_pid_from_path(path: &String) -> Result<u32, ParseError> {
    path
        .split("/")
        .nth(2)
        .ok_or_else(|| ParseError::ParsingError(path.clone()))?
        .parse::<u32>()
        .map_err(|e| ParseError::ParsingError(format!("invalid pid in path: '{}': '{}'", path, e)))
}

pub fn extract_tid_from_path(path: &String) -> Result<u32, ParseError> {
    path
        .split("/")
        .nth(4)
        .ok_or_else(|| ParseError::ParsingError(path.clone()))?
        .parse::<u32>()
        .map_err(|e| ParseError::ParsingError(format!("invalid tid in path: '{}': '{}'", path, e)))
}
