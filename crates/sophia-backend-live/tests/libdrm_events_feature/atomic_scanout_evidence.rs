use super::*;

#[test]
fn native_atomic_scanout_smoke_evidence_passes_only_after_submit_page_flip_and_retire() {
    let device = full_primary_plane_scanout_device();
    let mut submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = submit
        .submission
        .take()
        .expect("submitted scanout should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(42),
        },
    };
    let retired =
        retire_native_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&retired),
    );

    assert_eq!(
        evidence,
        LibdrmNativeAtomicScanoutSmokeEvidence {
            phase: LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            status: LibdrmNativeAtomicScanoutSmokeStatus::Passed,
            scanout_target: Some(LiveKmsScanoutTargetStatus::Ready),
            rendered_context: Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            gbm_export: Some(LiveRendererScanoutBufferExportStatus::Exported),
            gbm_export_detail: Some(LiveRendererScanoutBufferExportDetail::Exported),
            scanout_buffer: Some(sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready),
            buffer_format: Some(LibdrmNativeScanoutBufferFormatDetail::Xrgb8888),
            buffer_modifier: Some(LibdrmNativeScanoutBufferModifierDetail::Implicit),
            buffer_planes: Some(LibdrmNativeScanoutBufferPlaneDetail::Single),
            properties: Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered),
            format_table: Some(LibdrmNativePrimaryPlaneFormatTableStatus::Present),
            resources: Some(LibdrmNativePrimaryPlaneResourceCreateStatus::Created),
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::CreatedWithAddFb2),
            request: Some(LibdrmNativeAtomicRequestBuildStatus::Built),
            submit: Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip),
            request_scope: Some(LibdrmNativeAtomicCommitRequestScope::Modeset),
            commit_flags: Some(LibdrmNativeAtomicCommitFlagsReport {
                page_flip_event: true,
                nonblocking: true,
                allow_modeset: true,
                test_only: false,
            }),
            page_flip_wait: Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::Retired),
            page_flip_poll: Some(LibdrmPageFlipEventPollStatus::Emitted),
            page_flip: Some(LivePageFlipEventStatus::Presented),
            retire: Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip),
            retire_destroy: Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed),
            retire_cleanup_pending: false,
        }
    );
    assert_eq!(
        evidence.reduced_log_line(),
        "sophia_atomic_scanout_evidence schema=10 phase=InitialModeset status=Passed scanout_target=Ready rendered_context=Ready gbm_export=Exported gbm_export_detail=Exported scanout_buffer=Ready buffer_format=Xrgb8888 buffer_modifier=Implicit buffer_planes=Single properties=Discovered format_table=Present resources=Created framebuffer=CreatedWithAddFb2 request=Built submit=SubmittedWaitingForPageFlip request_scope=Modeset commit_page_flip_event=true commit_nonblocking=true commit_allow_modeset=true commit_test_only=false page_flip_wait=Retired page_flip_poll=Emitted page_flip=Presented retire=RetiredAfterPageFlip retire_destroy=Destroyed retire_cleanup_pending=false"
    );
}

#[test]
fn native_atomic_scanout_steady_state_evidence_requires_page_flip_request_scope() {
    let device = full_primary_plane_scanout_device();
    let selection = select_native_primary_plane_target(&device);
    let mut submit =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &device,
            selection,
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );
    let submission = submit
        .submission
        .take()
        .expect("submitted page flip should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(43),
        },
    };
    let retired =
        retire_native_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&retired),
    );

    assert_eq!(
        evidence.phase,
        LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip
    );
    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::Passed
    );
    assert_eq!(
        evidence.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::PageFlip)
    );
    assert_eq!(
        evidence.commit_flags,
        Some(LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        })
    );
    assert_eq!(
        evidence.reduced_log_line(),
        "sophia_atomic_scanout_evidence schema=10 phase=SteadyPageFlip status=Passed scanout_target=Ready rendered_context=Ready gbm_export=Exported gbm_export_detail=Exported scanout_buffer=Ready buffer_format=Xrgb8888 buffer_modifier=Implicit buffer_planes=Single properties=Discovered format_table=Present resources=Created framebuffer=CreatedWithAddFb2 request=Built submit=SubmittedWaitingForPageFlip request_scope=PageFlip commit_page_flip_event=true commit_nonblocking=true commit_allow_modeset=false commit_test_only=false page_flip_wait=Retired page_flip_poll=Emitted page_flip=Presented retire=RetiredAfterPageFlip retire_destroy=Destroyed retire_cleanup_pending=false"
    );
}

