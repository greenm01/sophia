use sophia_portal::{
    PortalCommand, PortalError, ScreenCaptureMode, ScreenCapturePortal, ScreenCaptureRequest,
    ScreenCaptureScope,
};
use sophia_protocol::{NamespaceId, PortalDecision, PortalTransferId, PortalTransferKind};

fn request(
    transfer: u64,
    generation: u64,
    mode: ScreenCaptureMode,
    mime_type: &str,
) -> ScreenCaptureRequest {
    ScreenCaptureRequest {
        transfer: PortalTransferId::from_raw(transfer),
        source_namespace: NamespaceId::from_raw(10),
        target_namespace: NamespaceId::from_raw(20),
        mode,
        scope: ScreenCaptureScope::Output,
        mime_type: mime_type.to_owned(),
        byte_size: 0,
        generation,
    }
}

#[test]
fn screenshot_capture_is_pending_by_default() {
    let mut portal = ScreenCapturePortal::new();

    let command = portal
        .request_capture(request(1, 7, ScreenCaptureMode::Screenshot, "image/png"))
        .unwrap();

    match command {
        PortalCommand::PromptScreenCapture(transfer) => {
            assert_eq!(transfer.transfer, PortalTransferId::from_raw(1));
            assert_eq!(transfer.kind, PortalTransferKind::Screenshot);
            assert_eq!(
                transfer.mime_type,
                Some("screenshot:output:image/png".to_owned())
            );
            assert_eq!(transfer.decision, PortalDecision::Pending);
            assert_eq!(transfer.generation, 7);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn screen_recording_capture_is_pending_by_default() {
    let mut portal = ScreenCapturePortal::new();
    let mut request = request(1, 7, ScreenCaptureMode::ScreenRecording, "video/webm");
    request.scope = ScreenCaptureScope::Desktop;

    let command = portal.request_capture(request).unwrap();

    match command {
        PortalCommand::PromptScreenCapture(transfer) => {
            assert_eq!(
                transfer.mime_type,
                Some("screen-recording:desktop:video/webm".to_owned())
            );
            assert_eq!(transfer.decision, PortalDecision::Pending);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn screen_capture_rejects_wrong_mime_for_mode() {
    let mut portal = ScreenCapturePortal::new();

    assert_eq!(
        portal.request_capture(request(1, 7, ScreenCaptureMode::Screenshot, "video/webm")),
        Err(PortalError::UnsupportedCaptureMimeType)
    );
    assert_eq!(
        portal.request_capture(request(
            2,
            7,
            ScreenCaptureMode::ScreenRecording,
            "image/png"
        )),
        Err(PortalError::UnsupportedCaptureMimeType)
    );
}

#[test]
fn denied_screen_capture_cancels_request() {
    let mut portal = ScreenCapturePortal::new();
    portal
        .request_capture(request(1, 7, ScreenCaptureMode::Screenshot, "image/png"))
        .unwrap();

    assert_eq!(
        portal.deny(PortalTransferId::from_raw(1)),
        Ok(PortalCommand::CancelScreenCapture {
            transfer: PortalTransferId::from_raw(1)
        })
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(1))
            .unwrap()
            .decision,
        PortalDecision::Denied
    );
}

#[test]
fn screen_capture_approval_requires_matching_generation() {
    let mut portal = ScreenCapturePortal::new();
    portal
        .request_capture(request(1, 7, ScreenCaptureMode::Screenshot, "image/png"))
        .unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 7),
        Ok(PortalCommand::HandoffScreenCapture {
            transfer: PortalTransferId::from_raw(1)
        })
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(1))
            .unwrap()
            .decision,
        PortalDecision::Allowed
    );
}

#[test]
fn stale_screen_capture_generation_revokes_request() {
    let mut portal = ScreenCapturePortal::new();
    portal
        .request_capture(request(1, 7, ScreenCaptureMode::Screenshot, "image/png"))
        .unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 8),
        Ok(PortalCommand::CancelScreenCapture {
            transfer: PortalTransferId::from_raw(1)
        })
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(1))
            .unwrap()
            .decision,
        PortalDecision::Revoked
    );
}

#[test]
fn source_owner_change_revokes_pending_screen_capture() {
    let mut portal = ScreenCapturePortal::new();
    portal
        .request_capture(request(1, 7, ScreenCaptureMode::Screenshot, "image/png"))
        .unwrap();
    portal
        .request_capture(request(
            2,
            9,
            ScreenCaptureMode::ScreenRecording,
            "video/webm",
        ))
        .unwrap();

    let commands = portal.source_owner_changed(NamespaceId::from_raw(10), 9);

    assert_eq!(
        commands,
        vec![PortalCommand::CancelScreenCapture {
            transfer: PortalTransferId::from_raw(1)
        }]
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(1))
            .unwrap()
            .decision,
        PortalDecision::Revoked
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(2))
            .unwrap()
            .decision,
        PortalDecision::Pending
    );
}
