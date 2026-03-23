use std::collections::HashMap;

use crate::util::Pid;

#[derive(Debug, Clone)]
pub struct ProcessHierarchyIndexDTO {
    pub pid_to_ppid: HashMap<Pid, Pid>,
    pub children_by_pid: HashMap<Pid, Vec<Pid>>,
    pub roots: Vec<Pid>,
}

impl ProcessHierarchyIndexDTO {
    pub fn with_values(
        pid_to_ppid: HashMap<Pid, Pid>,
        children_by_pid: HashMap<Pid, Vec<Pid>>,
        roots: Vec<Pid>,
    ) -> Self {
        ProcessHierarchyIndexDTO {
            pid_to_ppid,
            children_by_pid,
            roots,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessHierarchyNodeDTO {
    pub pid: Pid,
    pub ppid: Pid,
    pub name: String,
    pub children: Vec<ProcessHierarchyNodeDTO>,
}

impl ProcessHierarchyNodeDTO {
    pub fn with_values(
        pid: Pid,
        ppid: Pid,
        name: String,
        children: Vec<ProcessHierarchyNodeDTO>,
    ) -> Self {
        ProcessHierarchyNodeDTO {
            pid,
            ppid,
            name,
            children,
        }
    }
}
