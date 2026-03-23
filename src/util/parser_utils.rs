use std::num::ParseIntError;
use std::thread;

#[derive(Debug)]
pub enum ParseError {
    NonExistingResource(u32),
    ParsingError(String),
}

impl From<ParseIntError> for ParseError {
    fn from(err: ParseIntError) -> ParseError {
        ParseError::ParsingError(err.to_string())
    }
}

pub fn extract_pid_from_path(path: &String) -> Result<u32, ParseError> {
    path.split("/")
        .nth(2)
        .ok_or_else(|| ParseError::ParsingError(path.clone()))?
        .parse::<u32>()
        .map_err(|e| ParseError::ParsingError(format!("invalid pid in path: '{}': '{}'", path, e)))
}

pub fn extract_tid_from_path(path: &String) -> Result<u32, ParseError> {
    path.split("/")
        .nth(4)
        .ok_or_else(|| ParseError::ParsingError(path.clone()))?
        .parse::<u32>()
        .map_err(|e| ParseError::ParsingError(format!("invalid tid in path: '{}': '{}'", path, e)))
}

/// Returns how many workers to use.
/// It is at least 1 and at most the available CPU count.
///
/// # Examples
///
/// On a 4-core machine:
/// - total_items = 0 -> 1 worker
/// - total_items = 3 -> 3 workers
/// - total_items = 20 -> 4 workers
pub fn worker_count(total_items: usize) -> usize {
    let available = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let min_required_workers = 1;
    let requested_workers = total_items.max(min_required_workers);

    available.min(requested_workers)
}
