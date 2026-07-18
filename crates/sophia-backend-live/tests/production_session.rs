use sophia_backend_live::LiveProductionPresentationAdapter;
use sophia_engine::ProductionPresentationAdapter;
use sophia_protocol::CommittedSurfaceState;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn live_adapter_keeps_frame_and_retirement_inside_ordered_callbacks() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let compose_calls = Rc::clone(&calls);
    let submit_calls = Rc::clone(&calls);
    let feedback_calls = Rc::clone(&calls);
    let mut adapter = LiveProductionPresentationAdapter::new(
        move |cycle, committed: &[CommittedSurfaceState]| {
            compose_calls.borrow_mut().push(("compose", cycle));
            Ok::<_, &str>(committed.len())
        },
        move |cycle, frame| {
            submit_calls.borrow_mut().push(("submit_retire", cycle));
            Ok::<_, &str>(frame + 1)
        },
        move |cycle, retirement| {
            feedback_calls.borrow_mut().push(("feedback", cycle));
            Ok::<_, &str>(retirement + 1)
        },
    );

    let frame = adapter.compose(7, &[]).unwrap();
    let retirement = adapter.submit_and_retire(7, frame).unwrap();
    let evidence = adapter.route_protocol_feedback(7, retirement).unwrap();

    assert_eq!(evidence, 2);
    assert_eq!(
        *calls.borrow(),
        [("compose", 7), ("submit_retire", 7), ("feedback", 7)]
    );
}
