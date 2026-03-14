use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::util::{parser_utils::*, types::*};

const BASE_PROC_PATH: &str = "/proc";

pub trait TraitThreadParser {
    fn get_thread_stat_info(&self, pid: Pid, tid: Tid) -> Result<(u64, u64), ParseError>;
}

pub struct ThreadParser;

impl ThreadParser {
    pub fn new() -> Self {
        ThreadParser {}
    }

    fn parse_stat_info<R>(&self, mut buf_reader: R) -> Result<(u64, u64), ParseError>
    where
        R: BufRead,
    {
        let mut content = String::new();
        let size_read = buf_reader
            .read_to_string(&mut content)
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;
        if size_read == 0 {
            return Err(ParseError::ParsingError("Stat file has 0 bytes".to_string()));
        }

        let content = content.trim();

        let comm_start = content
            .find('(')
            .ok_or_else(|| ParseError::ParsingError("Stat file has wrong format".to_string()))?;
        let comm_end = content
            .rfind(") ")
            .ok_or_else(|| ParseError::ParsingError("Stat file has wrong format".to_string()))?;

        if comm_end <= comm_start {
            return Err(ParseError::ParsingError("Stat file has wrong format".to_string()));
        }

        let after_comm = &content[(comm_end + 2)..];
        let fields: Vec<&str> = after_comm.split_whitespace().collect();

        if fields.len() <= 12 {
            return Err(ParseError::ParsingError(
                "Stat file has too few fields".to_string(),
            ));
        }

        let utime = fields[11]
            .parse::<u64>()
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let stime = fields[12]
            .parse::<u64>()
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;

        Ok((utime, stime))
    }
}

impl TraitThreadParser for ThreadParser {
    fn get_thread_stat_info(&self, pid: Pid, tid: Tid) -> Result<(u64, u64), ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/task/{tid}/stat");
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        self.parse_stat_info(buf_reader)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::ThreadParser;

    #[test]
    fn parse_stat_info_reads_utime_and_stime() {
        let parser = ThreadParser::new();
        let input = "1234 (thread-name) S 1 2 3 4 5 6 7 8 9 10 11 42 84 15 16";

        let (utime, stime) = parser
            .parse_stat_info(Cursor::new(input))
            .expect("thread stat info should parse");

        assert_eq!(utime, 11);
        assert_eq!(stime, 42);
    }
}
