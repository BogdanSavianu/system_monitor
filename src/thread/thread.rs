use crate::util::Tid;

#[derive(Debug, Clone)]
pub struct Thread {
    pub tid: Tid,
    pub name: String,
    pub state: Option<char>,
    pub last_cpu: Option<u32>,
    pub voluntary_ctxt_switches: Option<u64>,
    pub nonvoluntary_ctxt_switches: Option<u64>,
    pub io_read_bytes: Option<u64>,
    pub io_write_bytes: Option<u64>,
    pub io_rchar: Option<u64>,
    pub io_wchar: Option<u64>,
    pub io_syscr: Option<u64>,
    pub io_syscw: Option<u64>,
}

impl Thread {
    pub fn new(tid: Tid) -> Self {
        Thread {
            tid,
            name: String::new(),
            state: None,
            last_cpu: None,
            voluntary_ctxt_switches: None,
            nonvoluntary_ctxt_switches: None,
            io_read_bytes: None,
            io_write_bytes: None,
            io_rchar: None,
            io_wchar: None,
            io_syscr: None,
            io_syscw: None,
        }
    }
}
