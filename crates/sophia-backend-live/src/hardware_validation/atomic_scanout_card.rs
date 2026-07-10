use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::hardware_validation::preflight::nodes::is_primary_card_node_entry;
use crate::prelude::*;

#[derive(Debug)]
pub struct RealAtomicScanoutCard(std::fs::File);

impl RealAtomicScanoutCard {
    fn open_nonblocking(path: &Path) -> io::Result<Self> {
        Ok(Self(
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
                .open(path)?,
        ))
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(self.0.try_clone()?))
    }

    pub fn try_clone_file(&self) -> io::Result<std::fs::File> {
        self.0.try_clone()
    }
}

impl AsFd for RealAtomicScanoutCard {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for RealAtomicScanoutCard {}
impl drm::control::Device for RealAtomicScanoutCard {}

#[derive(Debug)]
pub struct RealAtomicScanoutCardSelection {
    pub status: RealAtomicScanoutCardSelectionStatus,
    pub card: Option<RealAtomicScanoutCard>,
    pub selection: Option<LibdrmNativePrimaryPlaneSelection>,
}

impl RealAtomicScanoutCardSelection {
    fn failed(status: RealAtomicScanoutCardSelectionStatus) -> Self {
        Self {
            status,
            card: None,
            selection: None,
        }
    }

