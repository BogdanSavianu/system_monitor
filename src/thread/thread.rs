use crate::util::Tid;

#[derive(Debug, Clone)]
pub struct Thread {
    pub tid: Tid,
}

impl Thread {
    pub fn new(tid: Tid) -> Self {
        Thread {
            tid,
        }
    }
}
