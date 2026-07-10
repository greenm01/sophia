use crate::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendSessionLoopTickReport {
    pub input_gate: LiveInputReadinessGateReport,
    pub native_page_flip: LibdrmNativeReadAndPollReport,
    pub tick: LiveBackendRuntimeTickReport,
}