#[test]
fn native_atomic_scanout_smoke_evidence_fails_closed_before_page_flip() {
    let device = full_primary_plane_scanout_device();
    let mut submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = submit
        .submission
        .take()
        .expect("submitted scanout should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 0,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        None,
        None,
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::PageFlipMissing
    );
    assert_eq!(
        evidence.submit,
        Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
    );
    assert_eq!(
        evidence.commit_flags,
        Some(LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: true,
            test_only: false,
        })
    );
    assert_eq!(
        evidence.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::Modeset)
    );
    assert_eq!(
        evidence.page_flip_poll,
        Some(LibdrmPageFlipEventPollStatus::Idle)
    );
    assert_eq!(
        evidence.page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::CallbackMissing)
    );
    assert_eq!(evidence.retire_destroy, None);
    assert_eq!(evidence.retire_cleanup_pending, false);
    assert_eq!(
        submission.retire(&device).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_atomic_scanout_smoke_evidence_records_waiting_retire_on_missing_page_flip() {
    let device = full_primary_plane_scanout_device();
    let mut submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = submit
        .submission
        .take()
        .expect("submitted scanout should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 0,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let waiting_retire = LibdrmNativePrimaryPlaneScanoutRetireResult {
        status: LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip,
        destroy: None,
        submission: Some(submission),
        cleanup: None,
    };

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        None,
        Some(&waiting_retire),
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::PageFlipMissing
    );
    assert_eq!(
        evidence.retire,
        Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip)
    );
    assert_eq!(
        evidence.page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::CallbackMissing)
    );
    assert_eq!(evidence.retire_destroy, None);
    assert_eq!(evidence.retire_cleanup_pending, false);
    assert_eq!(
        submission.retire(&device).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_atomic_scanout_smoke_evidence_classifies_page_flip_wait_failures() {
    let device = full_primary_plane_scanout_device();
    let submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );

    let backpressure_poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 0,
            queued_remaining: 1,
            backpressure: true,
            disconnected: false,
            max_reached: false,
        });
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            Some(&submit),
            Some(&backpressure_poll),
            None,
            None,
        )
        .page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::PollBackpressure)
    );

    let disconnected_poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 0,
            queued_remaining: 0,
            backpressure: false,
            disconnected: true,
            max_reached: false,
        });
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            Some(&submit),
            Some(&disconnected_poll),
            None,
            None,
        )
        .page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::PollDisconnected)
    );

    let rejected_callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(41),
        },
    };
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            Some(&submit),
            Some(&backpressure_poll),
            Some(&rejected_callback),
            None,
        )
        .page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::PollBackpressure)
    );

    let emitted_poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            Some(&submit),
            Some(&emitted_poll),
            Some(&rejected_callback),
            None,
        )
        .page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::CallbackRejected)
    );

    submit
        .submission
        .expect("submitted scanout should retain resources")
        .retire(&device);
}

#[test]
fn native_atomic_scanout_smoke_evidence_requires_modeset_request_scope() {
    let device = full_primary_plane_scanout_device();
    let mut submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    submit.request_scope = Some(LibdrmNativeAtomicCommitRequestScope::PageFlip);
    let submission = submit
        .submission
        .take()
        .expect("submitted scanout should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(42),
        },
    };
    let retired =
        retire_native_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&retired),
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::RequestShapeMismatch
    );
    assert_eq!(
        evidence.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::PageFlip)
    );
}

#[test]
fn native_atomic_scanout_smoke_evidence_requires_phase_commit_flags() {
    let device = full_primary_plane_scanout_device();
    let selection = select_native_primary_plane_target(&device);
    let mut submit =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &device,
            selection,
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );
    submit.commit_flags = Some(LibdrmNativeAtomicCommitFlagsReport {
        page_flip_event: true,
        nonblocking: true,
        allow_modeset: true,
        test_only: false,
    });
    let submission = submit
        .submission
        .take()
        .expect("submitted page flip should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(43),
        },
    };
    let retired =
        retire_native_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&retired),
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::RequestShapeMismatch
    );
    assert_eq!(
        evidence.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::PageFlip)
    );
    assert_eq!(
        evidence.commit_flags,
        Some(LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: true,
            test_only: false,
        })
    );
}

#[test]
fn native_atomic_scanout_smoke_evidence_fails_before_submit_for_not_ready_target() {
    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        None,
        None,
        None,
        None,
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::KmsTargetUnavailable
    );
    assert_eq!(
        evidence.scanout_target,
        Some(LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch)
    );
    assert!(evidence.submit.is_none());
    assert!(evidence.request_scope.is_none());
    assert!(evidence.commit_flags.is_none());
}

