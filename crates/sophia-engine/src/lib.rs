use sophia_protocol::{LayerSnapshot, OutputId, Region, RenderCommand};

#[derive(Clone, Debug, PartialEq)]
pub struct FrameSnapshot {
    pub output: OutputId,
    pub layers: Vec<LayerSnapshot>,
    pub commands: Vec<RenderCommand>,
    pub damage: Region,
}

#[derive(Default)]
pub struct HeadlessEngine;

impl HeadlessEngine {
    pub fn plan_frame(&self, output: OutputId, layers: Vec<LayerSnapshot>) -> FrameSnapshot {
        FrameSnapshot {
            output,
            layers,
            commands: Vec::new(),
            damage: Region::empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_engine_returns_frame_value() {
        let engine = HeadlessEngine;
        let output = OutputId::from_raw(1);
        let frame = engine.plan_frame(output, Vec::new());

        assert_eq!(frame.output, output);
        assert!(frame.layers.is_empty());
    }
}
