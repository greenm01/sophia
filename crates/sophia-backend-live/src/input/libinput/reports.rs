use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibinputNativeEventAdapterReport {
    pub status: LibinputNativeEventAdapterStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibinputNativeEventAdapterStatus {
    SkeletonReady,
}

pub const fn native_libinput_event_adapter_report() -> LibinputNativeEventAdapterReport {
    LibinputNativeEventAdapterReport {
        status: LibinputNativeEventAdapterStatus::SkeletonReady,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LibinputNativeEventReadResult {
    pub report: LibinputNativeEventReadReport,
    pub events: Vec<InputEventPacket>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibinputNativeEventReadReport {
    pub status: LibinputNativeEventReadStatus,
    pub events_read: usize,
    pub queued_remaining: usize,
}

impl LibinputNativeEventReadReport {
    pub const fn idle() -> Self {
        Self {
            status: LibinputNativeEventReadStatus::Idle,
            events_read: 0,
            queued_remaining: 0,
        }
    }

    pub const fn would_block() -> Self {
        Self {
            status: LibinputNativeEventReadStatus::WouldBlock,
            events_read: 0,
            queued_remaining: 0,
        }
    }

    pub const fn events_read(events_read: usize, queued_remaining: usize) -> Self {
        Self {
            status: if events_read == 0 {
                LibinputNativeEventReadStatus::Idle
            } else {
                LibinputNativeEventReadStatus::EventsRead
            },
            events_read,
            queued_remaining,
        }
    }

    pub const fn read_failed() -> Self {
        Self {
            status: LibinputNativeEventReadStatus::ReadFailed,
            events_read: 0,
            queued_remaining: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibinputNativeEventReadStatus {
    Idle,
    WouldBlock,
    EventsRead,
    ReadFailed,
}

pub trait LiveLibinputEventReader {
    fn read_ready_input_events(&mut self, max_read: usize) -> LibinputNativeEventReadResult;
}
