use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}
