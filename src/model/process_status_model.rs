use crate::util::types::*;

pub struct ProcessStatusFileModel {
    pub virtual_mem: Vm,
    pub physical_mem: Pm,
    pub swap_mem: Swap,
    pub thread_count: u32,
}

impl ProcessStatusFileModel {
    pub fn new(vm: Vm, pm: Pm, sm: Swap, tc: u32) -> Self {
        ProcessStatusFileModel {
            virtual_mem: vm,
            physical_mem: pm,
            swap_mem: sm,
            thread_count: tc,
        }
    }
}
