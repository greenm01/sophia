use sophia_cli::input_proof::{
    PhysicalTextProof, PhysicalTextProofEvent, PhysicalTextProofProgress, application_exit_overdue,
    cursor_repaint_preserves_application, pointer_proof_suppresses_return,
    pointer_selection_pending, pointer_selection_waiting,
};

#[test]
fn pointer_selection_stays_pending_until_a_button_is_routed() {
    assert!(pointer_selection_pending(true, 0));
    assert!(!pointer_selection_pending(true, 1));
    assert!(!pointer_selection_pending(false, 0));
}

#[test]
fn pointer_selection_wait_remains_bounded_after_motion_changes_pixels() {
    assert!(pointer_selection_waiting(true, true, true, true, 0, true));
    assert!(pointer_selection_waiting(true, true, true, true, 1, false));
    assert!(!pointer_selection_waiting(true, true, true, true, 1, true));
    assert!(!pointer_selection_waiting(true, true, true, false, 0, true));
    assert!(!pointer_selection_waiting(false, true, true, true, 0, true));
}

#[test]
fn cursor_only_frame_cannot_satisfy_application_preservation() {
    assert!(!cursor_repaint_preserves_application(0, 10_000));
    assert!(!cursor_repaint_preserves_application(1, 12 * 16 * 4));
    assert!(cursor_repaint_preserves_application(1, 12 * 16 * 4 + 1));
}

#[test]
fn application_exit_overdue_only_when_surface_gone_and_client_alive() {
    // The overdue window opens only when the proof surface is gone but the
    // client process has not exited.
    assert!(application_exit_overdue(true, true, false));
    // A present surface means the client may still be running legitimately.
    assert!(!application_exit_overdue(true, false, false));
    // A reaped client exits the session loop through the normal proof break.
    assert!(!application_exit_overdue(true, true, true));
    assert!(!application_exit_overdue(true, false, true));
    // Non-application sessions never arm the application exit watchdog.
    assert!(!application_exit_overdue(false, true, false));
}

fn event(keycode: u8, pressed: bool) -> PhysicalTextProofEvent {
    PhysicalTextProofEvent {
        keycode,
        pressed,
        state: 0,
    }
}

fn sophia_events() -> Vec<PhysicalTextProofEvent> {
    [39, 32, 33, 43, 31, 38, 36]
        .into_iter()
        .flat_map(|keycode| [event(keycode, true), event(keycode, false)])
        .collect()
}

#[test]
fn accepts_exact_lowercase_text_and_return_pairs() {
    let mut proof = PhysicalTextProof::new("sophia").expect("proof should build");
    let events = sophia_events();

    assert_eq!(proof.expected_events(), 14);
    for (index, event) in events.into_iter().enumerate() {
        let progress = proof.observe(event).expect("event should match");
        assert_eq!(proof.matched_events(), index + 1);
        assert_eq!(
            progress,
            if index == 13 {
                PhysicalTextProofProgress::Complete
            } else {
                PhysicalTextProofProgress::Awaiting
            }
        );
    }
    assert!(proof.is_complete());
    assert_eq!(
        proof
            .observe(event(40, true))
            .expect("completed proof should remain frozen"),
        PhysicalTextProofProgress::Complete
    );
    assert_eq!(proof.matched_events(), 14);
}

#[test]
fn application_text_completes_without_a_submit_key() {
    let mut proof = PhysicalTextProof::new_without_submit("sophia").unwrap();
    let events = sophia_events();
    assert_eq!(proof.expected_events(), 12);
    for event in events.into_iter().take(12) {
        proof.observe(event).unwrap();
    }
    assert!(proof.is_complete());
}

#[test]
fn rejects_wrong_key_modifier_release_order_and_repeat() {
    for wrong in [
        event(40, true),
        PhysicalTextProofEvent {
            keycode: 39,
            pressed: true,
            state: 1,
        },
        event(39, false),
    ] {
        let mut proof = PhysicalTextProof::new("sophia").expect("proof should build");
        assert!(proof.observe(wrong).is_err());
        assert_eq!(proof.matched_events(), 0);
    }

    let mut proof = PhysicalTextProof::new("sophia").expect("proof should build");
    proof
        .observe(event(39, true))
        .expect("first press should match");
    assert!(proof.observe(event(39, true)).is_err());
    assert_eq!(proof.matched_events(), 1);
}

#[test]
fn completed_application_text_suppresses_return_press_and_release_only() {
    assert!(!pointer_proof_suppresses_return(true, 28, false));
    assert!(!pointer_proof_suppresses_return(true, 15, true));
    assert!(pointer_proof_suppresses_return(true, 28, true));
    assert!(!pointer_proof_suppresses_return(false, 28, true));
}

#[test]
fn remains_incomplete_without_return_release() {
    let mut proof = PhysicalTextProof::new("sophia").expect("proof should build");
    for event in sophia_events().into_iter().take(13) {
        proof.observe(event).expect("event should match");
    }

    assert!(!proof.is_complete());
    assert_eq!(proof.matched_events(), 13);
}

#[test]
fn enforces_text_bounds() {
    assert!(PhysicalTextProof::new("").is_err());
    assert!(PhysicalTextProof::new("Sophia").is_err());
    assert!(PhysicalTextProof::new("abcdefghijklmnopqrstuvwxy").is_err());
    assert!(PhysicalTextProof::new("abcdefghijklmnopqrstuvwx").is_ok());
}
