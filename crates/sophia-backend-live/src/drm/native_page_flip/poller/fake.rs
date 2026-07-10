use super::LibdrmPageFlipEventPoller;
use crate::prelude::*;
use std::sync::mpsc::SyncSender;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakeLibdrmPageFlipEventPoller {
    source: FakePageFlipCallbackSource,
}

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

impl LibdrmPageFlipEventPoller for FakeLibdrmPageFlipEventPoller {
    fn poll_page_flip_events(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LibdrmPageFlipEventPollReport {
        LibdrmPageFlipEventPollReport::from_source_report(self.source.emit_ready(sender, max_emit))
    }
}