    fn selected(card: RealAtomicScanoutCard, selection: LibdrmNativePrimaryPlaneSelection) -> Self {
        Self {
            status: RealAtomicScanoutCardSelectionStatus::Selected,
            card: Some(card),
            selection: Some(selection),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RealAtomicScanoutCardReadiness {
    pub openable: bool,
    pub atomic_capable: bool,
    pub scanout_target: bool,
    pub atomic_properties: bool,
}

pub(crate) fn inspect_real_atomic_scanout_card(path: &Path) -> RealAtomicScanoutCardReadiness {
    let Ok(card) = RealAtomicScanoutCard::open_nonblocking(path) else {
        return RealAtomicScanoutCardReadiness::default();
    };
    inspect_opened_real_atomic_scanout_card(&card).readiness
}

struct RealAtomicScanoutCardInspection {
    readiness: RealAtomicScanoutCardReadiness,
    selection: Option<LibdrmNativePrimaryPlaneSelection>,
}

fn inspect_opened_real_atomic_scanout_card(
    card: &RealAtomicScanoutCard,
) -> RealAtomicScanoutCardInspection {
    let mut readiness = RealAtomicScanoutCardReadiness {
        openable: true,
        ..RealAtomicScanoutCardReadiness::default()
    };

    if !admit_atomic_scanout_client_capabilities(card) {
        return RealAtomicScanoutCardInspection {
            readiness,
            selection: None,
        };
    }
    readiness.atomic_capable = true;

    let target = select_native_primary_plane_target(card);
    if target.status != LibdrmNativePrimaryPlaneSelectionStatus::Selected {
        return RealAtomicScanoutCardInspection {
            readiness,
            selection: None,
        };
    }
    let Some(selection) = target.selection else {
        return RealAtomicScanoutCardInspection {
            readiness,
            selection: None,
        };
    };
    readiness.scanout_target = true;

    let properties = discover_native_primary_plane_property_handles(
        card,
        selection.connector,
        selection.crtc,
        selection.plane,
    );
    readiness.atomic_properties =
        properties.status == LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered;

    RealAtomicScanoutCardInspection {
        readiness,
        selection: readiness.atomic_properties.then_some(selection),
    }
}

#[derive(Debug)]
pub struct RealAtomicScanoutPageFlipSession {
    card: RealAtomicScanoutCard,
    selection: LibdrmNativePrimaryPlaneSelection,
    reader: NativeLibdrmPageFlipEventReader<RealAtomicScanoutCard>,
    poller: NativeLibdrmPageFlipEventPoller,
}

impl RealAtomicScanoutPageFlipSession {
    pub fn card(&self) -> &RealAtomicScanoutCard {
        &self.card
    }

    pub const fn selection(&self) -> LibdrmNativePrimaryPlaneSelection {
        self.selection
    }

    pub fn page_flip_parts_mut(
        &mut self,
    ) -> (
        &RealAtomicScanoutCard,
        &mut NativeLibdrmPageFlipEventReader<RealAtomicScanoutCard>,
        &mut NativeLibdrmPageFlipEventPoller,
    ) {
        (&self.card, &mut self.reader, &mut self.poller)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn render_device_discovery(&self) -> io::Result<RealAtomicScanoutRenderDeviceDiscovery> {
        RealAtomicScanoutRenderDeviceDiscovery::from_card(&self.card)
    }

    #[cfg(all(feature = "gbm-probe", feature = "libinput-events"))]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout<P, E>(
        &mut self,
        runtime: &mut LiveBackendRuntimeAssembly<LiveInputReadinessGatedPoller<P>>,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        P: NonBlockingInputPoller,
        E: RenderDeviceDiscoveryBackend,
    {
        runtime.run_session_loop_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with(
            input,
            readiness,
            page_flip_budget,
            &self.card,
            exporter,
            &mut self.reader,
            &mut self.poller,
            sender,
        )
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Debug)]
pub struct RealAtomicScanoutRenderDeviceDiscovery {
    device: std::fs::File,
}

#[cfg(feature = "gbm-probe")]
impl RealAtomicScanoutRenderDeviceDiscovery {
    pub fn from_card(card: &RealAtomicScanoutCard) -> io::Result<Self> {
        Ok(Self {
            device: card.try_clone_file()?,
        })
    }
}

#[cfg(feature = "gbm-probe")]
impl RenderDeviceDiscoveryBackend for RealAtomicScanoutRenderDeviceDiscovery {
    type Device = std::fs::File;

    fn open_render_device(&self) -> io::Result<Self::Device> {
        self.device.try_clone()
    }
}

#[derive(Debug)]
pub struct RealAtomicScanoutPageFlipSessionResult {
    pub status: RealAtomicScanoutPageFlipSessionStatus,
    pub card_selection_status: RealAtomicScanoutCardSelectionStatus,
    pub session: Option<RealAtomicScanoutPageFlipSession>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealAtomicScanoutPageFlipSessionStatus {
    Ready,
    CardSelectionFailed,
    CardCloneFailed,
}

impl RealAtomicScanoutCardSelection {
    pub fn into_page_flip_session(
        mut self,
        slot: LibdrmNativeOutputSlot,
        output: OutputId,
        authority: LibdrmBackendFdAuthority,
    ) -> RealAtomicScanoutPageFlipSessionResult {
        let Some(card) = self.card.take() else {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed,
                card_selection_status: self.status,
                session: None,
            };
        };
        let Some(selection) = self.selection else {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed,
                card_selection_status: self.status,
                session: None,
            };
        };
        if self.status != RealAtomicScanoutCardSelectionStatus::Selected {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed,
                card_selection_status: self.status,
                session: None,
            };
        };

        let Ok(reader_card) = card.try_clone() else {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardCloneFailed,
                card_selection_status: self.status,
                session: None,
            };
        };
        let reader = NativeLibdrmPageFlipEventReader::new(reader_card)
            .with_crtc_routes([selection.crtc_route(slot)]);
        let poller = NativeLibdrmPageFlipEventPoller::new(
            LibdrmNativePageFlipSource::from_authority(authority),
        )
        .with_routes([LibdrmNativeOutputRoute { slot, output }]);

        RealAtomicScanoutPageFlipSessionResult {
            status: RealAtomicScanoutPageFlipSessionStatus::Ready,
            card_selection_status: self.status,
            session: Some(RealAtomicScanoutPageFlipSession {
                card,
                selection,
                reader,
                poller,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealAtomicScanoutCardSelectionStatus {
    Selected,
    DeviceDirectoryUnavailable,
    NoPrimaryCardNodes,
    PrimaryCardOpenUnavailable,
    AtomicClientCapabilityUnavailable,
    KmsScanoutTargetUnavailable,
    AtomicPropertyDiscoveryUnavailable,
}

pub fn select_real_atomic_scanout_card() -> RealAtomicScanoutCardSelection {
    select_real_atomic_scanout_card_from_dev_dri(Path::new("/dev/dri"))
}

pub fn select_real_atomic_scanout_card_from_dev_dri(
    dev_dri: &Path,
) -> RealAtomicScanoutCardSelection {
    let Ok(entries) = std::fs::read_dir(dev_dri) else {
        return RealAtomicScanoutCardSelection::failed(
            RealAtomicScanoutCardSelectionStatus::DeviceDirectoryUnavailable,
        );
    };
    let mut candidates = entries
        .filter_map(Result::ok)
        .filter(is_primary_card_node_entry)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return RealAtomicScanoutCardSelection::failed(
            RealAtomicScanoutCardSelectionStatus::NoPrimaryCardNodes,
        );
    }
    candidates.sort();

    let mut saw_openable = false;
    let mut saw_atomic_capable = false;
    let mut saw_scanout_target = false;

    for path in candidates {
        let Ok(card) = RealAtomicScanoutCard::open_nonblocking(&path) else {
            continue;
        };
        let inspection = inspect_opened_real_atomic_scanout_card(&card);
        saw_openable |= inspection.readiness.openable;
        saw_atomic_capable |= inspection.readiness.atomic_capable;
        saw_scanout_target |= inspection.readiness.scanout_target;

        let Some(selection) = inspection.selection else {
            continue;
        };
        return RealAtomicScanoutCardSelection::selected(card, selection);
    }

    RealAtomicScanoutCardSelection::failed(if !saw_openable {
        RealAtomicScanoutCardSelectionStatus::PrimaryCardOpenUnavailable
    } else if !saw_atomic_capable {
        RealAtomicScanoutCardSelectionStatus::AtomicClientCapabilityUnavailable
    } else if !saw_scanout_target {
        RealAtomicScanoutCardSelectionStatus::KmsScanoutTargetUnavailable
    } else {
        RealAtomicScanoutCardSelectionStatus::AtomicPropertyDiscoveryUnavailable
    })
}

fn admit_atomic_scanout_client_capabilities(card: &RealAtomicScanoutCard) -> bool {
    drm::Device::set_client_capability(card, drm::ClientCapability::UniversalPlanes, true).is_ok()
        && drm::Device::set_client_capability(card, drm::ClientCapability::Atomic, true).is_ok()
}
