use std::collections::HashMap;

use crate::{hashmap, process::Process, util::Pid};

#[derive(Debug, Clone)]
pub struct ProcessHierarchyModel {
    pub pid_to_ppid: HashMap<Pid, Pid>,
    pub children_by_pid: HashMap<Pid, Vec<Pid>>,
    pub roots: Vec<Pid>,
}

impl ProcessHierarchyModel {
    pub fn new() -> Self {
        ProcessHierarchyModel {
            pid_to_ppid: hashmap![],
            children_by_pid: hashmap![],
            roots: vec![],
        }
    }

    pub fn build(processes: &HashMap<Pid, Process>) -> Self {
        let mut pid_to_ppid: HashMap<Pid, Pid> = hashmap![];
        let mut children_by_pid: HashMap<Pid, Vec<Pid>> = hashmap![];

        for process in processes.values() {
            pid_to_ppid.insert(process.pid, process.ppid);
            children_by_pid
                .entry(process.ppid)
                .or_insert_with(Vec::new)
                .push(process.pid);
        }

        let mut roots: Vec<Pid> = Vec::new();
        for (pid, ppid) in pid_to_ppid.iter() {
            if *ppid == 0 || !pid_to_ppid.contains_key(ppid) {
                roots.push(*pid);
            }
        }

        for children in children_by_pid.values_mut() {
            children.sort();
        }
        roots.sort_unstable();

        ProcessHierarchyModel {
            pid_to_ppid,
            children_by_pid,
            roots,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{model::ProcessHierarchyModel, process::Process};

    #[test]
    fn build_indexes_children_and_roots() {
        let mut processes: HashMap<u32, Process> = HashMap::new();

        let mut root = Process::new(100);
        root.ppid = 0;
        processes.insert(100, root);

        let mut child_a = Process::new(200);
        child_a.ppid = 100;
        processes.insert(200, child_a);

        let mut child_b = Process::new(300);
        child_b.ppid = 100;
        processes.insert(300, child_b);

        let mut orphan = Process::new(400);
        orphan.ppid = 9999;
        processes.insert(400, orphan);

        let hierarchy = ProcessHierarchyModel::build(&processes);

        assert_eq!(hierarchy.pid_to_ppid.get(&200), Some(&100));
        assert_eq!(hierarchy.children_by_pid.get(&100), Some(&vec![200, 300]));
        assert_eq!(hierarchy.roots, vec![100, 400]);
    }
}
