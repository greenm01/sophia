use sophia_protocol::*;
use sophia_wayland_authority::*;

fn local() -> AuthorityLocalId {
    AuthorityLocalId::new(7, 1)
}

fn surface() -> SurfaceId {
    SurfaceId::new(9, 1)
}

fn create(reducer: &mut WaylandAuthorityReducer) {
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Created {
            namespace: NamespaceId::from_raw(3),
            local_id: local(),
            surface: surface(),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
        })
        .unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::AssignRole {
            local_id: local(),
            role: WaylandSurfaceRole::Toplevel,
        })
        .unwrap();
}

fn commit_feedback(transaction: TransactionId) -> AuthorityFeedback {
    AuthorityFeedback::Transaction(TransactionCommit {
        transaction,
        outcome: TransactionOutcome::Committed,
        applied_surfaces: vec![surface()],
    })
}

#[test]
fn attach_damage_and_commit_reduce_to_protocol_neutral_transaction() {
    let mut reducer = WaylandAuthorityReducer::new();
    create(&mut reducer);
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Attach {
            local_id: local(),
            buffer: BufferSource::CpuBuffer { handle: 44 },
        })
        .unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Damage {
            local_id: local(),
            damage: Region::single(Rect {
                x: 4,
                y: 5,
                width: 30,
                height: 20,
            }),
        })
        .unwrap();

    let actions = reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction: TransactionId::from_raw(11),
            timeout_msec: 250,
        })
        .unwrap();
    let WaylandAuthorityAction::SurfaceTransaction(transaction) = &actions[0] else {
        panic!("expected transaction");
    };
    assert_eq!(transaction.authority, AuthorityKind::SophiaWayland);
    assert_eq!(transaction.surface, surface());
    assert_eq!(
        transaction.target_buffer,
        BufferSource::CpuBuffer { handle: 44 }
    );
    assert_eq!(transaction.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(transaction.damage.rects.len(), 1);
}

#[test]
fn explicit_backend_release_is_forwarded_once() {
    let mut reducer = WaylandAuthorityReducer::new();
    create(&mut reducer);
    let source = BufferSource::CpuBuffer { handle: 44 };
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Attach {
            local_id: local(),
            buffer: source,
        })
        .unwrap();
    let transaction = TransactionId::from_raw(11);
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction,
            timeout_msec: 250,
        })
        .unwrap();
    reducer
        .apply_feedback(commit_feedback(transaction))
        .unwrap();
    let feedback = BufferReleaseFeedback {
        surface: surface(),
        source,
    };

    assert_eq!(
        reducer
            .apply_feedback(AuthorityFeedback::BufferReleased(feedback))
            .unwrap(),
        vec![WaylandAuthorityAction::BufferReleased(feedback)]
    );
    assert_eq!(
        reducer.apply_feedback(AuthorityFeedback::BufferReleased(feedback)),
        Err(WaylandAuthorityError::StalePresentation)
    );
}

#[test]
fn unacknowledged_configure_keeps_previous_geometry_ready() {
    let mut reducer = WaylandAuthorityReducer::new();
    create(&mut reducer);
    reducer
        .apply_xdg_event(WaylandXdgEvent::Configure {
            local_id: local(),
            serial: 12,
            size: Size {
                width: 1024,
                height: 768,
            },
        })
        .unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Attach {
            local_id: local(),
            buffer: BufferSource::CpuBuffer { handle: 45 },
        })
        .unwrap();
    let actions = reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction: TransactionId::from_raw(13),
            timeout_msec: 250,
        })
        .unwrap();
    let WaylandAuthorityAction::SurfaceTransaction(transaction) = &actions[0] else {
        panic!("expected transaction");
    };
    assert_eq!(transaction.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(transaction.target_geometry.width, 800);
    assert_eq!(transaction.target_geometry.height, 600);
}

