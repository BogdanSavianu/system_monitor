use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};

use crate::model::ProcessStatusFileModel;
use crate::process::Process;
use crate::thread::Thread;
use crate::util::{types::*, parser_utils::*};

const BASE_PROC_PATH: &str = "/proc";

pub trait TraitProcessParser {
    fn parse_process(&self, file_path: &String) -> Result<Process, ParseError>;
    fn get_threads_for_pid(&self, pid: Pid) -> Result<Vec<Thread>, ParseError>;
    fn get_status_info(&self, pid: Pid) -> Result<ProcessStatusFileModel, ParseError>;
    // for now it returns utime and stime used for jiffies
    fn get_stat_info(&self, pid: Pid) -> Result<(u64, u64), ParseError>;
    fn get_process_name(&self, pid: Pid) -> Result<String, ParseError>;
    fn get_process_cmdline(&self, pid: Pid) -> Result<String, ParseError>;
}

pub struct ProcessParser;

impl ProcessParser {
    pub fn new() -> Self {
        ProcessParser {}
    }

    fn parse_status_info<R>(&self, reader: R) -> Result<ProcessStatusFileModel, ParseError>
        where R: BufRead,
    {
        let mut vm_size: Option<Vm> = None;
        let mut pm_size: Option<Pm> = None;
        let mut swap_size: Option<Pm> = None;
        let mut thread_count: Option<u32> = None;

        for line in reader.lines() {
            let line = line.map_err(|err| ParseError::ParsingError(err.to_string()))?;
            let mut parts = line.split_whitespace();
            let key = parts.next();
            let value = parts.next();

            match (key, value) {
                (Some("VmSize:"), Some(val)) => {
                    vm_size = Some(
                        val.parse::<Vm>()
                            .map_err(|err| ParseError::ParsingError(err.to_string()))? as Vm
                    );
                }

                (Some("VmRSS:"), Some(val)) => {
                    pm_size = Some(
                        val.parse::<Pm>()
                            .map_err(|err| ParseError::ParsingError(err.to_string()))? as Pm
                    );
                }

                (Some("VmSwap:"), Some(val)) => {
                    swap_size = Some(
                        val.parse::<Swap>()
                            .map_err(|err| ParseError::ParsingError(err.to_string()))? as Swap
                    );
                }

                (Some("Threads:"), Some(val)) => {
                    thread_count = Some(
                        val.parse::<u32>()
                            .map_err(|err| ParseError::ParsingError(err.to_string()))?
                    );
                }

                _ => {}
            }

            if vm_size.is_some() && pm_size.is_some() && swap_size.is_some() && thread_count.is_some() {
                break;
            }

        }

        match (vm_size, pm_size, swap_size, thread_count) {
            (Some(vm), Some(pm), Some(swap), Some(th_count)) 
                => Ok(ProcessStatusFileModel::new(vm, pm, swap, th_count)),
            _ => Err(ParseError::ParsingError(
                "VmSize or VmRSS not found in status".into(),
            )),
        }
    }

    fn read_entire_file(&self, file_path: &String) -> Result<String, ParseError> {
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let mut buf_reader = BufReader::new(file);
        let mut buf = String::new();
        let _ = buf_reader.read_to_string(&mut buf);

        Ok(buf)
    }

