use sophia_backend_live::{
    LiveProductionOutputRuntimeAdapter, LiveProductionPageFlipRetirement,
    LiveProductionPageFlipTracker, LiveProductionPageFlipTrackerError,
    LiveProductionPresentationAdapter,
};
use sophia_engine::{
    DrmKmsMode, DrmKmsOutputDescriptor, DrmKmsOutputRegistry, OutputPresentationFeedback,
    OutputPresentationSchedule, ProductionOutputRuntimeAdapter, ProductionPresentationAdapter,
    ProductionRetirement,
};
use sophia_protocol::{CommittedSurfaceState, OutputId, Size, TransactionCommit};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn live_output_runtime_adapter_keeps_projection_and_invocation_in_one_callback() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&calls);
    let mut adapter = LiveProductionOutputRuntimeAdapter::new(
        2,
        move |index, committed: &[CommittedSurfaceState]| {
            observed.borrow_mut().push((index, committed.len()));
            Ok::<_, String>(index + committed.len())
        },
    );

    assert_eq!(adapter.output_count(), 2);
    assert_eq!(adapter.run_output(0, &[]).unwrap(), 0);
    assert_eq!(adapter.run_output(1, &[]).unwrap(), 1);
    assert_eq!(*calls.borrow(), [(0, 0), (1, 0)]);
}

#[test]
fn live_adapter_keeps_frame_and_retirement_inside_ordered_callbacks() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let compose_calls = Rc::clone(&calls);
    let submit_calls = Rc::clone(&calls);
    let feedback_calls = Rc::clone(&calls);
    let retire_calls = Rc::clone(&calls);
    let pending = Rc::new(RefCell::new(Vec::new()));
    let submitted = Rc::clone(&pending);
    let retired = Rc::clone(&pending);
    let mut adapter = LiveProductionPresentationAdapter::new(
        move |cycle,
              committed: &[CommittedSurfaceState],
              _authority_commits: &[TransactionCommit]| {
            compose_calls.borrow_mut().push(("compose", cycle));
            Ok::<_, &str>(committed.len())
        },
        move |cycle, frame| {
            submit_calls.borrow_mut().push(("submit", cycle));
            submitted.borrow_mut().push(ProductionRetirement {
                cycle,
                retirement: frame + 1,
            });
            Ok::<_, &str>(frame)
        },
        move || {
            retire_calls.borrow_mut().push(("retire", 7));
            Ok::<_, &str>(retired.borrow_mut().drain(..).collect())
        },
        move |cycle, retirement| {
            feedback_calls.borrow_mut().push(("feedback", cycle));
            Ok::<_, &str>(retirement + 1)
        },
    );

    let frame = adapter.compose(7, &[], &[]).unwrap();
    let submission = adapter.submit_frame(7, frame).unwrap();
    let retirement = adapter.poll_retirements().unwrap().pop().unwrap();
    let evidence = adapter
        .route_protocol_feedback(retirement.cycle, retirement.retirement)
        .unwrap();

    assert_eq!(submission, 0);
    assert_eq!(evidence, 2);
    assert_eq!(
        *calls.borrow(),
        [
            ("compose", 7),
            ("submit", 7),
            ("retire", 7),
            ("feedback", 7),
        ]
    );
}

fn production_outputs() -> DrmKmsOutputRegistry {
    let mut outputs = DrmKmsOutputRegistry::new();
    let _ = outputs.upsert(DrmKmsOutputDescriptor {
        output: OutputId::from_raw(7),
        connector_id: 7,
        crtc_id: 8,
        mode: DrmKmsMode {
            size: Size {
                width: 1920,
                height: 1080,
            },
            refresh_millihz: 60_000,
        },
        scale: 1,
    });
    outputs
}

#[test]
fn page_flip_tracker_emits_only_matching_retirements_with_origin_cycle() {
    let output = OutputId::from_raw(7);
    let mut tracker = LiveProductionPageFlipTracker::from_outputs(&production_outputs());

    let frame = tracker.submit(output, 41).unwrap();
    assert_eq!(frame, 1);
    assert!(tracker.drain_retirements().is_empty());

    tracker.observe_page_flip(output, 99, 12, 12_345).unwrap();
    assert_eq!(
        tracker.drain_retirements(),
        [ProductionRetirement {
            cycle: 41,
            retirement: LiveProductionPageFlipRetirement {
                output,
                ust: 12_345,
                msc: 99,
            },
        }]
    );
}

#[test]
fn page_flip_tracker_fails_closed_for_overlap_and_non_monotonic_feedback() {
    let output = OutputId::from_raw(7);
    let mut tracker = LiveProductionPageFlipTracker::from_outputs(&production_outputs());

    let _ = tracker.submit(output, 1).unwrap();
    assert!(matches!(
        tracker.submit(output, 2),
        Err(LiveProductionPageFlipTrackerError::Schedule(
            OutputPresentationSchedule::WaitingForRetirement { .. }
        ))
    ));
    tracker.observe_page_flip(output, 10, 5, 50).unwrap();
    let _ = tracker.drain_retirements();

    let _ = tracker.submit(output, 3).unwrap();
    assert!(matches!(
        tracker.observe_page_flip(output, 10, 6, 60),
        Err(LiveProductionPageFlipTrackerError::Feedback(
            OutputPresentationFeedback::NonMonotonicSequence { .. }
        ))
    ));
    assert!(tracker.drain_retirements().is_empty());
}
