#[derive(Debug, Clone)]
pub struct Thread {
    pub tid: Tid,
}

pub type Tid = u32;

impl Thread {
    pub fn new(tid: Tid) -> Self {
        Thread {
            tid,
        }
    }
}

//impl<'a> From<()> for Thread<'a> {
//    fn from(value: ()) -> Self {
//    }
//}
