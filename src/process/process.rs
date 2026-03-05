use std::fmt::Display;

pub use crate::thread::Thread;
use crate::util::types::*;


#[derive(Debug, Clone)]
pub struct Process {
    pub pid: Pid,
    pub name: String,
    pub cmdline: String,
    // Pm, Vm and Swap are both in KB
    pub physical_mem: Pm,
    pub virtual_mem: Vm,
    pub swap_mem: Swap,
    pub thread_count: u32,
}

impl Process {
    pub fn new(pid: Pid) -> Self {
        Process { 
            pid,
            name: "".into(),
            cmdline: "".into(),
            physical_mem: 0,
            virtual_mem: 0,
            swap_mem: 0,
            thread_count: 0,
        }
    }
}

impl Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, 
"Process id: {:?},
    Name: {:?},
    CmdLine: {:?},
    Thread Count: {:?},
    Virtual Memory: {:?},
    Physical Memory: {:?},
    Swap Memory: {:?}",
    self.pid, self.name, self.cmdline, self.thread_count, self.virtual_mem, self.physical_mem, self.swap_mem)
    }
}
