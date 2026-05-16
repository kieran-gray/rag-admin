use std::fmt;

#[derive(Clone, Copy)]
pub struct NodeStartEnd {
    pub start: u32,
    pub end: u32,
}

impl fmt::Display for NodeStartEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end)
    }
}
