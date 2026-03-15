#[derive(Debug, Clone)]
pub struct ThreadStatInfoModel {
    pub utime: u64,
    pub stime: u64,
    pub state: Option<char>,
    pub last_cpu: Option<u32>,
}