#[test]
fn acknowledged_configure_commits_and_presentation_finishes_frame() {
    let mut reducer = WaylandAuthorityReducer::new();
    create(&mut reducer);
    reducer
        .apply_xdg_event(WaylandXdgEvent::Configure {
            local_id: local(),
            serial: 12,
            size: Size {
                width: 1024,
                height: 768,
            },
        })
        .unwrap();
    reducer
        .apply_xdg_event(WaylandXdgEvent::AckConfigure {
            local_id: local(),
            serial: 12,
        })
        .unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Attach {
            local_id: local(),
            buffer: BufferSource::CpuBuffer { handle: 45 },
        })
        .unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::RequestFrame {
            local_id: local(),
            callback: 99,
        })
        .unwrap();
    let transaction = TransactionId::from_raw(13);
    let actions = reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction,
            timeout_msec: 250,
        })
        .unwrap();
    let WaylandAuthorityAction::SurfaceTransaction(packet) = &actions[0] else {
        panic!("expected transaction");
    };
    assert_eq!(packet.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(packet.target_geometry.width, 1024);
    assert_eq!(packet.target_geometry.height, 768);
    reducer
        .apply_feedback(commit_feedback(transaction))
        .unwrap();
    let actions = reducer
        .apply_feedback(AuthorityFeedback::Presented(
            SurfacePresentationFeedback::from_millis(surface(), 1, 500, 1),
        ))
        .unwrap();
    assert_eq!(
        actions,
        vec![WaylandAuthorityAction::FrameDone {
            callback: 99,
            presentation_msec: 500
        }]
    );
}

#[test]
fn scheduled_frame_completes_callback_before_page_flip_without_releasing_buffer() {
    let mut reducer = WaylandAuthorityReducer::new();
    create(&mut reducer);
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Attach {
            local_id: local(),
            buffer: BufferSource::CpuBuffer { handle: 45 },
        })
        .unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::RequestFrame {
            local_id: local(),
            callback: 99,
        })
        .unwrap();
    let transaction = TransactionId::from_raw(14);
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction,
            timeout_msec: 250,
        })
        .unwrap();
    reducer
        .apply_feedback(commit_feedback(transaction))
        .unwrap();

    let scheduled = SurfacePresentationFeedback::from_millis(surface(), 1, 400, 1);
    assert_eq!(
        reducer
            .apply_feedback(AuthorityFeedback::FrameScheduled(scheduled))
            .unwrap(),
        vec![WaylandAuthorityAction::FrameDone {
            callback: 99,
            presentation_msec: 400,
        }]
    );

    // Retirement does not issue the callback again, and does not release the
    // buffer that is still displayed.
    assert_eq!(
        reducer
            .apply_feedback(AuthorityFeedback::Presented(
                SurfacePresentationFeedback::from_millis(surface(), 1, 500, 1),
            ))
            .unwrap(),
        Vec::new()
    );
}

#[test]
fn presenting_latest_generation_releases_coalesced_buffers_and_callbacks() {
    let mut reducer = WaylandAuthorityReducer::new();
    create(&mut reducer);

    for (index, (buffer, callback)) in [(45, 90), (46, 91), (47, 92)].into_iter().enumerate() {
        reducer
            .apply_surface_event(WaylandSurfaceEvent::Attach {
                local_id: local(),
                buffer: BufferSource::CpuBuffer { handle: buffer },
            })
            .unwrap();
        reducer
            .apply_surface_event(WaylandSurfaceEvent::RequestFrame {
                local_id: local(),
                callback,
            })
            .unwrap();
        let transaction = TransactionId::from_raw(20 + index as u64);
        reducer
            .apply_surface_event(WaylandSurfaceEvent::Commit {
                local_id: local(),
                transaction,
                timeout_msec: 250,
            })
            .unwrap();
        reducer
            .apply_feedback(commit_feedback(transaction))
            .unwrap();
    }

    let actions = reducer
        .apply_feedback(AuthorityFeedback::Presented(
            SurfacePresentationFeedback::from_millis(surface(), 3, 500, 3),
        ))
        .unwrap();
    assert_eq!(
        actions,
        vec![
            WaylandAuthorityAction::FrameDone {
                callback: 90,
                presentation_msec: 500,
            },
            WaylandAuthorityAction::FrameDone {
                callback: 91,
                presentation_msec: 500,
            },
            WaylandAuthorityAction::FrameDone {
                callback: 92,
                presentation_msec: 500,
            },
            WaylandAuthorityAction::BufferReleased(BufferReleaseFeedback {
                surface: surface(),
                source: BufferSource::CpuBuffer { handle: 45 },
            }),
            WaylandAuthorityAction::BufferReleased(BufferReleaseFeedback {
                surface: surface(),
                source: BufferSource::CpuBuffer { handle: 46 },
            }),
        ]
    );
}

