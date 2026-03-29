use system_monitor::{dto::ProcessHierarchyNodeDTO, util::Pid};

#[derive(Debug, Clone)]
pub struct ProcessHierarchyNodeViewModel {
    pub pid: Pid,
    pub ppid: Pid,
    pub name: String,
    pub children: Vec<ProcessHierarchyNodeViewModel>,
}

impl ProcessHierarchyNodeViewModel {
    pub fn from_dto(dto: &ProcessHierarchyNodeDTO) -> Self {
        Self {
            pid: dto.pid,
            ppid: dto.ppid,
            name: dto.name.clone(),
            children: dto.children.iter().map(Self::from_dto).collect(),
        }
    }
}

pub fn hierarchy_from_dtos(
    roots: &[ProcessHierarchyNodeDTO],
) -> Vec<ProcessHierarchyNodeViewModel> {
    roots
        .iter()
        .map(ProcessHierarchyNodeViewModel::from_dto)
        .collect()
}
