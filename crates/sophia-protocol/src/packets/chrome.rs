use crate::ids::{IconTokenId, SurfaceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChromeDescriptor {
    pub surface: SurfaceId,
    pub label: Option<DisplayLabel>,
    pub icon: Option<IconTokenId>,
    pub trust_level: TrustLevel,
    pub attention: AttentionState,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayLabel {
    pub text: String,
    pub redacted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustLevel {
    Unknown,
    Trusted,
    Untrusted,
    Isolated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttentionState {
    None,
    Notice,
    Critical,
}

pub const SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerKind {
    Portal,
    Metadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerHealthState {
    Starting,
    Ready,
    Degraded,
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrokerHealthError {
    MessageTooLong { len: usize, max: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrokerHealthPacket {
    pub broker: BrokerKind,
    pub state: BrokerHealthState,
    pub generation: u64,
    pub message: Option<String>,
}

impl BrokerHealthPacket {
    pub fn new(
        broker: BrokerKind,
        state: BrokerHealthState,
        generation: u64,
        message: Option<String>,
    ) -> Result<Self, BrokerHealthError> {
        let packet = Self {
            broker,
            state,
            generation,
            message,
        };
        packet.validate()?;
        Ok(packet)
    }

    pub fn validate(&self) -> Result<(), BrokerHealthError> {
        if let Some(message) = &self.message {
            if message.len() > SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN {
                return Err(BrokerHealthError::MessageTooLong {
                    len: message.len(),
                    max: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
                });
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChromeActionRequest {
    pub surface: SurfaceId,
    pub generation: u64,
    pub kind: ChromeActionKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeActionKind {
    CloseSurfaceRequested,
}
