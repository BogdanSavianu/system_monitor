use std::fmt::Display;

pub use crate::thread::Thread;
use crate::util::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Sleeping,
    DiskSleep,
    Zombie,
    Stopped,
    TracingStop,
    Dead,
    Idle,
    Unknown,
}

impl ProcessState {
    pub fn from_char(c: char) -> Self {
        match c {
            'R' => Self::Running,
            'S' => Self::Sleeping,
            'D' => Self::DiskSleep,
            'Z' => Self::Zombie,
            'T' => Self::Stopped,
            't' => Self::TracingStop,
            'X' | 'x' => Self::Dead,
            'I' => Self::Idle,
            _ => Self::Unknown,
        }
    }

    pub fn as_char(self) -> char {
        match self {
            Self::Running => 'R',
            Self::Sleeping => 'S',
            Self::DiskSleep => 'D',
            Self::Zombie => 'Z',
            Self::Stopped => 'T',
            Self::TracingStop => 't',
            Self::Dead => 'X',
            Self::Idle => 'I',
            Self::Unknown => '?',
        }
    }
}

impl Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Running => "R ",
            Self::Sleeping => "S",
            Self::DiskSleep => "D (disk sleep)",
            Self::Zombie => "Z",
            Self::Stopped => "T (stopped)",
            Self::TracingStop => "t (tracing stop)",
            Self::Dead => "X (dead)",
            Self::Idle => "I (idle)",
            Self::Unknown => "?",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone)]
pub struct Process {
    pub pid: Pid,
    pub ppid: Pid,
    pub name: String,
    pub cmdline: String,
    // Pm, Vm and Swap are both in KB
    pub physical_mem: Pm,
    pub virtual_mem: Vm,
    pub swap_mem: Swap,
    pub thread_count: u32,
    pub state: ProcessState,
    pub uid: u32,
    pub fd_size: u32,
}

impl Process {
    pub fn new(pid: Pid) -> Self {
        Process {
            pid,
            ppid: 0,
            name: "".into(),
            cmdline: "".into(),
            physical_mem: 0,
            virtual_mem: 0,
            swap_mem: 0,
            thread_count: 0,
            state: ProcessState::Sleeping,
            uid: 0,
            fd_size: 0,
        }
    }
}

impl Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Process id: {:?},
            Parent id: {:?},
    Name: {:?},
    CmdLine: {:?},
    Thread Count: {:?},
    Virtual Memory: {:?},
    Physical Memory: {:?},
    Swap Memory: {:?}",
            self.pid,
            self.ppid,
            self.name,
            self.cmdline,
            self.thread_count,
            self.virtual_mem,
            self.physical_mem,
            self.swap_mem
        )
    }
}
