use crate::TransactionId;

pub const SOPHIA_IPC_MAGIC: u32 = 0x4850_4f53;
pub const SOPHIA_IPC_VERSION: u16 = 1;
pub const SOPHIA_IPC_HEADER_LEN: usize = 24;
pub const SOPHIA_IPC_MAX_PAYLOAD_LEN: usize = 64 * 1024;
pub const SOPHIA_IPC_MAX_ITEMS: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IpcMessageKind {
    WmRequest = 1,
    WmResponse = 2,
    BrokerHealth = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpcFrameHeader {
    pub message_kind: IpcMessageKind,
    pub transaction: TransactionId,
    pub payload_len: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IpcCodecError {
    Truncated,
    BadMagic,
    UnsupportedVersion(u16),
    UnknownMessageKind(u16),
    PayloadTooLarge(usize),
    ReservedNonZero(u32),
    TrailingBytes(usize),
    CountTooLarge {
        count: usize,
        max: usize,
    },
    TextTooLarge {
        field: &'static str,
        len: usize,
        max: usize,
    },
    InvalidUtf8 {
        field: &'static str,
    },
    InvalidEnum {
        field: &'static str,
        value: u32,
    },
    InvalidBool {
        field: &'static str,
        value: u8,
    },
}
