#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveBackendSessionLoopPageFlipBudget {
    pub max_read: usize,
    pub max_emit: usize,
}

impl LiveBackendSessionLoopPageFlipBudget {
    pub const fn new(max_read: usize, max_emit: usize) -> Self {
        Self { max_read, max_emit }
    }
}
