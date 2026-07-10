#[cfg(feature = "libdrm-events")]
use super::*;
#[cfg(feature = "libdrm-events")]
use crate::prelude::*;
#[cfg(feature = "libdrm-events")]
use std::{collections::VecDeque, sync::mpsc::SyncSender};

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
