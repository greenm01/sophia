#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveBackendSessionLoopReadiness {
    pub input_ready: bool,
    pub page_flip_ready: bool,
}

impl LiveBackendSessionLoopReadiness {
    pub const fn new(input_ready: bool, page_flip_ready: bool) -> Self {
        Self {
            input_ready,
            page_flip_ready,
        }
    }

    pub const fn idle() -> Self {
        Self::new(false, false)
    }

    pub const fn input_ready() -> Self {
        Self::new(true, false)
    }

    pub const fn page_flip_ready() -> Self {
        Self::new(false, true)
    }

    pub const fn all_ready() -> Self {
        Self::new(true, true)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveBackendReadinessCollector {
    input_ready: bool,
    page_flip_ready: bool,
}

impl LiveBackendReadinessCollector {
    pub const fn new() -> Self {
        Self {
            input_ready: false,
            page_flip_ready: false,
        }
    }

    pub fn observe_input_ready(&mut self) {
        self.input_ready = true;
    }

    pub fn observe_page_flip_ready(&mut self) {
        self.page_flip_ready = true;
    }

    pub const fn snapshot(&self) -> LiveBackendSessionLoopReadiness {
        LiveBackendSessionLoopReadiness::new(self.input_ready, self.page_flip_ready)
    }

    pub fn drain(&mut self) -> LiveBackendSessionLoopReadiness {
        let readiness = self.snapshot();
        self.input_ready = false;
        self.page_flip_ready = false;
        readiness
    }
}