#[test]
fn native_atomic_scanout_smoke_evidence_reports_precise_submit_stage_failures() {
    let device = full_primary_plane_scanout_device();

    let forged_ready = sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
        status: sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready,
        size: Size {
            width: -1,
            height: 720,
        },
        ..sophia_renderer_live::LiveRendererScanoutBufferDescriptor::new(
            Size {
                width: 1280,
                height: 720,
            },
            1280 * 4,
            LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            17,
        )
    };
    let scanout_buffer_failed =
        submit_native_primary_plane_scanout_from_renderer_descriptor(&device, forged_ready);
    let scanout_buffer_evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&scanout_buffer_failed),
        None,
        None,
        None,
    );
    assert_eq!(
        scanout_buffer_evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::ScanoutBufferUnavailable
    );
    assert_eq!(
        scanout_buffer_evidence.scanout_buffer,
        Some(sophia_renderer_live::LiveRendererScanoutBufferStatus::Invalid)
    );

    let mut property_failed = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    property_failed.properties =
        Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingPlaneProperty);
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            Some(&property_failed),
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::PropertyDiscoveryFailed
    );
    property_failed
        .submission
        .take()
        .expect("submitted scanout should retain resources")
        .retire(&device);

    let mut resource_failed = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    resource_failed.resources =
        Some(LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed);
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            Some(&resource_failed),
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::ResourceCreationFailed
    );
    resource_failed
        .submission
        .take()
        .expect("submitted scanout should retain resources")
        .retire(&device);

    let mut request_failed = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    request_failed.request = Some(LibdrmNativeAtomicRequestBuildStatus::MissingModeBlob);
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            Some(&request_failed),
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::RequestBuildFailed
    );
    request_failed
        .submission
        .take()
        .expect("submitted scanout should retain resources")
        .retire(&device);

    let mut submit_failed = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    submit_failed.status = LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed;
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            Some(&submit_failed),
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::AtomicSubmitFailed
    );
    submit_failed
        .submission
        .take()
        .expect("submitted scanout should retain resources")
        .retire(&device);
}

#[test]
fn native_atomic_scanout_smoke_evidence_reports_resource_retire_failure() {
    let device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let mut submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = submit
        .submission
        .take()
        .expect("submitted scanout should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(42),
        },
    };
    let retired =
        retire_native_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&retired),
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::RetireFailed
    );
    assert_eq!(
        evidence.retire,
        Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::ResourceRetireFailed)
    );
    assert_eq!(
        evidence.page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::ResourceRetireFailed)
    );
    assert_eq!(
        evidence.retire_destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed)
    );
    assert_eq!(evidence.retire_cleanup_pending, true);
}

#[test]
fn native_atomic_scanout_smoke_evidence_requires_destroyed_retire_resources() {
    let device = full_primary_plane_scanout_device();
    let submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(42),
        },
    };
    let inconsistent_retire = LibdrmNativePrimaryPlaneScanoutRetireResult {
        status: LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip,
        destroy: Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed),
        submission: None,
        cleanup: None,
    };

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&inconsistent_retire),
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::RetireFailed
    );
    assert_eq!(
        evidence.retire_destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed)
    );
    assert_eq!(
        evidence.page_flip_wait,
        Some(LibdrmNativeAtomicScanoutPageFlipWaitStatus::RetireMissing)
    );
}

#[test]
fn native_atomic_scanout_smoke_evidence_records_reduced_early_failures() {
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::smoke_child_timeout().status,
        LibdrmNativeAtomicScanoutSmokeStatus::SmokeChildTimeout
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::no_primary_card().status,
        LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::primary_card_open_failed().status,
        LibdrmNativeAtomicScanoutSmokeStatus::PrimaryCardOpenFailed
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::client_capability_failed().status,
        LibdrmNativeAtomicScanoutSmokeStatus::ClientCapabilityFailed
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed().status,
        LibdrmNativeAtomicScanoutSmokeStatus::KmsSelectionFailed
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::property_discovery_failed().status,
        LibdrmNativeAtomicScanoutSmokeStatus::PropertyDiscoveryFailed
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            None,
            LiveRendererScanoutBufferExportStatus::Exported,
            None,
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::RenderedContextUnavailable
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Unavailable,
            None,
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::GbmExportFailed
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Unavailable),
            LiveRendererScanoutBufferExportStatus::Unavailable,
            None,
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::RenderedContextUnavailable
    );

    let mut retained_resource_missing =
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            None,
            None,
            None,
            None,
        );
    retained_resource_missing.status =
        LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
    assert!(
        retained_resource_missing
            .reduced_log_line()
            .contains("status=RetainedResourceMissing")
    );

    let mut page_flip_reader_unavailable =
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Exported,
            None,
            None,
            None,
            None,
        );
    page_flip_reader_unavailable.status =
        LibdrmNativeAtomicScanoutSmokeStatus::PageFlipReaderUnavailable;
    assert!(
        page_flip_reader_unavailable
            .reduced_log_line()
            .contains("status=PageFlipReaderUnavailable")
    );
}
