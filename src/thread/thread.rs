#[derive(Debug)]
pub struct Thread {
    pub tid: u32,
}

impl Thread {
    pub fn new(tid: u32) -> Self {
        Thread {
            tid
        }
    }
}
