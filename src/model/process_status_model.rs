use crate::util::types::*;

pub struct ProcessStatusFileModel {
    pub virtual_mem: Vm,
    pub physical_mem: Pm,
    pub swap_mem: Swap,
    pub thread_count: u32,
    pub uid: u32,
    pub fd_size: u32,
}

impl ProcessStatusFileModel {
    pub fn new(vm: Vm, pm: Pm, sm: Swap, tc: u32, uid: u32, fd_size: u32) -> Self {
        ProcessStatusFileModel {
            virtual_mem: vm,
            physical_mem: pm,
            swap_mem: sm,
            thread_count: tc,
            uid,
            fd_size,
        }
    }
}
