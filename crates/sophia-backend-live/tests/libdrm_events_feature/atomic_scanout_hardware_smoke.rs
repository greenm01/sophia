use std::time::{Duration, Instant};

use super::*;
use sophia_backend_live::{
    LivePageFlipCallbackIntake, RealAtomicScanoutCard, select_real_atomic_scanout_card,
};
use sophia_renderer_live::{
    LiveRendererScanoutBufferExportStatus, NativeGbmRenderedScanoutContextStatus,
};

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

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let output = OutputId::from_raw(1);
    let authority =
        LibdrmBackendFdAuthority::new(1).expect("nonzero authority generation should mint");
    let mut session_result =
        select_real_atomic_scanout_card().into_page_flip_session(slot, output, authority);
    let Some(mut session) = session_result.session.take() else {
        fail_atomic_scanout_smoke(
            session_result
                .failure_evidence()
                .unwrap_or_else(LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed),
        );
    };
    let selected = session.selection();
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

    let discovery = match session.render_device_discovery() {
        Ok(discovery) => discovery,
        Err(_) => {
            let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
                scanout_target,
                Some(LibdrmNativeRenderedScanoutContextStatus::Unavailable),
                LiveRendererScanoutBufferExportStatus::Unavailable,
                None,
                None,
                None,
                None,
            );
            fail_atomic_scanout_smoke(evidence);
        }
    };
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery);

    let export = exporter.export_rendered_scanout_buffer(target);
    let rendered_context = rendered_context_status_from_native(exporter.context_status());
    if export.status != LiveRendererScanoutBufferExportStatus::Exported {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            rendered_context,
            export.status,
            None,
            None,
            None,
            None,
        );
        fail_atomic_scanout_smoke(evidence);
    }
    let Some(descriptor) = export.descriptor else {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            rendered_context,
            export.status,
            None,
            None,
            None,
            None,
        );
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        fail_atomic_scanout_smoke(evidence);
    };
    let Some(owned_buffer) = export.owner else {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            rendered_context,
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
        session.card(),
        LibdrmNativePrimaryPlaneSelectionResult {
            status: LibdrmNativePrimaryPlaneSelectionStatus::Selected,
            selection: Some(selected),
        },
        descriptor,
    );
    if submit.status != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            rendered_context,
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
            rendered_context,
            export.status,
            Some(&submit),
            None,
            None,
            None,
        );
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        fail_atomic_scanout_smoke(evidence);
    };

    let mut intake = LivePageFlipCallbackIntake::new(output);
    let page_flip = {
        let (card, reader, poller) = session.page_flip_parts_mut();
        wait_for_page_flip_retirement(card, reader, poller, &mut intake, submission)
    };

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        scanout_target,
        rendered_context,
        export.status,
        Some(&submit),
        Some(&page_flip.poll),
        page_flip.callback_report.as_ref(),
        page_flip.retired.as_ref(),
    );
    require_atomic_scanout_smoke_passed(evidence);
    drop(owned_buffer);

    let steady_export = exporter.export_rendered_scanout_buffer(target);
    let steady_rendered_context = rendered_context_status_from_native(exporter.context_status());
    if steady_export.status != LiveRendererScanoutBufferExportStatus::Exported {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
            scanout_target,
            steady_rendered_context,
            steady_export.status,
            None,
            None,
            None,
            None,
        );
        fail_atomic_scanout_smoke(evidence);
    }
    let Some(steady_descriptor) = steady_export.descriptor else {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
            scanout_target,
            steady_rendered_context,
            steady_export.status,
            None,
            None,
            None,
            None,
        );
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        fail_atomic_scanout_smoke(evidence);
    };
    let Some(steady_owned_buffer) = steady_export.owner else {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
            scanout_target,
            steady_rendered_context,
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
            session.card(),
            LibdrmNativePrimaryPlaneSelectionResult {
                status: LibdrmNativePrimaryPlaneSelectionStatus::Selected,
                selection: Some(selected),
            },
            steady_descriptor,
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );
    if steady_submit.status
        != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    {
        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
            scanout_target,
            steady_rendered_context,
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
            steady_rendered_context,
            steady_export.status,
            Some(&steady_submit),
            None,
            None,
            None,
        );
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        fail_atomic_scanout_smoke(evidence);
    };

    let steady_page_flip = {
        let (card, reader, poller) = session.page_flip_parts_mut();
        wait_for_page_flip_retirement(card, reader, poller, &mut intake, steady_submission)
    };

    let steady_evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
        scanout_target,
        steady_rendered_context,
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

fn rendered_context_status_from_native(
    status: Option<NativeGbmRenderedScanoutContextStatus>,
) -> Option<LibdrmNativeRenderedScanoutContextStatus> {
    status.map(|status| match status {
        NativeGbmRenderedScanoutContextStatus::Ready => {
            LibdrmNativeRenderedScanoutContextStatus::Ready
        }
        NativeGbmRenderedScanoutContextStatus::Unavailable => {
            LibdrmNativeRenderedScanoutContextStatus::Unavailable
        }
        NativeGbmRenderedScanoutContextStatus::Degraded => {
            LibdrmNativeRenderedScanoutContextStatus::Degraded
        }
    })
}

struct RealAtomicPageFlipWaitReport {
    poll: LibdrmPageFlipEventPollReport,
    callback_report: Option<LivePageFlipCallbackReport>,
    retired: Option<LibdrmNativePrimaryPlaneScanoutRetireResult>,
}

fn wait_for_page_flip_retirement(
    card: &RealAtomicScanoutCard,
    reader: &mut NativeLibdrmPageFlipEventReader<RealAtomicScanoutCard>,
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
