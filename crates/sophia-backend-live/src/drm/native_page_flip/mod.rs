use crate::prelude::*;
use std::sync::mpsc::{SyncSender, TrySendError};

mod authority;

pub use authority::*;

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmPageFlipEventPollReport {
    pub status: LibdrmPageFlipEventPollStatus,
    pub callbacks: LivePageFlipCallbackSourceReport,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmPageFlipEventPollStatus {
    Idle,
    Emitted,
    Backpressure,
    Disconnected,
    EmitLimitReached,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmPageFlipEventPollReport {
    pub fn from_source_report(callbacks: LivePageFlipCallbackSourceReport) -> Self {
        let status = if callbacks.disconnected {
            LibdrmPageFlipEventPollStatus::Disconnected
        } else if callbacks.backpressure {
            LibdrmPageFlipEventPollStatus::Backpressure
        } else if callbacks.max_reached {
            LibdrmPageFlipEventPollStatus::EmitLimitReached
        } else if callbacks.emitted > 0 {
            LibdrmPageFlipEventPollStatus::Emitted
        } else {
            LibdrmPageFlipEventPollStatus::Idle
        };

        Self { status, callbacks }
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LibdrmPageFlipEventPoller {
    fn poll_page_flip_events(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LibdrmPageFlipEventPollReport;
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakeLibdrmPageFlipEventPoller {
    source: FakePageFlipCallbackSource,
}

#[cfg(feature = "libdrm-events")]
impl FakeLibdrmPageFlipEventPoller {
    pub fn new(callbacks: impl IntoIterator<Item = LivePageFlipCallback>) -> Self {
        Self {
            source: FakePageFlipCallbackSource::new(callbacks),
        }
    }

    pub fn queued_len(&self) -> usize {
        self.source.queued_len()
    }
}

#[cfg(feature = "libdrm-events")]
impl LibdrmPageFlipEventPoller for FakeLibdrmPageFlipEventPoller {
    fn poll_page_flip_events(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LibdrmPageFlipEventPollReport {
        LibdrmPageFlipEventPollReport::from_source_report(self.source.emit_ready(sender, max_emit))
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeOutputSlot {
    raw: u16,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeOutputSlot {
    pub const fn new(raw: u16) -> Option<Self> {
        if raw == 0 {
            return None;
        }

        Some(Self { raw })
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeOutputRoute {
    pub slot: LibdrmNativeOutputSlot,
    pub output: OutputId,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeCrtcRoute {
    crtc: drm::control::crtc::Handle,
    slot: LibdrmNativeOutputSlot,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeCrtcRoute {
    pub const fn new(crtc: drm::control::crtc::Handle, slot: LibdrmNativeOutputSlot) -> Self {
        Self { crtc, slot }
    }

    const fn slot(self) -> LibdrmNativeOutputSlot {
        self.slot
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipCallback {
    pub output_slot: LibdrmNativeOutputSlot,
    pub frame_serial: u64,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePageFlipCallback {
    pub const fn new(output_slot: LibdrmNativeOutputSlot, frame_serial: u64) -> Self {
        Self {
            output_slot,
            frame_serial,
        }
    }

    pub fn decode(self, routes: &[LibdrmNativeOutputRoute]) -> LibdrmNativePageFlipDecodeReport {
        if self.frame_serial == 0 {
            return LibdrmNativePageFlipDecodeReport {
                status: LibdrmNativePageFlipDecodeStatus::InvalidFrameSerial,
                callback: None,
            };
        }

        let Some(route) = routes
            .iter()
            .find(|route| route.slot == self.output_slot)
            .copied()
        else {
            return LibdrmNativePageFlipDecodeReport {
                status: LibdrmNativePageFlipDecodeStatus::UnknownOutputSlot,
                callback: None,
            };
        };

        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::Decoded,
            callback: Some(LivePageFlipCallback {
                output: route.output,
                frame_serial: self.frame_serial,
            }),
        }
    }
}

#[cfg(feature = "libdrm-events")]
pub fn reduce_native_page_flip_event(
    event: &drm::control::PageFlipEvent,
    routes: &[LibdrmNativeCrtcRoute],
) -> Option<LibdrmNativePageFlipCallback> {
    let route = routes.iter().find(|route| route.crtc == event.crtc)?;
    Some(LibdrmNativePageFlipCallback::new(
        route.slot(),
        u64::from(event.frame),
    ))
}

#[cfg(feature = "libdrm-events")]
pub fn decode_native_page_flip_batch(
    callbacks: &[LibdrmNativePageFlipCallback],
    routes: &[LibdrmNativeOutputRoute],
    sender: &SyncSender<LivePageFlipCallback>,
    max_decode: usize,
) -> LibdrmNativePageFlipBatchReport {
    let mut source_report = LivePageFlipCallbackSourceReport::default();
    let mut decoded_callbacks = 0usize;
    let mut rejected_callbacks = 0usize;
    let mut stopped_at = None;

    for (index, native) in callbacks.iter().take(max_decode).copied().enumerate() {
        let decode = native.decode(routes);
        let Some(callback) = decode.callback else {
            rejected_callbacks = rejected_callbacks.saturating_add(1);
            continue;
        };
        decoded_callbacks = decoded_callbacks.saturating_add(1);

        match sender.try_send(callback) {
            Ok(()) => {
                source_report.emitted = source_report.emitted.saturating_add(1);
            }
            Err(TrySendError::Full(_)) => {
                source_report.backpressure = true;
                stopped_at = Some(index);
                break;
            }
            Err(TrySendError::Disconnected(_)) => {
                source_report.disconnected = true;
                stopped_at = Some(index);
                break;
            }
        }
    }

    if let Some(index) = stopped_at {
        source_report.queued_remaining = callbacks.len().saturating_sub(index);
    }

    if callbacks.len() > max_decode {
        source_report.max_reached = true;
        source_report.queued_remaining = source_report
            .queued_remaining
            .max(callbacks.len() - max_decode);
    }

    LibdrmNativePageFlipBatchReport {
        read_loop: LibdrmNativeReadLoopReport::callbacks_decoded(
            decoded_callbacks,
            rejected_callbacks,
        )
        .unwrap_or_else(LibdrmNativeReadLoopReport::idle),
        poll: LibdrmPageFlipEventPollReport::from_source_report(source_report),
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipBatchReport {
    pub read_loop: LibdrmNativeReadLoopReport,
    pub poll: LibdrmPageFlipEventPollReport,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipDecodeReport {
    pub status: LibdrmNativePageFlipDecodeStatus,
    pub callback: Option<LivePageFlipCallback>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePageFlipDecodeStatus {
    Decoded,
    UnknownOutputSlot,
    InvalidFrameSerial,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeLibdrmPageFlipEventPoller {
    source: LibdrmNativePageFlipSource,
    routes: Vec<LibdrmNativeOutputRoute>,
    pending_callbacks: VecDeque<LibdrmNativePageFlipCallback>,
    last_read_loop: LibdrmNativeReadLoopReport,
}

#[cfg(feature = "libdrm-events")]
impl NativeLibdrmPageFlipEventPoller {
    pub fn new(source: LibdrmNativePageFlipSource) -> Self {
        Self {
            source,
            routes: Vec::new(),
            pending_callbacks: VecDeque::new(),
            last_read_loop: LibdrmNativeReadLoopReport::idle(),
        }
    }

    pub fn with_routes(
        mut self,
        routes: impl IntoIterator<Item = LibdrmNativeOutputRoute>,
    ) -> Self {
        self.replace_routes(routes);
        self
    }

    pub fn replace_routes(&mut self, routes: impl IntoIterator<Item = LibdrmNativeOutputRoute>) {
        self.routes.clear();
        self.routes.extend(routes);
    }

    pub fn inject_callbacks(
        &mut self,
        callbacks: impl IntoIterator<Item = LibdrmNativePageFlipCallback>,
    ) {
        self.pending_callbacks.extend(callbacks);
    }

    pub fn read_page_flip_events<R>(
        &mut self,
        reader: &mut R,
        max_read: usize,
    ) -> LibdrmNativeReadLoopReport
    where
        R: LibdrmNativePageFlipReader,
    {
        let result = reader.read_ready_page_flip_callbacks(max_read);
        self.last_read_loop = result.report;
        if result.report.status != LibdrmNativeReadLoopStatus::ReadFailed {
            self.pending_callbacks.extend(result.callbacks);
        }
        result.report
    }

    pub fn read_and_poll_page_flip_events<R>(
        &mut self,
        reader: &mut R,
        sender: &SyncSender<LivePageFlipCallback>,
        max_read: usize,
        max_emit: usize,
    ) -> LibdrmNativeReadAndPollReport
    where
        R: LibdrmNativePageFlipReader,
    {
        if !self.pending_callbacks.is_empty() {
            let poll = self.poll_page_flip_events(sender, max_emit);
            return LibdrmNativeReadAndPollReport {
                read_loop: self.last_read_loop,
                poll,
            };
        }

        let read_loop = self.read_page_flip_events(reader, max_read);
        if read_loop.status == LibdrmNativeReadLoopStatus::ReadFailed {
            return LibdrmNativeReadAndPollReport {
                read_loop,
                poll: read_loop.into_poll_report(),
            };
        }

        if self.pending_callbacks.is_empty() {
            return LibdrmNativeReadAndPollReport {
                read_loop,
                poll: read_loop.into_poll_report(),
            };
        }

        LibdrmNativeReadAndPollReport {
            read_loop,
            poll: self.poll_page_flip_events(sender, max_emit),
        }
    }

    pub const fn source_report(&self) -> LibdrmNativePageFlipSourceReport {
        self.source.report()
    }

    pub const fn last_read_loop_report(&self) -> LibdrmNativeReadLoopReport {
        self.last_read_loop
    }

    pub fn pending_callback_count(&self) -> usize {
        self.pending_callbacks.len()
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn diagnostics(&self) -> LibdrmNativePollerDiagnostics {
        LibdrmNativePollerDiagnostics {
            route_count: self.routes.len(),
            pending_callbacks: self.pending_callbacks.len(),
            last_read_loop: self.last_read_loop,
        }
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativePageFlipReader {
    fn read_ready_page_flip_callbacks(&mut self, max_read: usize)
    -> LibdrmNativePageFlipReadResult;
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipReadResult {
    pub report: LibdrmNativeReadLoopReport,
    pub callbacks: Vec<LibdrmNativePageFlipCallback>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakeLibdrmNativePageFlipReader {
    queued: VecDeque<LibdrmNativePageFlipCallback>,
    fail_next_read: bool,
}

#[cfg(feature = "libdrm-events")]
impl FakeLibdrmNativePageFlipReader {
    pub fn new(callbacks: impl IntoIterator<Item = LibdrmNativePageFlipCallback>) -> Self {
        Self {
            queued: callbacks.into_iter().collect(),
            fail_next_read: false,
        }
    }

    pub fn fail_next_read(&mut self) {
        self.fail_next_read = true;
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePageFlipReader for FakeLibdrmNativePageFlipReader {
    fn read_ready_page_flip_callbacks(
        &mut self,
        max_read: usize,
    ) -> LibdrmNativePageFlipReadResult {
        if self.fail_next_read {
            self.fail_next_read = false;
            return LibdrmNativePageFlipReadResult {
                report: LibdrmNativeReadLoopReport::read_failed(),
                callbacks: Vec::new(),
            };
        }

        let mut callbacks = Vec::new();
        for _ in 0..max_read {
            let Some(callback) = self.queued.pop_front() else {
                break;
            };
            callbacks.push(callback);
        }

        LibdrmNativePageFlipReadResult {
            report: LibdrmNativeReadLoopReport::callbacks_decoded(callbacks.len(), 0)
                .unwrap_or_else(LibdrmNativeReadLoopReport::would_block),
            callbacks,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct NativeLibdrmPageFlipEventReader<D> {
    device: D,
    crtc_routes: Vec<LibdrmNativeCrtcRoute>,
}

#[cfg(feature = "libdrm-events")]
impl<D> NativeLibdrmPageFlipEventReader<D> {
    pub fn new(device: D) -> Self {
        Self {
            device,
            crtc_routes: Vec::new(),
        }
    }

    pub fn with_crtc_routes(
        mut self,
        routes: impl IntoIterator<Item = LibdrmNativeCrtcRoute>,
    ) -> Self {
        self.replace_crtc_routes(routes);
        self
    }

    pub fn replace_crtc_routes(&mut self, routes: impl IntoIterator<Item = LibdrmNativeCrtcRoute>) {
        self.crtc_routes.clear();
        self.crtc_routes.extend(routes);
    }

    pub fn crtc_route_count(&self) -> usize {
        self.crtc_routes.len()
    }
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativePageFlipReader for NativeLibdrmPageFlipEventReader<D>
where
    D: drm::control::Device,
{
    fn read_ready_page_flip_callbacks(
        &mut self,
        max_read: usize,
    ) -> LibdrmNativePageFlipReadResult {
        if max_read == 0 {
            return LibdrmNativePageFlipReadResult {
                report: LibdrmNativeReadLoopReport::would_block(),
                callbacks: Vec::new(),
            };
        }

        let events = match self.device.receive_events() {
            Ok(events) => events,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return LibdrmNativePageFlipReadResult {
                    report: LibdrmNativeReadLoopReport::would_block(),
                    callbacks: Vec::new(),
                };
            }
            Err(_) => {
                return LibdrmNativePageFlipReadResult {
                    report: LibdrmNativeReadLoopReport::read_failed(),
                    callbacks: Vec::new(),
                };
            }
        };

        let mut callbacks = Vec::new();
        let mut rejected_callbacks = 0usize;

        for event in events.take(max_read) {
            let drm::control::Event::PageFlip(page_flip) = event else {
                continue;
            };

            match reduce_native_page_flip_event(&page_flip, &self.crtc_routes) {
                Some(callback) => callbacks.push(callback),
                None => rejected_callbacks = rejected_callbacks.saturating_add(1),
            }
        }

        LibdrmNativePageFlipReadResult {
            report: LibdrmNativeReadLoopReport::callbacks_decoded(
                callbacks.len(),
                rejected_callbacks,
            )
            .unwrap_or_else(LibdrmNativeReadLoopReport::would_block),
            callbacks,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeReadAndPollReport {
    pub read_loop: LibdrmNativeReadLoopReport,
    pub poll: LibdrmPageFlipEventPollReport,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePollerDiagnostics {
    pub route_count: usize,
    pub pending_callbacks: usize,
    pub last_read_loop: LibdrmNativeReadLoopReport,
}

#[cfg(feature = "libdrm-events")]
impl From<LibdrmNativePollerDiagnostics> for LiveLibdrmPollerDiagnostics {
    fn from(diagnostics: LibdrmNativePollerDiagnostics) -> Self {
        Self {
            status: diagnostics.last_read_loop.status.into(),
            route_count: diagnostics.route_count,
            pending_callbacks: diagnostics.pending_callbacks,
            decoded_callbacks: diagnostics.last_read_loop.decoded_callbacks,
            rejected_callbacks: diagnostics.last_read_loop.rejected_callbacks,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeReadLoopReport {
    pub status: LibdrmNativeReadLoopStatus,
    pub decoded_callbacks: usize,
    pub rejected_callbacks: usize,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeReadLoopReport {
    pub const fn idle() -> Self {
        Self {
            status: LibdrmNativeReadLoopStatus::Idle,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }

    pub const fn would_block() -> Self {
        Self {
            status: LibdrmNativeReadLoopStatus::WouldBlock,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }

    pub const fn callbacks_decoded(
        decoded_callbacks: usize,
        rejected_callbacks: usize,
    ) -> Option<Self> {
        if decoded_callbacks == 0 && rejected_callbacks == 0 {
            return None;
        }

        Some(Self {
            status: if decoded_callbacks > 0 {
                LibdrmNativeReadLoopStatus::CallbackDecoded
            } else {
                LibdrmNativeReadLoopStatus::CallbackRejected
            },
            decoded_callbacks,
            rejected_callbacks,
        })
    }

    pub const fn callback_decoded(decoded_callbacks: usize) -> Option<Self> {
        Self::callbacks_decoded(decoded_callbacks, 0)
    }

    pub const fn read_failed() -> Self {
        Self {
            status: LibdrmNativeReadLoopStatus::ReadFailed,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }

    pub fn into_poll_report(self) -> LibdrmPageFlipEventPollReport {
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: if matches!(self.status, LibdrmNativeReadLoopStatus::CallbackDecoded) {
                self.decoded_callbacks
            } else {
                0
            },
            queued_remaining: 0,
            backpressure: false,
            disconnected: matches!(self.status, LibdrmNativeReadLoopStatus::ReadFailed),
            max_reached: false,
        })
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeReadLoopStatus {
    Idle,
    WouldBlock,
    CallbackDecoded,
    CallbackRejected,
    ReadFailed,
}

#[cfg(feature = "libdrm-events")]
impl From<LibdrmNativeReadLoopStatus> for LiveLibdrmPollerDiagnosticsStatus {
    fn from(status: LibdrmNativeReadLoopStatus) -> Self {
        match status {
            LibdrmNativeReadLoopStatus::Idle => Self::Idle,
            LibdrmNativeReadLoopStatus::WouldBlock => Self::WouldBlock,
            LibdrmNativeReadLoopStatus::CallbackDecoded => Self::CallbackDecoded,
            LibdrmNativeReadLoopStatus::CallbackRejected => Self::CallbackRejected,
            LibdrmNativeReadLoopStatus::ReadFailed => Self::ReadFailed,
        }
    }
}

#[cfg(feature = "libdrm-events")]
impl LibdrmPageFlipEventPoller for NativeLibdrmPageFlipEventPoller {
    fn poll_page_flip_events(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LibdrmPageFlipEventPollReport {
        let _ = self.source.report();
        if self.pending_callbacks.is_empty() {
            self.last_read_loop = LibdrmNativeReadLoopReport::idle();
            return self.last_read_loop.into_poll_report();
        }

        let pending = self.pending_callbacks.iter().copied().collect::<Vec<_>>();
        let report = decode_native_page_flip_batch(&pending, &self.routes, sender, max_emit);
        let processed_callbacks = pending
            .len()
            .saturating_sub(report.poll.callbacks.queued_remaining);

        for _ in 0..processed_callbacks {
            let _ = self.pending_callbacks.pop_front();
        }

        self.last_read_loop = report.read_loop;
        report.poll
    }
}
