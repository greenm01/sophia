use crate::ids::XWindowId;
use crate::{BufferSource, SurfaceId, TransactionCommit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityKind {
    SophiaX,
    SophiaWayland,
    SophiaNative,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityFeedback {
    Transaction(TransactionCommit),
    FrameScheduled(SurfacePresentationFeedback),
    Presented(SurfacePresentationFeedback),
    BufferReleased(BufferReleaseFeedback),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfacePresentationFeedback {
    pub surface: SurfaceId,
    pub generation: u64,
    pub presentation_msec: u64,
    pub presentation_micros: u64,
    pub output_msc: u64,
}

impl SurfacePresentationFeedback {
    pub const fn from_millis(
        surface: SurfaceId,
        generation: u64,
        presentation_msec: u64,
        output_msc: u64,
    ) -> Self {
        Self {
            surface,
            generation,
            presentation_msec,
            presentation_micros: presentation_msec.saturating_mul(1_000),
            output_msc,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferReleaseFeedback {
    pub surface: SurfaceId,
    pub source: BufferSource,
}

pub const DMA_BUF_MAX_PLANES: usize = 4;
pub const DMA_BUF_MAX_DIMENSION: i32 = 16_384;
pub const DMA_BUF_MAX_BYTES: u64 = 256 * 1024 * 1024;
pub const DRM_FORMAT_XRGB8888: u32 = u32::from_le_bytes(*b"XR24");
pub const DRM_FORMAT_ARGB8888: u32 = u32::from_le_bytes(*b"AR24");
pub const DRM_FORMAT_MOD_INVALID: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DmaBufPlaneDescriptor {
    pub offset: u32,
    pub stride: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DmaBufDescriptor {
    pub handle: crate::BufferHandle,
    pub size: crate::Size,
    pub format: u32,
    pub modifier: u64,
    pub plane_count: u8,
    pub planes: [Option<DmaBufPlaneDescriptor>; DMA_BUF_MAX_PLANES],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DmaBufDescriptorError {
    InvalidHandle,
    InvalidSize,
    UnsupportedFormat,
    InvalidPlaneCount,
    MissingPlane,
    UnexpectedPlane,
    InvalidStride,
    BufferTooLarge,
}

impl DmaBufDescriptor {
    pub fn validate(&self) -> Result<(), DmaBufDescriptorError> {
        if !self.handle.is_valid() {
            return Err(DmaBufDescriptorError::InvalidHandle);
        }
        if self.size.width <= 0
            || self.size.height <= 0
            || self.size.width > DMA_BUF_MAX_DIMENSION
            || self.size.height > DMA_BUF_MAX_DIMENSION
        {
            return Err(DmaBufDescriptorError::InvalidSize);
        }
        if !matches!(self.format, DRM_FORMAT_XRGB8888 | DRM_FORMAT_ARGB8888) {
            return Err(DmaBufDescriptorError::UnsupportedFormat);
        }
        let plane_count = usize::from(self.plane_count);
        if plane_count == 0 || plane_count > DMA_BUF_MAX_PLANES {
            return Err(DmaBufDescriptorError::InvalidPlaneCount);
        }
        for (index, plane) in self.planes.iter().enumerate() {
            if index < plane_count && plane.is_none() {
                return Err(DmaBufDescriptorError::MissingPlane);
            }
            if index >= plane_count && plane.is_some() {
                return Err(DmaBufDescriptorError::UnexpectedPlane);
            }
        }
        let width_bytes = u64::try_from(self.size.width)
            .ok()
            .and_then(|width| width.checked_mul(4))
            .ok_or(DmaBufDescriptorError::BufferTooLarge)?;
        let height =
            u64::try_from(self.size.height).map_err(|_| DmaBufDescriptorError::BufferTooLarge)?;
        for plane in self.planes.iter().take(plane_count).flatten() {
            let stride = u64::from(plane.stride);
            if stride < width_bytes {
                return Err(DmaBufDescriptorError::InvalidStride);
            }
            let end = u64::from(plane.offset)
                .checked_add(
                    height
                        .saturating_sub(1)
                        .checked_mul(stride)
                        .ok_or(DmaBufDescriptorError::BufferTooLarge)?,
                )
                .and_then(|value| value.checked_add(width_bytes))
                .ok_or(DmaBufDescriptorError::BufferTooLarge)?;
            if end > DMA_BUF_MAX_BYTES {
                return Err(DmaBufDescriptorError::BufferTooLarge);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuBufferFormat {
    Argb8888,
    Xrgb8888,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuBufferRegistration {
    pub handle: u64,
    pub size: crate::Size,
    pub stride: u32,
    pub format: CpuBufferFormat,
    pub generation: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthorityLocalId {
    raw: u64,
    generation: u32,
}

impl AuthorityLocalId {
    pub const NONE: Self = Self {
        raw: 0,
        generation: 0,
    };

    pub const fn new(raw: u64, generation: u32) -> Self {
        Self { raw, generation }
    }

    pub const fn raw(self) -> u64 {
        self.raw
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub const fn is_valid(self) -> bool {
        self.raw != 0 && self.generation != 0
    }
}

impl From<XWindowId> for AuthorityLocalId {
    fn from(window: XWindowId) -> Self {
        Self::new(u64::from(window.xid()), window.generation())
    }
}
