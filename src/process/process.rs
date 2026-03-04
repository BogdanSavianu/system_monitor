use std::fmt::Display;

pub use crate::thread::Thread;


#[derive(Debug, Clone)]
pub struct Process {
    pub pid: u32,
    //pub threads_list: Vec<Thread>
}

pub type Pid = u32;

impl Process {
    pub fn new(pid: Pid) -> Self {
        Process { 
            pid,
            //threads_list: vec![]
        }
    }
}

impl Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, 
"Process id: {:?}", 
    self.pid, /*self.threads_list*/)
    }
}
