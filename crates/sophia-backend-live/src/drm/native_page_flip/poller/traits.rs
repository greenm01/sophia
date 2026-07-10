use super::super::*;
use crate::prelude::*;
use std::sync::mpsc::SyncSender;

pub trait LibdrmPageFlipEventPoller {
    fn poll_page_flip_events(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LibdrmPageFlipEventPollReport;
}
