use crate::prelude::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveBrokerRuntimeAdapter;

impl LiveBrokerRuntimeAdapter {
    pub fn from_health_packet(packet: &BrokerHealthPacket) -> SessionRuntimeObservation {
        Self::health_observation(packet)
    }

    pub fn health_observation(packet: &BrokerHealthPacket) -> SessionRuntimeObservation {
        SessionRuntimeObservation::BrokerHealthChanged {
            broker: packet.broker,
            state: packet.state,
            generation: packet.generation,
            status_message_len: packet.message.as_deref().map(str::len).unwrap_or(0),
        }
    }
}
