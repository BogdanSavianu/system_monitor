use std::fmt::Display;

pub use crate::thread::Thread;
use crate::util::types::*;


#[derive(Debug, Clone)]
pub struct Process {
    pub pid: Pid,
    // Pm and Vm are both in KB
    pub physical_mem: Pm,
    pub virtual_mem: Vm,
}

impl Process {
    pub fn new(pid: Pid) -> Self {
        Process { 
            pid,
            physical_mem: 0,
            virtual_mem: 0,
        }
    }
}

impl Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, 
"Process id: {:?},
    Virtual Memory: {:?},
    Physical Memory: {:?}",
    self.pid, self.virtual_mem, self.physical_mem)
    }
}