    fn parse_stat_info<R>(&self, mut buf_reader: R) -> Result<(u64, u64), ParseError> 
        where R: BufRead,
    {
        let mut content = String::new();
        let size_read = buf_reader
            .read_to_string(&mut content)
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;
        if size_read == 0 {
            return Err(ParseError::ParsingError("Stat file has 0 bytes".to_string()));
        }

        let content = content.trim();

        // /proc/<pid>/stat format starts with: "pid (comm) state ..."
        let comm_start = content
            .find('(')
            .ok_or_else(|| ParseError::ParsingError("Stat file has wrong format".to_string()))?;

        // comm is unpredictable since it is the command line and can contain ')' itself
        // that is why I use rfind to find its last appearance
        let comm_end = content
            .rfind(") ")
            .ok_or_else(|| ParseError::ParsingError("Stat file has wrong format".to_string()))?;

        if comm_end <= comm_start {
            return Err(ParseError::ParsingError("Stat file has wrong format".to_string()));
        }

        // skip state and ppid
        let after_comm = &content[(comm_end + 2)..];
        let fields: Vec<&str> = after_comm.split_whitespace().collect();

        // relative to field 3: field14(utime) => idx 11, field15(stime) => idx 12
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

    fn normalize_cmdline(&self, s: &String) -> String {
        s.split("\0")
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn normalize_name(&self, s: &String) -> String {
        s.trim_end_matches("\n").into()
    }

    fn get_base_thread_path(&self, pid: Pid) -> String {
        format!("{BASE_PROC_PATH}/{pid}/task")
    }
}

impl TraitProcessParser for ProcessParser {
    fn parse_process(&self, file_path: &String) -> Result<Process, ParseError> {
        let pid = extract_pid_from_path(file_path)?;
        let mut process = Process::new(pid);
        let name = self.get_process_name(pid)?;
        let cmdline = self.get_process_cmdline(pid)?;
        let status_file_model = self.get_status_info(pid)?;

        process.name = name;
        process.cmdline = cmdline;
        process.virtual_mem = status_file_model.virtual_mem;
        process.physical_mem = status_file_model.physical_mem;
        process.swap_mem = status_file_model.swap_mem;
        process.thread_count = status_file_model.thread_count;

        Ok (process)
    }

    fn get_threads_for_pid(&self, pid: Pid) -> Result<Vec<Thread>, ParseError> {
        let mut threads = vec![];
        let entries = fs::read_dir(self.get_base_thread_path(pid))
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let thread_path = entry.path();
            if thread_path.is_dir() {
                let thr_path_str = thread_path.display().to_string();
                let tid = extract_tid_from_path(&thr_path_str)?;
                threads.push(Thread::new(tid));
            }
        }

        Ok(threads)
    }

    fn get_status_info(&self, pid: Pid) -> Result<ProcessStatusFileModel, ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/status");
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        self.parse_status_info(buf_reader)
    }

    fn get_stat_info(&self, pid: Pid) -> Result<(u64, u64), ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/stat");
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        self.parse_stat_info(buf_reader)
    }

    fn get_process_name(&self, pid: Pid) -> Result<String, ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/comm");
        let buf = self.read_entire_file(&file_path)?;
        let normalized = self.normalize_name(&buf);

        Ok(normalized)
    }

    fn get_process_cmdline(&self, pid: Pid) -> Result<String, ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/cmdline");
        let buf = self.read_entire_file(&file_path)?;
        let normalized = self.normalize_cmdline(&buf);

        Ok(normalized)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::ProcessParser;

    #[test]
    fn parse_status_info_extracts_expected_fields() {
        let parser = ProcessParser::new();
        let input = "Name:\ttest\nVmSize:\t1000 kB\nVmRSS:\t512 kB\nVmSwap:\t8 kB\nThreads:\t4\n";

        let model = parser
            .parse_status_info(Cursor::new(input))
            .expect("status info should parse");

        assert_eq!(model.virtual_mem, 1000);
        assert_eq!(model.physical_mem, 512);
        assert_eq!(model.swap_mem, 8);
        assert_eq!(model.thread_count, 4);
    }

    #[test]
    fn parse_status_info_fails_when_required_field_missing() {
        let parser = ProcessParser::new();
        let input = "VmSize:\t1000 kB\nVmRSS:\t512 kB\nThreads:\t4\n";

        let result = parser.parse_status_info(Cursor::new(input));
        assert!(result.is_err());
    }

    #[test]
    fn parse_stat_info_reads_utime_and_stime() {
        let parser = ProcessParser::new();
        // fields after ") " begin at field 3 (state); utime/stime are relative indices 11 and 12.
        let input = "1234 (my process) R 1 2 3 4 5 6 7 8 9 10 11 42 84 15 16";

        let (utime, stime) = parser
            .parse_stat_info(Cursor::new(input))
            .expect("stat info should parse");

        assert_eq!(utime, 11);
        assert_eq!(stime, 42);
    }

    #[test]
    fn parse_stat_info_fails_on_empty_content() {
        let parser = ProcessParser::new();
        let result = parser.parse_stat_info(Cursor::new(""));
        assert!(result.is_err());
    }

    #[test]
    fn normalize_cmdline_replaces_nul_with_spaces() {
        let parser = ProcessParser::new();
        let input = String::from("python\0script.py\0--flag\0");
        let normalized = parser.normalize_cmdline(&input);

        assert_eq!(normalized, "python script.py --flag");
    }

    #[test]
    fn normalize_name_trims_trailing_newline() {
        let parser = ProcessParser::new();
        let input = String::from("bash\n");
        let normalized = parser.normalize_name(&input);

        assert_eq!(normalized, "bash");
    }
}
