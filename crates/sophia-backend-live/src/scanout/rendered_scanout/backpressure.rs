#[cfg(feature = "libdrm-events")]
pub const LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS: u64 = 2;

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRenderedPrimaryPlaneScanoutBackpressureReport {
    pub status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus,
    pub in_flight: bool,
    pub in_flight_ticks: u64,
    pub threshold_ticks: u64,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRenderedPrimaryPlaneScanoutBackpressureStatus {
    Idle,
    WaitingForPageFlip,
    StalledWaitingForPageFlip,
}

#[cfg(feature = "libdrm-events")]
impl LiveRenderedPrimaryPlaneScanoutBackpressureReport {
    pub const fn from_in_flight_state(
        in_flight: bool,
        in_flight_ticks: u64,
        threshold_ticks: u64,
    ) -> Self {
        let status = if !in_flight {
            LiveRenderedPrimaryPlaneScanoutBackpressureStatus::Idle
        } else if threshold_ticks > 0 && in_flight_ticks >= threshold_ticks {
            LiveRenderedPrimaryPlaneScanoutBackpressureStatus::StalledWaitingForPageFlip
        } else {
            LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip
        };

        Self {
            status,
            in_flight,
            in_flight_ticks,
            threshold_ticks,
        }
    }
}
