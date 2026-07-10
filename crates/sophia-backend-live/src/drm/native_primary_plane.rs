use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativePrimaryPlaneResourceDevice {
    fn create_mode_blob_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64>;

    fn add_scanout_framebuffer<B>(
        &self,
        buffer: &B,
        depth: u32,
        bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized;

    fn destroy_scanout_framebuffer(
        &self,
        framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()>;

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()>;
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativePrimaryPlaneResourceDevice for D
where
    D: drm::control::Device,
{
    fn create_mode_blob_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        let Some(mode) = selection.mode else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "selected KMS target does not carry a native mode",
            ));
        };
        match self.create_property_blob(&mode)? {
            drm::control::property::Value::Blob(blob) => Ok(blob),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "DRM mode blob creation returned a non-blob value",
            )),
        }
    }

    fn add_scanout_framebuffer<B>(
        &self,
        buffer: &B,
        depth: u32,
        bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        self.add_framebuffer(buffer, depth, bpp)
    }

    fn destroy_scanout_framebuffer(
        &self,
        framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        self.destroy_framebuffer(framebuffer)
    }

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()> {
        self.destroy_property_blob(mode_blob)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceBundle {
    framebuffer: drm::control::framebuffer::Handle,
    mode_blob: u64,
    size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneResourceBundle {
    pub const fn into_objects(
        self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> LibdrmNativePrimaryPlaneObjects {
        selection.into_objects(self.framebuffer, self.mode_blob)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmRendererScanoutBuffer {
    size: Size,
    pitch: u32,
    format: u32,
    handle: drm::buffer::Handle,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmRendererScanoutBuffer {
    pub fn from_descriptor(descriptor: LiveRendererScanoutBufferDescriptor) -> Option<Self> {
        if descriptor.status != LiveRendererScanoutBufferStatus::Ready
            || descriptor.format != LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
            || descriptor.size.width <= 0
            || descriptor.size.height <= 0
            || descriptor.pitch == 0
        {
            return None;
        }

        Some(Self {
            size: descriptor.size,
            pitch: descriptor.pitch,
            format: descriptor.format,
            handle: drm::control::from_u32(descriptor.gem_handle)?,
        })
    }
}

#[cfg(feature = "libdrm-events")]
impl drm::buffer::Buffer for LibdrmRendererScanoutBuffer {
    fn size(&self) -> (u32, u32) {
        (self.size.width as u32, self.size.height as u32)
    }

    fn format(&self) -> drm::buffer::DrmFourcc {
        drm::buffer::DrmFourcc::Xrgb8888
    }

    fn pitch(&self) -> u32 {
        self.pitch
    }

    fn handle(&self) -> drm::buffer::Handle {
        self.handle
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneResourceCreateResult {
    pub status: LibdrmNativePrimaryPlaneResourceCreateStatus,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceBundle>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneResourceCreateStatus {
    Created,
    InvalidSelectionSize,
    BufferSizeMismatch,
    MissingMode,
    ModeBlobCreateFailed,
    FramebufferCreateFailed,
}

#[cfg(feature = "libdrm-events")]
pub fn create_native_primary_plane_resources<D, B>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    buffer: &B,
) -> LibdrmNativePrimaryPlaneResourceCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
    B: drm::buffer::Buffer + ?Sized,
{
    if selection.size.width <= 0 || selection.size.height <= 0 {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize,
            resources: None,
        };
    }

    let (buffer_width, buffer_height) = buffer.size();
    if buffer_width != selection.size.width as u32 || buffer_height != selection.size.height as u32
    {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::BufferSizeMismatch,
            resources: None,
        };
    }

    let mode_blob = match device.create_mode_blob_for_selection(selection) {
        Ok(mode_blob) => mode_blob,
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            return LibdrmNativePrimaryPlaneResourceCreateResult {
                status: LibdrmNativePrimaryPlaneResourceCreateStatus::MissingMode,
                resources: None,
            };
        }
        Err(_) => {
            return LibdrmNativePrimaryPlaneResourceCreateResult {
                status: LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed,
                resources: None,
            };
        }
    };
    let Ok(framebuffer) = device.add_scanout_framebuffer(buffer, 24, 32) else {
        let _ = device.destroy_mode_blob(mode_blob);
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed,
            resources: None,
        };
    };

    LibdrmNativePrimaryPlaneResourceCreateResult {
        status: LibdrmNativePrimaryPlaneResourceCreateStatus::Created,
        resources: Some(LibdrmNativePrimaryPlaneResourceBundle {
            framebuffer,
            mode_blob,
            size: selection.size,
        }),
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceDestroyReport {
    pub status: LibdrmNativePrimaryPlaneResourceDestroyStatus,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneResourceDestroyStatus {
    Destroyed,
    FramebufferDestroyFailed,
    ModeBlobDestroyFailed,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceCleanup {
    framebuffer: Option<drm::control::framebuffer::Handle>,
    mode_blob: Option<u64>,
    size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneResourceCleanup {
    pub const fn from_bundle(resources: LibdrmNativePrimaryPlaneResourceBundle) -> Self {
        Self {
            framebuffer: Some(resources.framebuffer),
            mode_blob: Some(resources.mode_blob),
            size: resources.size,
        }
    }

    pub fn retry<D>(self, device: &D) -> LibdrmNativePrimaryPlaneResourceDestroyReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        destroy_native_primary_plane_resource_cleanup(device, self)
    }
}

#[cfg(feature = "libdrm-events")]
pub fn destroy_native_primary_plane_resources<D>(
    device: &D,
    resources: LibdrmNativePrimaryPlaneResourceBundle,
) -> LibdrmNativePrimaryPlaneResourceDestroyReport
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    destroy_native_primary_plane_resource_cleanup(
        device,
        LibdrmNativePrimaryPlaneResourceCleanup::from_bundle(resources),
    )
}

#[cfg(feature = "libdrm-events")]
pub fn destroy_native_primary_plane_resource_cleanup<D>(
    device: &D,
    mut cleanup: LibdrmNativePrimaryPlaneResourceCleanup,
) -> LibdrmNativePrimaryPlaneResourceDestroyReport
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    if let Some(framebuffer) = cleanup.framebuffer {
        if device.destroy_scanout_framebuffer(framebuffer).is_err() {
            return LibdrmNativePrimaryPlaneResourceDestroyReport {
                status: LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed,
                cleanup: Some(cleanup),
            };
        }
        cleanup.framebuffer = None;
    }

    if let Some(mode_blob) = cleanup.mode_blob {
        if device.destroy_mode_blob(mode_blob).is_err() {
            return LibdrmNativePrimaryPlaneResourceDestroyReport {
                status: LibdrmNativePrimaryPlaneResourceDestroyStatus::ModeBlobDestroyFailed,
                cleanup: Some(cleanup),
            };
        }
        cleanup.mode_blob = None;
    }

    if cleanup.framebuffer.is_some() || cleanup.mode_blob.is_some() {
        return LibdrmNativePrimaryPlaneResourceDestroyReport {
            status: LibdrmNativePrimaryPlaneResourceDestroyStatus::ModeBlobDestroyFailed,
            cleanup: Some(cleanup),
        };
    }

    LibdrmNativePrimaryPlaneResourceDestroyReport {
        status: LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed,
        cleanup: None,
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneObjects {
    connector: drm::control::connector::Handle,
    crtc: drm::control::crtc::Handle,
    plane: drm::control::plane::Handle,
    framebuffer: drm::control::framebuffer::Handle,
    mode_blob: u64,
    size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneObjects {
    pub const fn new(
        connector: drm::control::connector::Handle,
        crtc: drm::control::crtc::Handle,
        plane: drm::control::plane::Handle,
        framebuffer: drm::control::framebuffer::Handle,
        mode_blob: u64,
        size: Size,
    ) -> Self {
        Self {
            connector,
            crtc,
            plane,
            framebuffer,
            mode_blob,
            size,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativeAtomicRequestBuildResult {
    pub status: LibdrmNativeAtomicRequestBuildStatus,
    pub request: Option<LibdrmNativeAtomicCommitRequest>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicRequestBuildStatus {
    Built,
    InvalidSize,
}

#[cfg(feature = "libdrm-events")]
pub fn build_native_primary_plane_atomic_request(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
) -> LibdrmNativeAtomicRequestBuildResult {
    if objects.size.width <= 0 || objects.size.height <= 0 {
        return LibdrmNativeAtomicRequestBuildResult {
            status: LibdrmNativeAtomicRequestBuildStatus::InvalidSize,
            request: None,
        };
    }

    let width = objects.size.width as u64;
    let height = objects.size.height as u64;
    let mut request = drm::control::atomic::AtomicModeReq::new();
    request.add_property(
        objects.connector,
        properties.connector_crtc_id,
        drm::control::property::Value::CRTC(Some(objects.crtc)),
    );
    request.add_property(
        objects.crtc,
        properties.crtc_mode_id,
        drm::control::property::Value::Blob(objects.mode_blob),
    );
    request.add_property(
        objects.crtc,
        properties.crtc_active,
        drm::control::property::Value::Boolean(true),
    );
    request.add_property(
        objects.plane,
        properties.plane_fb_id,
        drm::control::property::Value::Framebuffer(Some(objects.framebuffer)),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_id,
        drm::control::property::Value::CRTC(Some(objects.crtc)),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_x,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_y,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_w,
        drm::control::property::Value::UnsignedRange(width << 16),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_h,
        drm::control::property::Value::UnsignedRange(height << 16),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_x,
        drm::control::property::Value::SignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_y,
        drm::control::property::Value::SignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_w,
        drm::control::property::Value::UnsignedRange(width),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_h,
        drm::control::property::Value::UnsignedRange(height),
    );

    LibdrmNativeAtomicRequestBuildResult {
        status: LibdrmNativeAtomicRequestBuildStatus::Built,
        request: Some(LibdrmNativeAtomicCommitRequest::new(request)),
    }
}
