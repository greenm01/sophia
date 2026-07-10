use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::*;
use sophia_backend_live::LivePageFlipCallbackIntake;
use sophia_renderer_live::LiveRendererScanoutBufferExportStatus;

#[derive(Debug)]
struct RealDrmCard(std::fs::File);

impl RealDrmCard {
    fn open(path: &Path) -> io::Result<Self> {
        Ok(Self(
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
                .open(path)?,
        ))
    }

    fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(self.0.try_clone()?))
    }

    fn try_clone_file(&self) -> io::Result<std::fs::File> {
        self.0.try_clone()
    }
}

impl AsFd for RealDrmCard {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for RealDrmCard {}
impl drm::control::Device for RealDrmCard {}

#[test]
fn native_atomic_scanout_smokes_real_primary_card_when_enabled() {
    if std::env::var_os("SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE").is_none() {
        return;
    }

    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("atomic_scanout_hardware_smoke::native_atomic_scanout_real_primary_card_child")
        .arg("--nocapture")
        .env("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD", "1")
        .spawn()
        .expect("real atomic scanout smoke child should start");
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        if let Some(status) = child
            .try_wait()
            .expect("real atomic scanout smoke child should be waitable")
        {
            assert!(
                status.success(),
                "real atomic scanout smoke child failed with status {status}"
            );
            return;
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            println!(
                "{}",
                LibdrmNativeAtomicScanoutSmokeEvidence::smoke_child_timeout().reduced_log_line()
            );
            panic!("real atomic scanout smoke child timed out waiting for page-flip evidence");
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn native_atomic_scanout_real_primary_card_child() {
    if std::env::var_os("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD").is_none() {
        return;
    }

    let Some(card_path) = first_atomic_scanout_ready_primary_card_node() else {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::no_primary_card();
        fail_atomic_scanout_smoke(evidence);
    };
    let card = match RealDrmCard::open(&card_path) {
        Ok(card) => card,
        Err(_) => {
            let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::primary_card_open_failed();
            fail_atomic_scanout_smoke(evidence);
        }
    };

    if drm::Device::set_client_capability(&card, drm::ClientCapability::UniversalPlanes, true)
        .is_err()
        || drm::Device::set_client_capability(&card, drm::ClientCapability::Atomic, true).is_err()
    {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::client_capability_failed();
        fail_atomic_scanout_smoke(evidence);
    }

    let selection = select_native_primary_plane_target(&card);
    if selection.status != LibdrmNativePrimaryPlaneSelectionStatus::Selected {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed();
        fail_atomic_scanout_smoke(evidence);
    }
    let Some(selected) = selection.selection else {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed();
        fail_atomic_scanout_smoke(evidence);
    };
    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let output = OutputId::from_raw(1);
    let target = LiveGbmEglFrameTargetRecord::new(selected.size());
    let scanout_target = if !target.is_valid_scanout_target() {
        LiveKmsScanoutTargetStatus::InvalidFrameTarget
    } else if target.size != selected.size() {
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    } else {
        LiveKmsScanoutTargetStatus::Ready
    };
    if scanout_target != LiveKmsScanoutTargetStatus::Ready {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            None,
            LiveRendererScanoutBufferExportStatus::Unavailable,
            None,
            None,
            None,
            None,
        );
        fail_atomic_scanout_smoke(evidence);
    }

    let context_report =
        NativeGbmRenderedScanoutContext::from_backend_device_result(card.try_clone_file());
    let rendered_context = match context_report.status {
        NativeGbmRenderedScanoutContextStatus::Ready => {
            LibdrmNativeRenderedScanoutContextStatus::Ready
        }
        NativeGbmRenderedScanoutContextStatus::Unavailable => {
            LibdrmNativeRenderedScanoutContextStatus::Unavailable
        }
        NativeGbmRenderedScanoutContextStatus::Degraded => {
            LibdrmNativeRenderedScanoutContextStatus::Degraded
        }
    };
    let Some(context) = context_report.context else {
        let export_status = match context_report.status {
            NativeGbmRenderedScanoutContextStatus::Ready => {
                LiveRendererScanoutBufferExportStatus::Degraded
            }
            NativeGbmRenderedScanoutContextStatus::Unavailable => {
                LiveRendererScanoutBufferExportStatus::Unavailable
            }
            NativeGbmRenderedScanoutContextStatus::Degraded => {
                LiveRendererScanoutBufferExportStatus::Degraded
            }
        };
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            export_status,
            None,
            None,
            None,
            None,
        );
        fail_atomic_scanout_smoke(evidence);
    };

    let export = context.export_rendered_owned_scanout_buffer(target);
    if export.status != LiveRendererScanoutBufferExportStatus::Exported {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            export.status,
            None,
            None,
            None,
            None,
        );
        fail_atomic_scanout_smoke(evidence);
    }
    let Some(owned_buffer) = export.buffer else {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            export.status,
            None,
            None,
            None,
            None,
        );
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        fail_atomic_scanout_smoke(evidence);
    };

    let mut submit = submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
        &card,
        LibdrmNativePrimaryPlaneSelectionResult {
            status: selection.status,
            selection: Some(selected),
        },
        owned_buffer.descriptor(),
    );
    if submit.status != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            export.status,
            Some(&submit),
            None,
            None,
            None,
        );
        fail_atomic_scanout_smoke(evidence);
    }
    let Some(submission) = submit.submission.take() else {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            export.status,
            Some(&submit),
            None,
            None,
            None,
        );
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        fail_atomic_scanout_smoke(evidence);
    };

    let mut reader = match card.try_clone() {
        Ok(card) => {
            NativeLibdrmPageFlipEventReader::new(card).with_crtc_routes([selected.crtc_route(slot)])
        }
        Err(_) => {
            let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
                scanout_target,
                Some(rendered_context),
                export.status,
                Some(&submit),
                None,
                None,
                None,
            );
            evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::PageFlipReaderUnavailable;
            fail_atomic_scanout_smoke(evidence);
        }
    };
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(1).expect("nonzero authority generation should mint"),
    );
    let mut poller = NativeLibdrmPageFlipEventPoller::new(source)
        .with_routes([LibdrmNativeOutputRoute { slot, output }]);
    let mut intake = LivePageFlipCallbackIntake::new(output);
    let page_flip =
        wait_for_page_flip_retirement(&card, &mut reader, &mut poller, &mut intake, submission);

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        scanout_target,
        Some(rendered_context),
        export.status,
        Some(&submit),
        Some(&page_flip.poll),
        page_flip.callback_report.as_ref(),
        page_flip.retired.as_ref(),
    );
    require_atomic_scanout_smoke_passed(evidence);
    drop(owned_buffer);

    let steady_export = context.export_rendered_owned_scanout_buffer(target);
    if steady_export.status != LiveRendererScanoutBufferExportStatus::Exported {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            steady_export.status,
            None,
            None,
            None,
            None,
        );
        fail_atomic_scanout_smoke(evidence);
    }
    let Some(steady_owned_buffer) = steady_export.buffer else {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            steady_export.status,
            None,
            None,
            None,
            None,
        );
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        fail_atomic_scanout_smoke(evidence);
    };

    let mut steady_submit =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &card,
            LibdrmNativePrimaryPlaneSelectionResult {
                status: selection.status,
                selection: Some(selected),
            },
            steady_owned_buffer.descriptor(),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );
    if steady_submit.status
        != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            steady_export.status,
            Some(&steady_submit),
            None,
            None,
            None,
        );
        fail_atomic_scanout_smoke(evidence);
    }
    let Some(steady_submission) = steady_submit.submission.take() else {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            steady_export.status,
            Some(&steady_submit),
            None,
            None,
            None,
        );
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        fail_atomic_scanout_smoke(evidence);
    };

    let steady_page_flip = wait_for_page_flip_retirement(
        &card,
        &mut reader,
        &mut poller,
        &mut intake,
        steady_submission,
    );

    let steady_evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
        scanout_target,
        Some(rendered_context),
        steady_export.status,
        Some(&steady_submit),
        Some(&steady_page_flip.poll),
        steady_page_flip.callback_report.as_ref(),
        steady_page_flip.retired.as_ref(),
    );
    require_atomic_scanout_smoke_passed(steady_evidence);
    drop(steady_owned_buffer);
}

