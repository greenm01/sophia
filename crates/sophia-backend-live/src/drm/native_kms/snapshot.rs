use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativeConnectorSnapshot {
    pub(crate) connected: bool,
    pub(crate) current_encoder: Option<drm::control::encoder::Handle>,
    encoders: Vec<drm::control::encoder::Handle>,
    pub(crate) mode_size: Option<Size>,
    pub(crate) native_mode: Option<drm::control::Mode>,
}

impl LibdrmNativeConnectorSnapshot {
    pub fn new(
        connected: bool,
        current_encoder: Option<drm::control::encoder::Handle>,
        encoders: impl IntoIterator<Item = drm::control::encoder::Handle>,
        mode_size: Option<Size>,
    ) -> Self {
        Self {
            connected,
            current_encoder,
            encoders: encoders.into_iter().collect(),
            mode_size,
            native_mode: None,
        }
    }

    pub fn new_with_native_mode(
        connected: bool,
        current_encoder: Option<drm::control::encoder::Handle>,
        encoders: impl IntoIterator<Item = drm::control::encoder::Handle>,
        mode: Option<drm::control::Mode>,
    ) -> Self {
        let mode_size = mode.map(|mode| {
            let (width, height) = mode.size();
            Size {
                width: i32::from(width),
                height: i32::from(height),
            }
        });
        Self {
            connected,
            current_encoder,
            encoders: encoders.into_iter().collect(),
            mode_size,
            native_mode: mode,
        }
    }

    pub(crate) fn ordered_encoders(&self) -> Vec<drm::control::encoder::Handle> {
        let mut handles = Vec::new();
        if let Some(current) = self.current_encoder {
            handles.push(current);
        }
        for encoder in self.encoders.iter().copied() {
            if !handles.contains(&encoder) {
                handles.push(encoder);
            }
        }
        handles
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativeEncoderSnapshot {
    current_crtc: Option<drm::control::crtc::Handle>,
    compatible_crtcs: Vec<drm::control::crtc::Handle>,
}

impl LibdrmNativeEncoderSnapshot {
    pub fn new(
        current_crtc: Option<drm::control::crtc::Handle>,
        compatible_crtcs: impl IntoIterator<Item = drm::control::crtc::Handle>,
    ) -> Self {
        Self {
            current_crtc,
            compatible_crtcs: compatible_crtcs.into_iter().collect(),
        }
    }

    pub(crate) fn ordered_crtcs(&self) -> Vec<drm::control::crtc::Handle> {
        let mut handles = Vec::new();
        if let Some(current) = self.current_crtc {
            handles.push(current);
        }
        for crtc in self.compatible_crtcs.iter().copied() {
            if !handles.contains(&crtc) {
                handles.push(crtc);
            }
        }
        handles
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativePlaneSnapshot {
    compatible_crtcs: Vec<drm::control::crtc::Handle>,
}

impl LibdrmNativePlaneSnapshot {
    pub fn new(compatible_crtcs: impl IntoIterator<Item = drm::control::crtc::Handle>) -> Self {
        Self {
            compatible_crtcs: compatible_crtcs.into_iter().collect(),
        }
    }

    pub(crate) fn supports_crtc(&self, crtc: drm::control::crtc::Handle) -> bool {
        self.compatible_crtcs.contains(&crtc)
    }
}