#[test]
fn detach_after_mapping_emits_null_buffer_unmap_transaction() {
    let mut reducer = WaylandAuthorityReducer::new();
    create(&mut reducer);
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Attach {
            local_id: local(),
            buffer: BufferSource::CpuBuffer { handle: 45 },
        })
        .unwrap();
    let first = TransactionId::from_raw(20);
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction: first,
            timeout_msec: 250,
        })
        .unwrap();
    reducer.apply_feedback(commit_feedback(first)).unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Detach { local_id: local() })
        .unwrap();
    let actions = reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction: TransactionId::from_raw(21),
            timeout_msec: 250,
        })
        .unwrap();
    let WaylandAuthorityAction::SurfaceTransaction(packet) = &actions[0] else {
        panic!("expected transaction");
    };
    assert_eq!(packet.target_buffer, BufferSource::None);
    assert_eq!(packet.previous_committed_generation, 1);
}

#[test]
fn stale_presentation_is_rejected_and_overlapping_commits_are_ordered() {
    let mut reducer = WaylandAuthorityReducer::new();
    create(&mut reducer);
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Attach {
            local_id: local(),
            buffer: BufferSource::CpuBuffer { handle: 45 },
        })
        .unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction: TransactionId::from_raw(30),
            timeout_msec: 250,
        })
        .unwrap();
    let queued = reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction: TransactionId::from_raw(31),
            timeout_msec: 250,
        })
        .unwrap();
    let WaylandAuthorityAction::SurfaceTransaction(queued) = &queued[0] else {
        panic!("expected queued transaction");
    };
    assert_eq!(queued.previous_committed_generation, 1);
    reducer
        .apply_feedback(commit_feedback(TransactionId::from_raw(30)))
        .unwrap();
    assert_eq!(
        reducer.apply_feedback(AuthorityFeedback::Presented(
            SurfacePresentationFeedback::from_millis(surface(), 0, 500, 0),
        )),
        Err(WaylandAuthorityError::StalePresentation)
    );
}

#[test]
fn destroying_a_surface_releases_committed_and_pipelined_buffers_once() {
    let mut reducer = WaylandAuthorityReducer::new();
    create(&mut reducer);
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Attach {
            local_id: local(),
            buffer: BufferSource::DmaBuf { handle: 45 },
        })
        .unwrap();
    let first = TransactionId::from_raw(40);
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction: first,
            timeout_msec: 250,
        })
        .unwrap();
    reducer.apply_feedback(commit_feedback(first)).unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Attach {
            local_id: local(),
            buffer: BufferSource::CpuBuffer { handle: 46 },
        })
        .unwrap();
    reducer
        .apply_surface_event(WaylandSurfaceEvent::Commit {
            local_id: local(),
            transaction: TransactionId::from_raw(41),
            timeout_msec: 250,
        })
        .unwrap();

    let actions = reducer
        .apply_surface_event(WaylandSurfaceEvent::Destroyed { local_id: local() })
        .unwrap();
    assert_eq!(
        actions,
        vec![
            WaylandAuthorityAction::BufferReleased(BufferReleaseFeedback {
                surface: surface(),
                source: BufferSource::DmaBuf { handle: 45 },
            }),
            WaylandAuthorityAction::BufferReleased(BufferReleaseFeedback {
                surface: surface(),
                source: BufferSource::CpuBuffer { handle: 46 },
            }),
            WaylandAuthorityAction::SurfaceDestroyed { surface: surface() },
        ]
    );

    assert_eq!(
        reducer
            .apply_feedback(commit_feedback(TransactionId::from_raw(41)))
            .unwrap(),
        Vec::new()
    );
    assert_eq!(
        reducer.apply_feedback(commit_feedback(TransactionId::from_raw(41))),
        Err(WaylandAuthorityError::UnknownTransaction)
    );
}
