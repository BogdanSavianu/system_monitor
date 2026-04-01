use system_monitor::{
    dto::ThreadCpuSampleDTO,
    util::{Pid, Tid},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ThreadRowViewModel {
    pub pid: Pid,
    pub tid: Tid,
    pub process_name: String,
    pub thread_name: String,
    pub state: Option<char>,
    pub cpu_top: f64,
}

pub fn thread_rows_from_dtos(samples: &[ThreadCpuSampleDTO]) -> Vec<ThreadRowViewModel> {
    samples
        .iter()
        .map(|sample| ThreadRowViewModel {
            pid: sample.pid,
            tid: sample.tid,
            process_name: sample.process_name.clone(),
            thread_name: sample.thread_name.clone(),
            state: sample.state,
            cpu_top: sample.cpu_top,
        })
        .collect()
}
