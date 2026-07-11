use crate::DrmKmsOutputRegistry;
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputLogicalGeometry {
    pub output: OutputId,
    pub logical: Rect,
    pub pixel_size: Size,
    pub scale: u32,
}

impl OutputLogicalGeometry {
    pub fn project_rect(self, rect: Rect) -> Option<Rect> {
        let left = rect.x.max(self.logical.x);
        let top = rect.y.max(self.logical.y);
        let right = rect
            .x
            .saturating_add(rect.width)
            .min(self.logical.x.saturating_add(self.logical.width));
        let bottom = rect
            .y
            .saturating_add(rect.height)
            .min(self.logical.y.saturating_add(self.logical.height));
        let width = right.saturating_sub(left);
        let height = bottom.saturating_sub(top);
        (width > 0 && height > 0).then_some(Rect {
            x: left.saturating_sub(self.logical.x),
            y: top.saturating_sub(self.logical.y),
            width,
            height,
        })
    }

    pub fn project_damage(self, damage: &Region) -> Region {
        Region {
            rects: damage
                .rects
                .iter()
                .filter_map(|rect| self.project_rect(*rect))
                .collect(),
        }
    }

    pub fn project_rect_pixels(self, rect: Rect) -> Option<Rect> {
        let local = self.project_rect(rect)?;
        let scale = i32::try_from(self.scale.max(1)).unwrap_or(i32::MAX);
        Some(Rect {
            x: local.x.saturating_mul(scale),
            y: local.y.saturating_mul(scale),
            width: local.width.saturating_mul(scale).min(self.pixel_size.width),
            height: local
                .height
                .saturating_mul(scale)
                .min(self.pixel_size.height),
        })
    }

    pub fn project_damage_pixels(self, damage: &Region) -> Region {
        Region {
            rects: damage
                .rects
                .iter()
                .filter_map(|rect| self.project_rect_pixels(*rect))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExtendedDesktopTopology {
    outputs: BTreeMap<OutputId, OutputLogicalGeometry>,
    logical_size: Size,
}

impl ExtendedDesktopTopology {
    pub fn from_drm_outputs(outputs: &DrmKmsOutputRegistry) -> Self {
        let mut logical_x = 0i32;
        let mut logical_height = 0i32;
        let mut geometries = BTreeMap::new();
        for output in outputs.outputs() {
            let scale = output.scale.max(1);
            let scale_i32 = i32::try_from(scale).unwrap_or(i32::MAX);
            let logical_size = Size {
                width: output.mode.size.width.saturating_div(scale_i32).max(1),
                height: output.mode.size.height.saturating_div(scale_i32).max(1),
            };
            let logical = Rect {
                x: logical_x,
                y: 0,
                width: logical_size.width,
                height: logical_size.height,
            };
            geometries.insert(
                output.output,
                OutputLogicalGeometry {
                    output: output.output,
                    logical,
                    pixel_size: output.mode.size,
                    scale,
                },
            );
            logical_x = logical_x.saturating_add(logical_size.width);
            logical_height = logical_height.max(logical_size.height);
        }
        Self {
            outputs: geometries,
            logical_size: Size {
                width: logical_x,
                height: logical_height,
            },
        }
    }

    pub fn get(&self, output: OutputId) -> Option<&OutputLogicalGeometry> {
        self.outputs.get(&output)
    }

    pub fn outputs(&self) -> impl Iterator<Item = &OutputLogicalGeometry> {
        self.outputs.values()
    }

    pub const fn logical_size(&self) -> Size {
        self.logical_size
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputVrrCapability {
    pub capable: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputVrrEligibility {
    pub opaque_fullscreen_surface_count: u8,
    pub unoccluded: bool,
    pub overlays_present: bool,
    pub composition_required: bool,
}

impl OutputVrrEligibility {
    pub const fn fullscreen_eligible(self) -> bool {
        self.opaque_fullscreen_surface_count == 1
            && self.unoccluded
            && !self.overlays_present
            && !self.composition_required
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputVrrDecision {
    Enabled,
    DisabledByPolicy,
    Unsupported,
    Ineligible,
}

pub const fn decide_output_vrr(
    policy_enabled: bool,
    capability: OutputVrrCapability,
    eligibility: OutputVrrEligibility,
) -> OutputVrrDecision {
    if !policy_enabled {
        OutputVrrDecision::DisabledByPolicy
    } else if !capability.capable {
        OutputVrrDecision::Unsupported
    } else if !eligibility.fullscreen_eligible() {
        OutputVrrDecision::Ineligible
    } else {
        OutputVrrDecision::Enabled
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadlessOutput {
    pub id: OutputId,
    pub size: Size,
    pub scale: u32,
}

impl HeadlessOutput {
    pub const fn deterministic() -> Self {
        Self {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1280,
                height: 720,
            },
            scale: 1,
        }
    }
}

impl Default for HeadlessOutput {
    fn default() -> Self {
        Self::deterministic()
    }
}
