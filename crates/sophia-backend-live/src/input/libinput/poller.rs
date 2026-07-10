use crate::prelude::*;

use super::{LibinputNativeEventReadReport, LibinputNativeEventReadStatus};

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLibinputEventPoller<R> {
    reader: R,
    max_read_per_poll: usize,
    last_read: LibinputNativeEventReadReport,
}

impl<R> NativeLibinputEventPoller<R> {
    pub fn new(reader: R, max_read_per_poll: usize) -> Self {
        Self {
            reader,
            max_read_per_poll,
            last_read: LibinputNativeEventReadReport::idle(),
        }
    }

    pub const fn last_read_report(&self) -> LibinputNativeEventReadReport {
        self.last_read
    }

    pub const fn max_read_per_poll(&self) -> usize {
        self.max_read_per_poll
    }

    pub fn reader(&self) -> &R {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut R {
        &mut self.reader
    }
}

impl<R> NonBlockingInputPoller for NativeLibinputEventPoller<R>
where
    R: LiveLibinputEventReader,
{
    fn poll_ready(&mut self) -> io::Result<Vec<InputEventPacket>> {
        let result = self.reader.read_ready_input_events(self.max_read_per_poll);
        self.last_read = result.report;
        if result.report.status == LibinputNativeEventReadStatus::ReadFailed {
            return Err(io::Error::other("reduced native libinput read failed"));
        }
        Ok(result.events)
    }
}