fn require_atomic_scanout_smoke_passed(evidence: LibdrmNativeAtomicScanoutSmokeEvidence) {
    println!("{}", evidence.reduced_log_line());
    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::Passed
    );
}

fn fail_atomic_scanout_smoke(evidence: LibdrmNativeAtomicScanoutSmokeEvidence) -> ! {
    println!("{}", evidence.reduced_log_line());
    panic!(
        "real atomic scanout smoke failed with status {:?}",
        evidence.status
    );
}

struct RealAtomicPageFlipWaitReport {
    poll: LibdrmPageFlipEventPollReport,
    callback_report: Option<LivePageFlipCallbackReport>,
    retired: Option<LibdrmNativePrimaryPlaneScanoutRetireResult>,
}

fn wait_for_page_flip_retirement(
    card: &RealDrmCard,
    reader: &mut NativeLibdrmPageFlipEventReader<RealDrmCard>,
    poller: &mut NativeLibdrmPageFlipEventPoller,
    intake: &mut LivePageFlipCallbackIntake,
    submission: LibdrmNativePrimaryPlaneScanoutSubmission,
) -> RealAtomicPageFlipWaitReport {
    let (sender, receiver) = mpsc::sync_channel(1);
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut submission = Some(submission);

    loop {
        let report = poller.read_and_poll_page_flip_events(reader, &sender, 4, 1);
        let last_poll = report.poll;

        if let Ok(callback) = receiver.try_recv() {
            let callback_report = intake.observe(callback);
            let retired = retire_native_primary_plane_scanout_after_page_flip(
                card,
                submission
                    .take()
                    .expect("callback path should still own submitted resources"),
                &callback_report,
            );
            return RealAtomicPageFlipWaitReport {
                poll: last_poll,
                callback_report: Some(callback_report),
                retired: Some(retired),
            };
        }

        if matches!(
            last_poll.status,
            LibdrmPageFlipEventPollStatus::Disconnected
                | LibdrmPageFlipEventPollStatus::Backpressure
        ) || Instant::now() >= deadline
        {
            return RealAtomicPageFlipWaitReport {
                poll: last_poll,
                callback_report: None,
                retired: submission.map(|submission| LibdrmNativePrimaryPlaneScanoutRetireResult {
                    status: LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip,
                    destroy: None,
                    submission: Some(submission),
                    cleanup: None,
                }),
            };
        }

        std::thread::sleep(Duration::from_millis(5));
    }
}

fn first_atomic_scanout_ready_primary_card_node() -> Option<PathBuf> {
    let entries = std::fs::read_dir("/dev/dri").ok()?;
    let mut candidates = Vec::new();

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with("card") {
            candidates.push(entry.path());
        }
    }

    candidates.sort();
    candidates
        .into_iter()
        .find(|path| primary_card_node_is_atomic_scanout_ready(path))
}

fn primary_card_node_is_atomic_scanout_ready(path: &Path) -> bool {
    let Ok(card) = RealDrmCard::open(path) else {
        return false;
    };

    if drm::Device::set_client_capability(&card, drm::ClientCapability::UniversalPlanes, true)
        .is_err()
        || drm::Device::set_client_capability(&card, drm::ClientCapability::Atomic, true).is_err()
    {
        return false;
    }

    let selection = select_native_primary_plane_target(&card);
    selection.status == LibdrmNativePrimaryPlaneSelectionStatus::Selected
        && selection.selection.is_some()
}
