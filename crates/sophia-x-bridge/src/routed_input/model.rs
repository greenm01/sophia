use crate::prelude::*;

use super::adapter::routed_input_decision_allows_delivery;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputEdgeKind {
    ActiveGrab,
    FocusPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInputEdgeSmokeReport {
    pub edge: RoutedInputEdgeKind,
    pub decision: XLibreRoutedInputDecision,
    pub delivery_allowed: bool,
}

pub fn smoke_routed_input_edge(
    edge: RoutedInputEdgeKind,
    serial: u64,
    target_window: XWindowId,
) -> RoutedInputEdgeSmokeReport {
    let outcome = match edge {
        RoutedInputEdgeKind::ActiveGrab => XLibreRoutedInputOutcome::RejectedActiveGrab,
        RoutedInputEdgeKind::FocusPolicy => XLibreRoutedInputOutcome::RejectedFocusPolicy,
    };
    let decision = XLibreRoutedInputDecision {
        serial,
        target_window,
        outcome,
    };

    RoutedInputEdgeSmokeReport {
        edge,
        delivery_allowed: routed_input_decision_allows_delivery(&decision),
        decision,
    }
}

pub fn smoke_routed_input_edges(target_window: XWindowId) -> [RoutedInputEdgeSmokeReport; 2] {
    [
        smoke_routed_input_edge(RoutedInputEdgeKind::ActiveGrab, 1, target_window),
        smoke_routed_input_edge(RoutedInputEdgeKind::FocusPolicy, 2, target_window),
    ]
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInputSmokeReport {
    pub display_name: Option<String>,
    pub extension_opcode: u8,
    pub target_window: XWindowId,
    pub device: DeviceId,
    pub decision: XLibreRoutedInputDecision,
    pub dispatch_elapsed: Duration,
    pub request_bytes: usize,
    pub event_x: i16,
    pub event_y: i16,
    pub button: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInputStressReport {
    pub display_name: Option<String>,
    pub extension_opcode: u8,
    pub target_window: XWindowId,
    pub device: DeviceId,
    pub iterations: usize,
    pub accepted: usize,
    pub request_bytes: usize,
    pub threshold: Duration,
    pub stats: RoutedInputDispatchStats,
    pub recommendation: RoutedInputOptimizationRecommendation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputOptimizationRecommendation {
    KeepX11RequestPath,
    ConsiderSharedMemoryRing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputTransport {
    X11Request,
    SharedMemoryRing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedMemoryRouteRingState {
    Unavailable,
    Available,
    Failed,
}

pub fn select_routed_input_transport(
    recommendation: RoutedInputOptimizationRecommendation,
    shm_state: SharedMemoryRouteRingState,
) -> RoutedInputTransport {
    match (recommendation, shm_state) {
        (
            RoutedInputOptimizationRecommendation::ConsiderSharedMemoryRing,
            SharedMemoryRouteRingState::Available,
        ) => RoutedInputTransport::SharedMemoryRing,
        _ => RoutedInputTransport::X11Request,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RoutedInputDispatchStats {
    samples: Vec<Duration>,
}

impl RoutedInputDispatchStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_samples(samples: impl IntoIterator<Item = Duration>) -> Self {
        Self {
            samples: samples.into_iter().collect(),
        }
    }

    pub fn record(&mut self, elapsed: Duration) {
        self.samples.push(elapsed);
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn min(&self) -> Option<Duration> {
        self.samples.iter().copied().min()
    }

    pub fn max(&self) -> Option<Duration> {
        self.samples.iter().copied().max()
    }

    pub fn average(&self) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }

        let total_nanos: u128 = self.samples.iter().map(|sample| sample.as_nanos()).sum();
        let average_nanos = total_nanos / self.samples.len() as u128;
        let average_nanos = average_nanos.min(u128::from(u64::MAX)) as u64;

        Some(Duration::from_nanos(average_nanos))
    }

    pub fn percentile_nearest(&self, percentile: u8) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }

        let percentile = percentile.min(100);
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let last = sorted.len() - 1;
        let index = (last * usize::from(percentile) + 50) / 100;

        sorted.get(index).copied()
    }

    pub fn recommendation(
        &self,
        max_dispatch_threshold: Duration,
    ) -> RoutedInputOptimizationRecommendation {
        match self.max() {
            Some(max) if max > max_dispatch_threshold => {
                RoutedInputOptimizationRecommendation::ConsiderSharedMemoryRing
            }
            _ => RoutedInputOptimizationRecommendation::KeepX11RequestPath,
        }
    }
}
