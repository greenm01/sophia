use super::*;
use crate::prelude::*;

mod events;
mod snapshots;
mod topology;

pub use events::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XMirrorState {
    windows: Vec<XWindowMirror>,
}
