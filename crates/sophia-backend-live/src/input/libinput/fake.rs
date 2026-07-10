use crate::prelude::*;

use super::{LibinputNativeEventReadReport, LibinputNativeEventReadResult};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FakeLiveLibinputEventReader {
    queued: VecDeque<InputEventPacket>,
    fail_next_read: bool,
}

impl FakeLiveLibinputEventReader {
    pub fn new(events: impl IntoIterator<Item = InputEventPacket>) -> Self {
        Self {
            queued: events.into_iter().collect(),
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

impl LiveLibinputEventReader for FakeLiveLibinputEventReader {
    fn read_ready_input_events(&mut self, max_read: usize) -> LibinputNativeEventReadResult {
        if self.fail_next_read {
            self.fail_next_read = false;
            return LibinputNativeEventReadResult {
                report: LibinputNativeEventReadReport::read_failed(),
                events: Vec::new(),
            };
        }

        let mut events = Vec::new();
        for _ in 0..max_read {
            let Some(event) = self.queued.pop_front() else {
                break;
            };
            events.push(event);
        }

        LibinputNativeEventReadResult {
            report: LibinputNativeEventReadReport::events_read(events.len(), self.queued.len()),
            events,
        }
    }
}
