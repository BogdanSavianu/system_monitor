use std::fs::File;
use std::io::{BufRead, BufReader, Read};

use crate::model::ThreadStatInfoModel;
use crate::thread::Thread;
use crate::util::{parser_utils::*, types::*};

const BASE_PROC_PATH: &str = "/proc";

pub trait TraitThreadParser {
    fn get_thread_stat_info(&self, pid: Pid, tid: Tid) -> Result<(u64, u64), ParseError>;
    fn parse_thread(&self, pid: Pid, thread: Thread) -> Thread;
}

pub struct ThreadParser;

impl ThreadParser {
    pub fn new() -> Self {
        ThreadParser {}
    }

    fn parse_stat_info<R>(&self, mut buf_reader: R) -> Result<ThreadStatInfoModel, ParseError>
    where
        R: BufRead,
    {
        let mut content = String::new();
        let size_read = buf_reader
            .read_to_string(&mut content)
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;
        if size_read == 0 {
            return Err(ParseError::ParsingError(
                "Stat file has 0 bytes".to_string(),
            ));
        }

        let content = content.trim();

        let comm_start = content
            .find('(')
            .ok_or_else(|| ParseError::ParsingError("Stat file has wrong format".to_string()))?;
        let comm_end = content
            .rfind(") ")
            .ok_or_else(|| ParseError::ParsingError("Stat file has wrong format".to_string()))?;

        if comm_end <= comm_start {
            return Err(ParseError::ParsingError(
                "Stat file has wrong format".to_string(),
            ));
        }

        let after_comm = &content[(comm_end + 2)..];
        let fields: Vec<&str> = after_comm.split_whitespace().collect();

        if fields.len() <= 12 {
            return Err(ParseError::ParsingError(
                "Stat file has too few fields".to_string(),
            ));
        }

        let state = fields[0].chars().next();
        let utime = fields[11]
            .parse::<u64>()
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let stime = fields[12]
            .parse::<u64>()
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;

        let last_cpu = fields.get(36).and_then(|value| value.parse::<u32>().ok());

        Ok(ThreadStatInfoModel {
            utime,
            stime,
            state,
            last_cpu,
        })
    }

    fn parse_status_info<R>(&self, reader: R, thread: &mut Thread) -> Result<(), ParseError>
    where
        R: BufRead,
    {
        for line in reader.lines() {
            let line = line.map_err(|err| ParseError::ParsingError(err.to_string()))?;
            let mut parts = line.split_whitespace();
            let key = parts.next();
            let value = parts.next();

            match (key, value) {
                (Some("Name:"), Some(val)) => {
                    thread.name = val.to_string();
                }
                (Some("voluntary_ctxt_switches:"), Some(val)) => {
                    thread.voluntary_ctxt_switches = val.parse::<u64>().ok();
                }
                (Some("nonvoluntary_ctxt_switches:"), Some(val)) => {
                    thread.nonvoluntary_ctxt_switches = val.parse::<u64>().ok();
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn parse_io_info<R>(&self, reader: R, thread: &mut Thread) -> Result<(), ParseError>
    where
        R: BufRead,
    {
        for line in reader.lines() {
            let line = line.map_err(|err| ParseError::ParsingError(err.to_string()))?;
            let mut parts = line.split_whitespace();
            let key = parts.next();
            let value = parts.next();

            match (key, value) {
                (Some("rchar:"), Some(val)) => {
                    thread.io_rchar = val.parse::<u64>().ok();
                }
                (Some("wchar:"), Some(val)) => {
                    thread.io_wchar = val.parse::<u64>().ok();
                }
                (Some("syscr:"), Some(val)) => {
                    thread.io_syscr = val.parse::<u64>().ok();
                }
                (Some("syscw:"), Some(val)) => {
                    thread.io_syscw = val.parse::<u64>().ok();
                }
                (Some("read_bytes:"), Some(val)) => {
                    thread.io_read_bytes = val.parse::<u64>().ok();
                }
                (Some("write_bytes:"), Some(val)) => {
                    thread.io_write_bytes = val.parse::<u64>().ok();
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn get_thread_base_path(&self, pid: Pid, tid: Tid) -> String {
        format!("{BASE_PROC_PATH}/{pid}/task/{tid}")
    }

    fn get_thread_comm(&self, pid: Pid, tid: Tid) -> Result<String, ParseError> {
        let file_path = format!("{}/comm", self.get_thread_base_path(pid, tid));
        let file =
            File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let mut reader = BufReader::new(file);
        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;

        Ok(content.trim_end_matches('\n').to_string())
    }
}

impl TraitThreadParser for ThreadParser {
    fn get_thread_stat_info(&self, pid: Pid, tid: Tid) -> Result<(u64, u64), ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/task/{tid}/stat");
        let file =
            File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        let stat_info = self.parse_stat_info(buf_reader)?;

        Ok((stat_info.utime, stat_info.stime))
    }

    fn parse_thread(&self, pid: Pid, mut thread: Thread) -> Thread {
        let tid = thread.tid;

        let stat_path = format!("{}/stat", self.get_thread_base_path(pid, tid));
        if let Ok(file) = File::open(stat_path) {
            if let Ok(stat_info) = self.parse_stat_info(BufReader::new(file)) {
                thread.state = stat_info.state;
                thread.last_cpu = stat_info.last_cpu;
            }
        }

        if thread.name.is_empty() {
            if let Ok(name) = self.get_thread_comm(pid, tid) {
                thread.name = name;
            }
        }

        let status_path = format!("{}/status", self.get_thread_base_path(pid, tid));
        if let Ok(file) = File::open(status_path) {
            let _ = self.parse_status_info(BufReader::new(file), &mut thread);
        }

        let io_path = format!("{}/io", self.get_thread_base_path(pid, tid));
        if let Ok(file) = File::open(io_path) {
            let _ = self.parse_io_info(BufReader::new(file), &mut thread);
        }

        thread
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

        let stat = parser
            .parse_stat_info(Cursor::new(input))
            .expect("thread stat info should parse");

        assert_eq!(stat.utime, 11);
        assert_eq!(stat.stime, 42);
        assert_eq!(stat.state, Some('S'));
    }

    #[test]
    fn parse_status_info_reads_context_switches() {
        let parser = ThreadParser::new();
        let input = "Name:\tworker\nvoluntary_ctxt_switches:\t30\nnonvoluntary_ctxt_switches:\t5\n";
        let mut thread = crate::thread::Thread::new(101);

        parser
            .parse_status_info(Cursor::new(input), &mut thread)
            .expect("thread status info should parse");

        assert_eq!(thread.name, "worker");
        assert_eq!(thread.voluntary_ctxt_switches, Some(30));
        assert_eq!(thread.nonvoluntary_ctxt_switches, Some(5));
    }

    #[test]
    fn parse_io_info_reads_core_io_counters() {
        let parser = ThreadParser::new();
        let input =
            "rchar: 1000\nwchar: 2000\nsyscr: 10\nsyscw: 20\nread_bytes: 300\nwrite_bytes: 400\n";
        let mut thread = crate::thread::Thread::new(333);

        parser
            .parse_io_info(Cursor::new(input), &mut thread)
            .expect("thread io should parse");

        assert_eq!(thread.io_rchar, Some(1000));
        assert_eq!(thread.io_wchar, Some(2000));
        assert_eq!(thread.io_syscr, Some(10));
        assert_eq!(thread.io_syscw, Some(20));
        assert_eq!(thread.io_read_bytes, Some(300));
        assert_eq!(thread.io_write_bytes, Some(400));
    }
}
